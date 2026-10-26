//! Git hook installer (scopes `repo` and `user`).
//!
//! - Repo scope writes shims into the directory Git really uses (`git rev-parse --git-path hooks`, which honours
//!   a local `core.hooksPath` such as Husky's `.husky/_`). A foreign hook with the same name is moved to
//!   `<name>.reporinse-chained` and runs first. Versioned hook files are never touched (conflict instead).
//! - User scope creates `<reporinse-home>/git-hooks` and points `git config --global core.hooksPath` at it,
//!   only when no global/system `core.hooksPath` exists. Its shims also run each repository's `.git/hooks/<name>`.
//! - Everything written is recorded in a manifest (see `manifest.rs`); uninstall removes only files whose hash
//!   still matches and restores chained hooks and config exactly.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::command::{Git, path_key, slash_path};
use super::discovery::{self, RepoLayout};
use super::manifest::{self, ChainedHook, ConfigChange, ManagedFile, Manifest};
use super::shim::{self, CHAINED_SUFFIX, COMMIT_MSG, PRE_COMMIT, ShimSpec};
use super::transaction::{self, Step, file_mode, read_optional};
use crate::core::integration::{
    InstallAction, InstallActionKind, InstallRequest, InstallResult, InstallScope, Installer,
    IntegrationStatus,
};
use crate::core::model::TargetKind;

pub const MANIFEST_DIR: &str = "reporinse";
pub const REPO_MANIFEST: &str = "git-manifest.json";
pub const USER_HOOKS_DIR: &str = "git-hooks";
pub const USER_MANIFEST: &str = ".reporinse-manifest.json";
const HOOKS_PATH_KEY: &str = "core.hooksPath";

pub struct GitInstaller {
    /// Binary recorded in the shims; defaults to the running executable.
    binary: Option<PathBuf>,
}

impl GitInstaller {
    pub fn new() -> GitInstaller {
        GitInstaller { binary: None }
    }

    /// Records `binary` in the shims instead of the running executable.
    pub fn with_binary(binary: impl Into<PathBuf>) -> GitInstaller {
        GitInstaller {
            binary: Some(binary.into()),
        }
    }

    fn binary(&self) -> String {
        let path = self
            .binary
            .clone()
            .or_else(|| std::env::current_exe().ok())
            .unwrap_or_else(|| PathBuf::from("reporinse"));
        slash_path(&path)
    }
}

impl Default for GitInstaller {
    fn default() -> Self {
        GitInstaller::new()
    }
}

/// Hooks needed by a set of modules: commit-message rules → `commit-msg`, file/change-set rules → `pre-commit`.
pub fn hooks_for(modules: &[String]) -> Vec<&'static str> {
    let registry = crate::modules::registry();
    let mut hooks = BTreeSet::new();
    for id in modules {
        let Some(rule) = registry.find(id) else {
            continue;
        };
        for target in rule.metadata().targets {
            match target {
                TargetKind::CommitMessage => {
                    hooks.insert(COMMIT_MSG);
                }
                TargetKind::File | TargetKind::ChangeSet => {
                    hooks.insert(PRE_COMMIT);
                }
                TargetKind::AgentEvent => {}
            }
        }
    }
    // Stable order: commit-msg, pre-commit.
    [COMMIT_MSG, PRE_COMMIT]
        .into_iter()
        .filter(|h| hooks.contains(h))
        .collect()
}

/// Where one scope's files live.
struct Target {
    scope: InstallScope,
    hooks_dir: PathBuf,
    manifest_path: PathBuf,
    layout: Option<RepoLayout>,
    /// Directory `git config --global` runs from.
    git: Git,
}

fn scope_name(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::Repo => "repo",
        InstallScope::User => "user",
    }
}

