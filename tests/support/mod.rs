//! Shared helpers for integration tests that run `git` and the `reporinse` binary.
//!
//! Every [`Sandbox`] is a fresh temp directory whose path contains spaces and accents. Commands started through
//! it get an isolated environment: `HOME`, `XDG_CONFIG_HOME`, `APPDATA`, `REPORINSE_CONFIG_HOME`, a temp
//! `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM=1` and fixed identities; variables that would point Git at another
//! repository (`GIT_DIR`, `GIT_INDEX_FILE`…) are removed. The developer's real configuration is never read or
//! written.
//!
//! Usage from a test file: `mod support;` then `let sb = support::Sandbox::new(); let repo = sb.repo("r");`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Variables removed from every child so an outer Git process (or hook) cannot leak into the test.
const SCRUBBED_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_PREFIX",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_CEILING_DIRECTORIES",
    "GIT_TEMPLATE_DIR",
    "REPORINSE_HOOK_ACTIVE",
    "NO_COLOR",
];

/// Path of the `reporinse` binary built for these tests.
pub fn reporinse_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("reporinse")
}

pub struct Sandbox {
    dir: tempfile::TempDir,
    pub home: PathBuf,
    pub reporinse_home: PathBuf,
    pub gitconfig: PathBuf,
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let dir = tempfile::Builder::new()
            .prefix("reporinse test ñá ")
            .tempdir()
            .expect("temp dir");
        let home = dir.path().join("home dir");
        let reporinse_home = home.join("config").join("reporinse");
        std::fs::create_dir_all(&home).unwrap();
        let gitconfig = home.join("gitconfig global");
        std::fs::write(
            &gitconfig,
            "[user]\n\tname = Test User\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n\
             [core]\n\tautocrlf = false\n[commit]\n\tgpgsign = false\n",
        )
        .unwrap();
        Sandbox {
            dir,
            home,
            reporinse_home,
            gitconfig,
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Applies the isolated environment to `cmd`.
    pub fn isolate<'c>(&self, cmd: &'c mut Command) -> &'c mut Command {
        for var in SCRUBBED_ENV {
            cmd.env_remove(var);
        }
        cmd.env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env("APPDATA", self.home.join("appdata"))
            .env("CODEX_HOME", self.home.join(".codex"))
            .env("CLAUDE_CONFIG_DIR", self.home.join(".claude"))
            .env("REPORINSE_CONFIG_HOME", &self.reporinse_home)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test User")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test User")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("GIT_MERGE_AUTOEDIT", "no")
    }

    /// Runs `git` in `dir`; does not assert success.
    pub fn try_git(&self, dir: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new("git");
        cmd.current_dir(dir).args(args);
        self.isolate(&mut cmd).output().expect("run git")
    }

    /// Runs `git` in `dir`, asserts success and returns trimmed stdout.
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        let out = self.try_git(dir, args);
        assert!(
            out.status.success(),
            "git {args:?} failed in {}:\n{}{}",
            dir.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// Runs the `reporinse` binary in `dir`.
    pub fn reporinse(&self, dir: &Path, args: &[&str]) -> Output {
        self.reporinse_with(&reporinse_bin(), dir, args)
    }

    /// Runs a specific copy of the binary (e.g. one that a test deletes afterwards).
    pub fn reporinse_with(&self, bin: &Path, dir: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new(bin);
        cmd.current_dir(dir).args(args);
        self.isolate(&mut cmd).output().expect("run reporinse")
    }

    /// `git init` a new repository at `<sandbox>/<name>` (name may contain spaces/Unicode).
    pub fn repo(&self, name: &str) -> TestRepo<'_> {
        let dir = self.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        self.git(&dir, &["init", "-q"]);
        TestRepo { sb: self, dir }
    }

    /// Value of a global config key in the isolated global file.
    pub fn global_config(&self, key: &str) -> Option<String> {
        let out = self.try_git(self.path(), &["config", "--global", "--get", key]);
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Sandbox::new()
    }
}

pub struct TestRepo<'s> {
    pub sb: &'s Sandbox,
    pub dir: PathBuf,
}

impl TestRepo<'_> {
    pub fn git(&self, args: &[&str]) -> String {
        self.sb.git(&self.dir, args)
    }

    pub fn try_git(&self, args: &[&str]) -> Output {
        self.sb.try_git(&self.dir, args)
    }

    pub fn reporinse(&self, args: &[&str]) -> Output {
        self.sb.reporinse(&self.dir, args)
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    pub fn write(&self, rel: &str, content: impl AsRef<[u8]>) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Writes an executable hook script (LF endings) at `rel`.
    pub fn write_hook(&self, rel: &str, script: &str) {
        self.write(rel, script.replace("\r\n", "\n"));
        make_executable(&self.path(rel));
    }

    /// `git commit --allow-empty` with the exact message bytes (via `-F`, `--cleanup=verbatim` optional).
    pub fn commit(&self, message: &str, extra: &[&str]) -> Output {
        let file = self.sb.path().join(format!("msg-{}.txt", unique()));
        std::fs::write(&file, message).unwrap();
        let file_arg = file.to_string_lossy().to_string();
        let mut args = vec!["commit", "-q", "--allow-empty", "-F", file_arg.as_str()];
        args.extend_from_slice(extra);
        self.try_git(&args)
    }

    /// Like [`commit`](Self::commit) but asserts success.
    pub fn commit_ok(&self, message: &str, extra: &[&str]) {
        let out = self.commit(message, extra);
        assert!(
            out.status.success(),
            "commit failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Full message of the last commit, exactly as stored.
    pub fn last_message(&self) -> String {
        let out = self.try_git(&["cat-file", "commit", "HEAD"]);
        let raw = String::from_utf8_lossy(&out.stdout).to_string();
        raw.split_once("\n\n")
            .map(|(_, m)| m.to_string())
            .unwrap_or_default()
    }

    /// Absolute hooks directory Git uses.
    pub fn hooks_dir(&self) -> PathBuf {
        PathBuf::from(self.git(&["rev-parse", "--path-format=absolute", "--git-path", "hooks"]))
    }

    pub fn git_dir(&self) -> PathBuf {
        PathBuf::from(self.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]))
    }
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

#[cfg(unix)]
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) {}

/// Every file under `dir` (relative path → bytes), skipping Git's object store and logs that change on any
/// commit. Use it to assert that a directory is byte-for-byte what it was.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Asserts the process exited with `code`, showing its output otherwise.
pub fn assert_code(out: &Output, code: i32) {
    assert_eq!(
        out.status.code(),
        Some(code),
        "unexpected exit code\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Locate Git for Windows shell without requiring global PATH edits.
pub fn shell() -> Command {
    if cfg!(windows) {
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            for candidate in [dir.join("sh.exe"), dir.join("../bin/sh.exe")] {
                if candidate.is_file() {
                    return Command::new(candidate);
                }
            }
        }
    }
    Command::new("sh")
}
