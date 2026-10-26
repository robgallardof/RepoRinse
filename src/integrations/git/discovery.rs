//! Where Git looks for hooks: repository layout, `core.hooksPath` at every scope, and the RepoRinse home.

use std::path::{Path, PathBuf};

use super::command::{Git, path_key, slash_path};

/// One `core.hooksPath` value and the config scope it comes from (`system`, `global`, `local`, `worktree`,
/// `command`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksPathSetting {
    pub scope: String,
    pub value: String,
}

/// A non-bare repository as seen from one of its worktrees.
#[derive(Debug, Clone)]
pub struct RepoLayout {
    /// Top of the current worktree; hooks run here.
    pub top: PathBuf,
    /// Shared `.git` directory (the main one, also for linked worktrees).
    pub common_dir: PathBuf,
    /// Directory Git really runs hooks from (`git rev-parse --git-path hooks`), absolute.
    pub hooks_dir: PathBuf,
    /// Every `core.hooksPath` value Git sees, lowest precedence first; the last one wins.
    pub hooks_path: Vec<HooksPathSetting>,
}

impl RepoLayout {
    /// The `core.hooksPath` Git uses, if any.
    pub fn effective_hooks_path(&self) -> Option<&HooksPathSetting> {
        self.hooks_path.last()
    }

    /// True when the hooks directory comes from a user- or machine-wide setting.
    pub fn hooks_dir_is_shared(&self) -> bool {
        self.effective_hooks_path()
            .is_some_and(|s| s.scope == "global" || s.scope == "system")
    }

    /// Hooks directory relative to the worktree top, when it lives inside it (e.g. `.husky/_`).
    pub fn hooks_dir_in_worktree(&self) -> Option<String> {
        let top = format!("{}/", path_key(&slash_path(&self.top)));
        let dir = slash_path(&self.hooks_dir);
        let key = path_key(&dir);
        key.starts_with(&top)
            .then(|| dir[top.len()..].trim_end_matches('/').to_string())
    }

    pub fn git(&self) -> Git {
        Git::at(&self.top)
    }
}

/// Inspects the repository containing `dir`. Refuses bare repositories (no commits are made there).
pub fn discover(dir: &Path) -> Result<RepoLayout, String> {
    let probe = Git::at(dir);
    let bare = probe
        .text(&["rev-parse", "--is-bare-repository"])
        .map_err(|_| format!("{} is not inside a Git repository", dir.display()))?;
    if bare == "true" {
        return Err(format!(
            "{} is a bare repository: there is no working tree where commits are made, so commit hooks \
             would never run. Install RepoRinse in a clone instead.",
            dir.display()
        ));
    }
    let top = PathBuf::from(probe.text(&["rev-parse", "--show-toplevel"])?);
    // From the top, relative values of core.hooksPath resolve exactly as Git resolves them for hooks.
    let git = Git::at(&top);
    let common_dir =
        PathBuf::from(git.text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?);
    let hooks_dir =
        PathBuf::from(git.text(&["rev-parse", "--path-format=absolute", "--git-path", "hooks"])?);
    let hooks_path = git
        .config_values(&["--show-scope", "--get-all", "core.hooksPath"])?
        .iter()
        .filter_map(|line| {
            let (scope, value) = line.split_once('\t')?;
            Some(HooksPathSetting {
                scope: scope.to_string(),
                value: value.to_string(),
            })
        })
        .collect();
    Ok(RepoLayout {
        top,
        common_dir,
        hooks_dir,
        hooks_path,
    })
}

/// `core.hooksPath` values set in the global and system files (`~` expanded), from `dir` (any directory).
pub fn shared_hooks_path(dir: &Path) -> Result<Vec<HooksPathSetting>, String> {
    let git = Git::at(dir);
    let mut out = Vec::new();
    for scope in ["system", "global"] {
        let flag = format!("--{scope}");
        for value in git.config_values(&[&flag, "--type=path", "--get-all", "core.hooksPath"])? {
            out.push(HooksPathSetting {
                scope: scope.to_string(),
                value,
            });
        }
    }
    Ok(out)
}

/// RepoRinse's per-user directory: `$REPORINSE_CONFIG_HOME`, else `$XDG_CONFIG_HOME/reporinse`,
/// else `%APPDATA%\reporinse` on Windows, else `~/.config/reporinse`.
pub fn reporinse_home() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = var("REPORINSE_CONFIG_HOME") {
        return Some(dir);
    }
    if let Some(dir) = var("XDG_CONFIG_HOME") {
        return Some(dir.join("reporinse"));
    }
    if cfg!(windows)
        && let Some(dir) = var("APPDATA")
    {
        return Some(dir.join("reporinse"));
    }
    var("HOME")
        .or_else(|| var("USERPROFILE"))
        .map(|home| home.join(".config").join("reporinse"))
}