fn resolve(scope: InstallScope, root: &Path) -> Result<Target, String> {
    match scope {
        InstallScope::Repo => {
            let layout = discovery::discover(root)?;
            let manifest_path = layout.common_dir.join(MANIFEST_DIR).join(REPO_MANIFEST);
            Ok(Target {
                scope,
                hooks_dir: layout.hooks_dir.clone(),
                manifest_path,
                git: layout.git(),
                layout: Some(layout),
            })
        }
        InstallScope::User => {
            let home = discovery::reporinse_home().ok_or(
                "cannot find the user configuration directory (set REPORINSE_CONFIG_HOME or HOME)",
            )?;
            let hooks_dir = home.join(USER_HOOKS_DIR);
            Ok(Target {
                scope,
                manifest_path: hooks_dir.join(USER_MANIFEST),
                hooks_dir,
                layout: discovery::discover(root).ok(),
                git: Git::at(if root.is_dir() { root } else { Path::new(".") }),
            })
        }
    }
}

fn action(kind: InstallActionKind, target: &Path, description: impl Into<String>) -> InstallAction {
    InstallAction {
        kind,
        target: slash_path(target),
        description: description.into(),
    }
}

/// Planned work: shown in `--dry-run`, executed otherwise.
#[derive(Default)]
struct Plan {
    result: InstallResult,
    steps: Vec<Step>,
}

impl Plan {
    fn push(&mut self, action: InstallAction, step: Option<Step>) {
        self.result.actions.push(action);
        self.steps.extend(step);
    }

    fn finish(mut self, request: &InstallRequest, git: &Git) -> InstallResult {
        if !self.result.conflicts.is_empty() || request.dry_run {
            if request.dry_run && self.result.conflicts.is_empty() {
                self.result
                    .notes
                    .push("dry run: nothing was written".to_string());
            }
            return self.result;
        }
        if let Err(e) = transaction::execute(&self.steps, git) {
            self.result.conflicts.push(e);
        }
        self.result
    }
}

/// Hook managers recognised from the repository layout.
fn detect_tools(layout: Option<&RepoLayout>, hooks_dir: &Path) -> Vec<String> {
    let mut found = BTreeSet::new();
    if slash_path(hooks_dir).contains("/.husky") {
        found.insert("husky");
    }
    if let Some(layout) = layout {
        let top = &layout.top;
        if top.join(".husky").is_dir() {
            found.insert("husky");
        }
        if [
            "lefthook.yml",
            ".lefthook.yml",
            "lefthook.yaml",
            ".lefthook.yaml",
        ]
        .iter()
        .any(|f| top.join(f).is_file())
        {
            found.insert("lefthook");
        }
        if top.join(".pre-commit-config.yaml").is_file() {
            found.insert("pre-commit");
        }
    }
    found.into_iter().map(str::to_string).collect()
}

/// Tool that generated a hook file, from its content.
fn hook_tool(content: &[u8], hooks_dir: &Path) -> Option<String> {
    let text = String::from_utf8_lossy(content);
    let tool = if text.contains("no-ai-coauthor") {
        "no-ai-coauthor"
    } else if text.contains("lefthook") {
        "lefthook"
    } else if text.contains("File generated by pre-commit") || text.contains("pre_commit") {
        "pre-commit"
    } else if text.contains("husky") || slash_path(hooks_dir).contains("/.husky") {
        "husky"
    } else {
        return None;
    };
    Some(tool.to_string())
}

fn tool_note(tool: &str) -> &'static str {
    match tool {
        "husky" => {
            "Husky regenerates its hooks on `npm install` / `husky`; run `reporinse install` again afterwards \
             (until then `reporinse uninstall` reports the regenerated files as drift)."
        }
        "lefthook" => {
            "`lefthook install` rewrites the hooks; run `reporinse install` again afterwards."
        }
        "pre-commit" => {
            "`pre-commit install` moves foreign hooks to *.legacy; run `reporinse install` again afterwards."
        }
        "no-ai-coauthor" => {
            "the no-ai-coauthor hook was chained and still runs first; see docs/migration/no-ai-couathor.md to \
             retire it."
        }
        _ => "",
    }
}

fn is_tracked(layout: Option<&RepoLayout>, hook: &str) -> bool {
    let Some(layout) = layout else {
        return false;
    };
    let Some(rel) = layout.hooks_dir_in_worktree() else {
        return false;
    };
    let path = if rel.is_empty() {
        hook.to_string()
    } else {
        format!("{rel}/{hook}")
    };
    layout
        .git()
        .text(&["ls-files", "--", &path])
        .is_ok_and(|out| !out.trim().is_empty())
}

fn hooks_path_conflict(value: &str, scope: &str) -> String {
    let marker_dir = Path::new(value);
    let old_tool = std::fs::read(marker_dir.join(COMMIT_MSG))
        .ok()
        .filter(|b| String::from_utf8_lossy(b).contains("no-ai-coauthor"));
    let mut msg = format!(
        "core.hooksPath is already set in the {scope} Git config to '{value}', and RepoRinse never overrides it. \
         Options: (1) install per repository with `reporinse install --scope repo` in repositories that set \
         their own core.hooksPath; (2) call `reporinse hook git commit-msg \"$1\"` from the hook in '{value}'; \
         (3) unset it yourself (`git config --{scope} --unset core.hooksPath`) and run this command again."
    );
    if old_tool.is_some() {
        msg.push_str(
            " That directory holds the no-ai-coauthor hook: run its `install.sh --uninstall` (or `install.ps1 \
             -Uninstall`) first; see docs/migration/no-ai-couathor.md.",
        );
    }
    msg
}

impl GitInstaller {
    fn plan_install(&self, request: &InstallRequest) -> Plan {
        let mut plan = Plan::default();
        let target = match resolve(request.scope, &request.root) {
            Ok(t) => t,
            Err(e) => {
                plan.result.conflicts.push(e);
                return plan;
            }
        };
        let old = match Manifest::load(&target.manifest_path) {
            Ok(m) => m,
            Err(e) => {
                plan.result.conflicts.push(e);
                return plan;
            }
        };

        let mut config = old.as_ref().map(|m| m.config.clone()).unwrap_or_default();
        match target.scope {
            InstallScope::Repo => {
                let layout = target.layout.as_ref().expect("repo scope has a layout");
                if let Some(setting) = layout
                    .effective_hooks_path()
                    .filter(|_| layout.hooks_dir_is_shared())
                {
                    plan.result.conflicts.push(format!(
                        "this repository uses the {} core.hooksPath '{}': installing there would affect every \
                         repository. {}",
                        setting.scope,
                        setting.value,
                        if is_ours_user_dir(&setting.value) {
                            "RepoRinse is already installed for the user; its hooks also run this repository's \
                             .git/hooks."
                        } else {
                            "Set a local core.hooksPath for this repository, or call `reporinse hook git \
                             commit-msg \"$1\"` from that hook yourself."
                        }
                    ));
                    return plan;
                }
                if let Some(setting) = layout.effective_hooks_path() {
                    plan.result.notes.push(format!(
                        "using the {} core.hooksPath '{}' (left unchanged)",
                        setting.scope, setting.value
                    ));
                }
            }
            InstallScope::User => {
                let shared = match discovery::shared_hooks_path(target.git.dir()) {
                    Ok(s) => s,
                    Err(e) => {
                        plan.result.conflicts.push(e);
                        return plan;
                    }
                };
                let ours = path_key(&slash_path(&target.hooks_dir));
                if let Some(other) = shared.iter().find(|s| path_key(&s.value) != ours) {
                    plan.result
                        .conflicts
                        .push(hooks_path_conflict(&other.value, &other.scope));
                    return plan;
                }
                if shared.is_empty() {
                    let value = slash_path(&target.hooks_dir);
                    plan.push(
                        action(
                            InstallActionKind::SetConfig,
                            Path::new(HOOKS_PATH_KEY),
                            format!("git config --global core.hooksPath {value}"),
                        ),
                        Some(Step::SetGlobalConfig {
                            key: HOOKS_PATH_KEY.to_string(),
                            value: value.clone(),
                        }),
                    );
                    config = vec![ConfigChange {
                        scope: "global".to_string(),
                        key: HOOKS_PATH_KEY.to_string(),
                        value,
                        previous: None,
                    }];
                }
                if let Some(local) = target
                    .layout
                    .as_ref()
                    .and_then(|l| l.effective_hooks_path())
                    .filter(|s| s.scope != "global" && s.scope != "system")
                {
                    plan.result.notes.push(format!(
                        "this repository sets its own core.hooksPath '{}' ({}); Git uses it instead of the user \
                         hooks here. Use `reporinse install --scope repo` in it.",
                        local.value, local.scope
                    ));
                }
                plan.result.notes.push(
                    "user hooks also run each repository's own .git/hooks/<name>, which a global \
                     core.hooksPath would otherwise hide."
                        .to_string(),
                );
            }
        }

        let binary = self.binary();
        let wanted = hooks_for(&request.modules);
        let detected = detect_tools(target.layout.as_ref(), &target.hooks_dir);
        let mut files = Vec::new();
        let mut chained: Vec<ChainedHook> = Vec::new();
        let mut tools: BTreeSet<String> = detected.iter().cloned().collect();

        for hook in &wanted {
            let path = target.hooks_dir.join(hook);
            let content = shim::render(&ShimSpec {
                hook,
                scope: target.scope,
                binary: &binary,
                modules: &request.modules,
            });
            let new_hash = manifest::sha256_hex(content.as_bytes());
            let write = Step::Write {
                path: path.clone(),
                bytes: content.into_bytes(),
                executable: true,
            };
            let current = match read_optional(&path) {
                Ok(c) => c,
                Err(e) => return plan_error(plan, format!("{}: {e}", path.display())),
            };
            let entry = old.as_ref().and_then(|m| m.file(hook));
            if let Some(c) = old.as_ref().and_then(|m| m.chained(hook)) {
                chained.push(c.clone());
            }
            match (current, entry) {
                (None, _) => plan.push(
                    action(InstallActionKind::CreateFile, &path, format!("RepoRinse {hook} hook")),
                    Some(write),
                ),
                (Some(bytes), Some(entry)) if manifest::sha256_hex(&bytes) == entry.sha256 => {
                    if new_hash == entry.sha256 {
                        plan.push(
                            action(InstallActionKind::Skip, &path, "already up to date"),
                            None,
                        );
                    } else {
                        plan.push(
                            action(InstallActionKind::UpdateFile, &path, format!("upgrade RepoRinse {hook} hook")),
                            Some(write),
                        );
                    }
                }
                (Some(_), Some(_)) if request.force => plan.push(
                    action(
                        InstallActionKind::UpdateFile,
                        &path,
                        "overwrite: the file changed since RepoRinse wrote it (--force)",
                    ),
                    Some(write),
                ),
                (Some(_), Some(_)) => plan.result.conflicts.push(format!(
                    "{} changed since RepoRinse installed it (another tool or a person edited it). Inspect it, \
                     then re-run with --force to overwrite, or `reporinse uninstall` to remove RepoRinse.",
                    slash_path(&path)
                )),
                (Some(bytes), None) if shim::is_managed(&bytes) => plan.push(
                    action(
                        InstallActionKind::UpdateFile,
                        &path,
                        "rewrite RepoRinse hook missing from the manifest",
                    ),
                    Some(write),
                ),
                (Some(bytes), None) => {
                    if is_tracked(target.layout.as_ref(), hook) {
                        plan.result.conflicts.push(format!(
                            "{} is versioned in this repository; RepoRinse does not modify tracked files. Add \
                             `reporinse hook git {hook} \"$@\"` to it yourself (see docs/integrations/git.md).",
                            slash_path(&path)
                        ));
                        continue;
                    }
                    let stored = target.hooks_dir.join(format!("{hook}{CHAINED_SUFFIX}"));
                    if stored.exists() {
                        plan.result.conflicts.push(format!(
                            "{} already exists and is not recorded in the manifest; move it away first.",
                            slash_path(&stored)
                        ));
                        continue;
                    }
                    let tool = hook_tool(&bytes, &target.hooks_dir);
                    if let Some(t) = &tool {
                        tools.insert(t.clone());
                    }
                    plan.push(
                        action(
                            InstallActionKind::CreateFile,
                            &stored,
                            format!(
                                "keep the existing {hook} hook{} here; it runs first",
                                tool.as_deref().map(|t| format!(" ({t})")).unwrap_or_default()
                            ),
                        ),
                        Some(Step::Move {
                            from: path.clone(),
                            to: stored.clone(),
                        }),
                    );
                    plan.push(
                        action(InstallActionKind::CreateFile, &path, format!("RepoRinse {hook} dispatcher")),
                        Some(write),
                    );
                    chained.push(ChainedHook {
                        hook: hook.to_string(),
                        original: hook.to_string(),
                        stored_as: format!("{hook}{CHAINED_SUFFIX}"),
                        sha256: manifest::sha256_hex(&bytes),
                        mode: file_mode(&path),
                        tool,
                    });
                }
            }
            files.push(ManagedFile {
                hook: hook.to_string(),
                path: hook.to_string(),
                sha256: new_hash,
            });
        }

        // Hooks no longer needed by the selected modules go away (and give back what they chained).
        if let Some(old) = &old {
            for f in old
                .files
                .iter()
                .filter(|f| !wanted.contains(&f.hook.as_str()))
            {
                remove_hook(
                    &mut plan,
                    &target.hooks_dir,
                    f,
                    old.chained(&f.hook),
                    request.force,
                );
            }
        }

        for tool in &tools {
            let note = tool_note(tool);
            if !note.is_empty() {
                plan.result.notes.push(format!("{tool} detected: {note}"));
            }
        }

        let new = Manifest {
            format: manifest::FORMAT,
            tool: "reporinse".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            provider: super::PROVIDER.to_string(),
            scope: scope_name(target.scope).to_string(),
            hooks_dir: slash_path(&target.hooks_dir),
            binary,
            modules: request.modules.clone(),
            installed_at: manifest::now_rfc3339(),
            files,
            chained,
            config,
            detected: tools.into_iter().collect(),
        };
        if old.as_ref().is_some_and(|o| o.same_content(&new)) {
            plan.push(
                action(
                    InstallActionKind::Skip,
                    &target.manifest_path,
                    "manifest unchanged",
                ),
                None,
            );
        } else {
            plan.push(
                action(
                    if old.is_some() {
                        InstallActionKind::UpdateFile
                    } else {
                        InstallActionKind::CreateFile
                    },
                    &target.manifest_path,
                    format!("manifest (modules: {})", request.modules.join(", ")),
                ),
                Some(Step::Write {
                    path: target.manifest_path.clone(),
                    bytes: new.to_json().into_bytes(),
                    executable: false,
                }),
            );
        }
        plan
    }

    fn plan_uninstall(&self, request: &InstallRequest) -> Plan {
        let mut plan = Plan::default();
        let target = match resolve(request.scope, &request.root) {
            Ok(t) => t,
            Err(e) => return plan_error(plan, e),
        };
        let old = match Manifest::load(&target.manifest_path) {
            Ok(Some(m)) => m,
            Ok(None) => {
                plan.result.notes.push(format!(
                    "RepoRinse Git hooks are not installed for scope {} (no manifest at {})",
                    scope_name(target.scope),
                    slash_path(&target.manifest_path)
                ));
                return plan;
            }
            Err(e) => return plan_error(plan, e),
        };

        // Partial uninstall: keep the other modules installed.
        if !request.modules.is_empty() {
            let remaining: Vec<String> = old
                .modules
                .iter()
                .filter(|m| !request.modules.contains(m))
                .cloned()
                .collect();
            if remaining.len() == old.modules.len() {
                plan.result.notes.push(format!(
                    "none of {} is installed (installed: {})",
                    request.modules.join(", "),
                    old.modules.join(", ")
                ));
                return plan;
            }
            if !remaining.is_empty() {
                let mut keep = request.clone();
                keep.modules = remaining;
                let installer = GitInstaller {
                    binary: Some(PathBuf::from(&old.binary)),
                };
                return installer.plan_install(&keep);
            }
        }

        let hooks_dir = PathBuf::from(&old.hooks_dir);
        for f in &old.files {
            remove_hook(
                &mut plan,
                &hooks_dir,
                f,
                old.chained(&f.hook),
                request.force,
            );
        }
        for change in &old.config {
            let current = discovery::shared_hooks_path(target.git.dir())
                .ok()
                .and_then(|v| v.into_iter().rfind(|s| s.scope == change.scope))
                .map(|s| s.value);
            if current.as_deref().map(path_key) != Some(path_key(&change.value)) {
                plan.result.notes.push(format!(
                    "{} {} changed since install ({}); left as is",
                    change.scope,
                    change.key,
                    current.as_deref().unwrap_or("unset")
                ));
                continue;
            }
            let (kind, desc, step) = match &change.previous {
                None => (
                    InstallActionKind::UnsetConfig,
                    format!("git config --global --unset {}", change.key),
                    Step::UnsetGlobalConfig {
                        key: change.key.clone(),
                    },
                ),
                Some(prev) => (
                    InstallActionKind::SetConfig,
                    format!("restore {} = {prev}", change.key),
                    Step::SetGlobalConfig {
                        key: change.key.clone(),
                        value: prev.clone(),
                    },
                ),
            };
            plan.push(action(kind, Path::new(&change.key), desc), Some(step));
        }
        plan.push(
            action(
                InstallActionKind::DeleteFile,
                &target.manifest_path,
                "manifest",
            ),
            Some(Step::Delete {
                path: target.manifest_path.clone(),
            }),
        );
        let dir = match target.scope {
            InstallScope::Repo => target.manifest_path.parent().map(Path::to_path_buf),
            InstallScope::User => Some(hooks_dir),
        };
        if let Some(dir) = dir {
            plan.steps.push(Step::RemoveDirIfEmpty { path: dir });
        }
        plan
    }
}

fn plan_error(mut plan: Plan, message: impl Into<String>) -> Plan {
    plan.result.conflicts.push(message.into());
    plan
}

/// True when `value` is the user-scope hooks directory of RepoRinse.
fn is_ours_user_dir(value: &str) -> bool {
    discovery::reporinse_home()
        .is_some_and(|h| path_key(&slash_path(&h.join(USER_HOOKS_DIR))) == path_key(value))
}

/// Deletes one managed shim (if unchanged) and moves its chained hook back.
fn remove_hook(
    plan: &mut Plan,
    hooks_dir: &Path,
    file: &ManagedFile,
    chained: Option<&ChainedHook>,
    force: bool,
) {
    let path = hooks_dir.join(&file.path);
    let mut freed = true;
    match read_optional(&path) {
        Ok(None) => plan
            .result
            .notes
            .push(format!("{} was already removed", slash_path(&path))),
        Ok(Some(bytes)) if manifest::sha256_hex(&bytes) == file.sha256 => plan.push(
            action(
                InstallActionKind::DeleteFile,
                &path,
                format!("RepoRinse {} hook", file.hook),
            ),
            Some(Step::Delete { path: path.clone() }),
        ),
        Ok(Some(_)) if force => plan.push(
            action(
                InstallActionKind::DeleteFile,
                &path,
                "changed since install; removed because of --force",
            ),
            Some(Step::Delete { path: path.clone() }),
        ),
        Ok(Some(_)) => {
            freed = false;
            plan.result.conflicts.push(format!(
                "{} changed since RepoRinse installed it; it was left in place. Check it and delete it by hand, \
                 or re-run with --force.",
                slash_path(&path)
            ));
        }
        Err(e) => {
            freed = false;
            plan.result
                .conflicts
                .push(format!("{}: {e}", path.display()));
        }
    }

    let Some(chained) = chained else {
        return;
    };
    let stored = hooks_dir.join(&chained.stored_as);
    let original = hooks_dir.join(&chained.original);
    match read_optional(&stored) {
        Ok(None) => plan.result.notes.push(format!(
            "the chained hook {} no longer exists; nothing to restore",
            slash_path(&stored)
        )),
        Ok(Some(bytes)) if manifest::sha256_hex(&bytes) != chained.sha256 && !force => {
            plan.result.conflicts.push(format!(
                "{} changed since install; restore it by hand (rename it to {}) or re-run with --force.",
                slash_path(&stored),
                chained.original
            ));
        }
        Ok(Some(_)) if !freed => {}
        Ok(Some(_)) => plan.push(
            action(
                InstallActionKind::RestoreFile,
                &original,
                format!("restore the original {} hook", chained.hook),
            ),
            Some(Step::Move {
                from: stored,
                to: original,
            }),
        ),
        Err(e) => plan
            .result
            .conflicts
            .push(format!("{}: {e}", stored.display())),
    }
}

impl Installer for GitInstaller {
    fn provider(&self) -> &'static str {
        super::PROVIDER
    }

    fn supported_scopes(&self) -> &'static [InstallScope] {
        &[InstallScope::Repo, InstallScope::User]
    }

    fn status(&self, scope: InstallScope, root: &Path) -> IntegrationStatus {
        let mut status = IntegrationStatus {
            provider: super::PROVIDER.to_string(),
            scope,
            detected: Git::at(root).text(&["--version"]).is_ok(),
            installed: false,
            modules: Vec::new(),
            problems: Vec::new(),
        };
        let target = match resolve(scope, root) {
            Ok(t) => t,
            Err(e) => {
                status.problems.push(e);
                return status;
            }
        };
        let manifest = match Manifest::load(&target.manifest_path) {
            Ok(Some(m)) => m,
            Ok(None) => return status,
            Err(e) => {
                status.problems.push(e);
                return status;
            }
        };
        status.installed = true;
        status.modules = manifest.modules.clone();
        let dir = PathBuf::from(&manifest.hooks_dir);
        for f in &manifest.files {
            let path = dir.join(&f.path);
            match read_optional(&path) {
                Ok(Some(b)) if manifest::sha256_hex(&b) == f.sha256 => {}
                Ok(Some(_)) => status
                    .problems
                    .push(format!("{} changed since install", slash_path(&path))),
                _ => status
                    .problems
                    .push(format!("{} is missing", slash_path(&path))),
            }
        }
        if path_key(&manifest.hooks_dir) != path_key(&slash_path(&target.hooks_dir)) {
            status.problems.push(format!(
                "Git now uses hooks from {}, not {}",
                slash_path(&target.hooks_dir),
                manifest.hooks_dir
            ));
        }
        status
    }

    fn install(&self, request: &InstallRequest) -> InstallResult {
        let git = Git::at(&request.root);
        self.plan_install(request).finish(request, &git)
    }

    fn uninstall(&self, request: &InstallRequest) -> InstallResult {
        let git = Git::at(&request.root);
        self.plan_uninstall(request).finish(request, &git)
    }
}
