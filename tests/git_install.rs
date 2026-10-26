//! Installer behaviour: scopes, core.hooksPath handling, manifest, idempotency, drift and CLI errors.

mod support;

use support::{Sandbox, TestRepo, assert_code, snapshot, stderr, stdout};

const AI_TRAILER: &str = "Co-Authored-By: Claude <noreply@anthropic.com>";

fn install_args<'a>(scope: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "install",
        "--provider",
        "git",
        "--scope",
        scope,
        "--only",
        "git.ai-attribution",
        "--yes",
    ];
    args.extend_from_slice(extra);
    args
}

fn uninstall_args<'a>(scope: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["uninstall", "--provider", "git", "--scope", scope, "--yes"];
    args.extend_from_slice(extra);
    args
}

fn assert_cleans(repo: &TestRepo<'_>) {
    repo.commit_ok(&format!("feat: x\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "feat: x\n");
}

fn manifest(repo: &TestRepo<'_>) -> serde_json::Value {
    let text = std::fs::read_to_string(repo.git_dir().join("reporinse").join("git-manifest.json"))
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn manifest_records_what_was_installed() {
    let sb = Sandbox::new();
    let repo = sb.repo("manifest");
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    let m = manifest(&repo);
    assert_eq!(m["format"], 1);
    assert_eq!(m["provider"], "git");
    assert_eq!(m["scope"], "repo");
    assert_eq!(m["modules"], serde_json::json!(["git.ai-attribution"]));
    assert_eq!(m["files"][0]["hook"], "commit-msg");
    assert_eq!(m["files"][0]["sha256"].as_str().unwrap().len(), 64);
    assert!(m["installed_at"].as_str().unwrap().ends_with('Z'));
    let shim = std::fs::read_to_string(repo.hooks_dir().join("commit-msg")).unwrap();
    assert!(
        shim.contains("reporinse_only='git.ai-attribution'"),
        "--only is passed to the hook"
    );
    assert!(!shim.contains('\r'));
}

#[test]
fn reinstall_is_idempotent() {
    let sb = Sandbox::new();
    let repo = sb.repo("idempotent");
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    let first = snapshot(&repo.git_dir());
    let out = repo.reporinse(&install_args("repo", &[]));
    assert_code(&out, 0);
    assert_eq!(
        snapshot(&repo.git_dir()),
        first,
        "second install changes nothing"
    );
    assert!(stdout(&out).contains("already up to date"));
    let hooks: Vec<_> = std::fs::read_dir(repo.hooks_dir())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.ends_with(".sample"))
        .collect();
    assert_eq!(
        hooks,
        vec!["commit-msg".to_string()],
        "no duplicate dispatchers"
    );
    assert_cleans(&repo);
}

#[test]
fn dry_run_writes_nothing() {
    let sb = Sandbox::new();
    let repo = sb.repo("dry run");
    let before = snapshot(&repo.git_dir());
    let out = repo.reporinse(&install_args("repo", &["--dry-run"]));
    assert_code(&out, 0);
    assert!(stdout(&out).contains("dry run"));
    assert!(stdout(&out).contains("commit-msg"));
    assert_eq!(snapshot(&repo.git_dir()), before);

    let global_before = std::fs::read(&sb.gitconfig).unwrap();
    assert_code(&repo.reporinse(&install_args("user", &["--dry-run"])), 0);
    assert_eq!(std::fs::read(&sb.gitconfig).unwrap(), global_before);
    assert!(!sb.reporinse_home.exists());

    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    let installed = snapshot(&repo.git_dir());
    assert_code(&repo.reporinse(&uninstall_args("repo", &["--dry-run"])), 0);
    assert_eq!(snapshot(&repo.git_dir()), installed);
}

#[test]
fn drift_is_detected_on_uninstall() {
    let sb = Sandbox::new();
    let repo = sb.repo("drift");
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    let hook = repo.hooks_dir().join("commit-msg");
    let mut text = std::fs::read_to_string(&hook).unwrap();
    text.push_str("# edited by a person\n");
    std::fs::write(&hook, &text).unwrap();

    let out = repo.reporinse(&uninstall_args("repo", &[]));
    assert_code(&out, 4);
    assert!(
        stderr(&out).contains("changed since RepoRinse installed it"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        std::fs::read_to_string(&hook).unwrap(),
        text,
        "nothing removed on conflict"
    );
    assert!(
        repo.git_dir()
            .join("reporinse")
            .join("git-manifest.json")
            .exists()
    );

    // Reinstall also refuses to overwrite it without --force.
    assert_code(&repo.reporinse(&install_args("repo", &[])), 4);

    let out = repo.reporinse(&uninstall_args("repo", &["--force"]));
    assert_code(&out, 0);
    assert!(!hook.exists());
    assert!(!repo.git_dir().join("reporinse").exists());
}

#[test]
fn deleted_hook_is_recreated_and_uninstall_tolerates_it() {
    let sb = Sandbox::new();
    let repo = sb.repo("deleted");
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    std::fs::remove_file(repo.hooks_dir().join("commit-msg")).unwrap();
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    assert_cleans(&repo);
    std::fs::remove_file(repo.hooks_dir().join("commit-msg")).unwrap();
    let out = repo.reporinse(&uninstall_args("repo", &[]));
    assert_code(&out, 0);
    assert!(stdout(&out).contains("already removed"));
}

#[test]
fn relative_core_hooks_path_is_honoured_and_kept() {
    let sb = Sandbox::new();
    let repo = sb.repo("relative hooks");
    repo.git(&["config", "core.hooksPath", "my hooks"]);
    std::fs::create_dir_all(repo.path("sub dir")).unwrap();
    // Install from a subdirectory: the relative path is still resolved from the worktree top.
    let out = repo
        .sb
        .reporinse(&repo.path("sub dir"), &install_args("repo", &[]));
    assert_code(&out, 0);
    assert!(repo.path("my hooks/commit-msg").is_file());
    assert!(!repo.path("sub dir/my hooks").exists());
    assert_eq!(repo.git(&["config", "core.hooksPath"]), "my hooks");
    assert_cleans(&repo);
    assert_code(&repo.reporinse(&uninstall_args("repo", &[])), 0);
    assert!(!repo.path("my hooks/commit-msg").exists());
    assert_eq!(repo.git(&["config", "core.hooksPath"]), "my hooks");
}

#[test]
fn absolute_core_hooks_path_is_honoured() {
    let sb = Sandbox::new();
    let repo = sb.repo("absolute hooks");
    let dir = sb.path().join("shared hooks ñ");
    std::fs::create_dir_all(&dir).unwrap();
    let value = dir.to_string_lossy().replace('\\', "/");
    repo.git(&["config", "core.hooksPath", &value]);
    assert_code(&repo.reporinse(&install_args("repo", &[])), 0);
    assert!(dir.join("commit-msg").is_file());
    assert_cleans(&repo);
    assert_code(&repo.reporinse(&uninstall_args("repo", &[])), 0);
    assert!(!dir.join("commit-msg").exists());
}

#[test]
fn versioned_hooks_are_never_modified() {
    let sb = Sandbox::new();
    let repo = sb.repo("tracked hooks");
    repo.write_hook(".githooks/commit-msg", "#!/bin/sh\nexit 0\n");
    repo.git(&["add", ".githooks/commit-msg"]);
    repo.commit_ok("add hooks\n", &[]);
    repo.git(&["config", "core.hooksPath", ".githooks"]);
    let before = snapshot(&repo.path(".githooks"));
    let out = repo.reporinse(&install_args("repo", &[]));
    assert_code(&out, 4);
    assert!(stderr(&out).contains("versioned"), "{}", stderr(&out));
    assert_eq!(snapshot(&repo.path(".githooks")), before);
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn linked_worktree_shares_the_hooks() {
    let sb = Sandbox::new();
    let repo = sb.repo("main wt");
    repo.commit_ok("base\n", &[]);
    let wt = sb.path().join("linked wt ñ");
    let wt_arg = wt.to_string_lossy().to_string();
    repo.git(&["worktree", "add", "-q", &wt_arg, "-b", "feature"]);
    // Install from the linked worktree; the manifest goes to the common dir.
    let out = sb.reporinse(&wt, &install_args("repo", &[]));
    assert_code(&out, 0);
    assert!(
        repo.git_dir()
            .join("reporinse")
            .join("git-manifest.json")
            .is_file()
    );
    let linked = TestRepo {
        sb: &sb,
        dir: wt.clone(),
    };
    assert_cleans(&linked);
    assert_cleans(&repo);
    assert_code(&sb.reporinse(&wt, &uninstall_args("repo", &[])), 0);
    assert!(!repo.git_dir().join("hooks").join("commit-msg").exists());
}

#[test]
fn bare_repository_is_refused() {
    let sb = Sandbox::new();
    let dir = sb.path().join("bare.git");
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "-q", "--bare"]);
    let out = sb.reporinse(&dir, &install_args("repo", &[]));
    assert_code(&out, 4);
    assert!(stderr(&out).contains("bare repository"));
    assert!(!dir.join("hooks").join("commit-msg").exists());
}

#[test]
fn not_a_repository_is_a_clear_conflict() {
    let sb = Sandbox::new();
    let dir = sb.path().join("plain dir");
    std::fs::create_dir_all(&dir).unwrap();
    let out = sb.reporinse(&dir, &install_args("repo", &[]));
    assert_code(&out, 4);
    assert!(stderr(&out).contains("not inside a Git repository"));
}

#[test]
fn user_scope_installs_globally_and_keeps_repo_hooks_running() {
    let sb = Sandbox::new();
    let repo = sb.repo("user scope");
    // A repository-local hook that a global core.hooksPath would normally hide.
    repo.write_hook(
        ".git/hooks/commit-msg",
        "#!/bin/sh\necho local-hook-ran >> \"$(git rev-parse --git-dir)/local-ran.txt\"\n",
    );
    let global_before = std::fs::read(&sb.gitconfig).unwrap();

    let out = repo.reporinse(&install_args("user", &[]));
    assert_code(&out, 0);
    let hooks = sb.reporinse_home.join("git-hooks");
    assert!(hooks.join("commit-msg").is_file());
    assert!(hooks.join(".reporinse-manifest.json").is_file());
    let configured = sb.global_config("core.hooksPath").unwrap();
    assert_eq!(configured, hooks.to_string_lossy().replace('\\', "/"));

    assert_cleans(&repo);
    let ran = std::fs::read_to_string(repo.git_dir().join("local-ran.txt")).unwrap();
    assert_eq!(ran.trim(), "local-hook-ran");

    // Idempotent at user scope too.
    let snap = snapshot(&hooks);
    assert_code(&repo.reporinse(&install_args("user", &[])), 0);
    assert_eq!(snapshot(&hooks), snap);

    // A repository that installs repo scope under an active user install gets a clear conflict.
    let other = sb.repo("other repo");
    let out = other.reporinse(&install_args("repo", &[]));
    assert_code(&out, 4);
    assert!(
        stderr(&out).contains("already installed for the user"),
        "{}",
        stderr(&out)
    );

    assert_code(&repo.reporinse(&uninstall_args("user", &[])), 0);
    assert_eq!(
        std::fs::read(&sb.gitconfig).unwrap(),
        global_before,
        "global config restored exactly"
    );
    assert!(!hooks.exists());
    assert!(
        repo.path(".git/hooks/commit-msg").is_file(),
        "repo hook untouched"
    );
}

#[test]
fn global_hooks_path_conflict_exits_4() {
    let sb = Sandbox::new();
    let repo = sb.repo("global conflict");
    let corp = sb.path().join("corporate hooks");
    std::fs::create_dir_all(&corp).unwrap();
    sb.git(
        sb.path(),
        &[
            "config",
            "--global",
            "core.hooksPath",
            &corp.to_string_lossy(),
        ],
    );
    let global_before = std::fs::read(&sb.gitconfig).unwrap();

    let out = repo.reporinse(&install_args("global", &[]));
    assert_code(&out, 4);
    let err = stderr(&out);
    assert!(err.contains("never overrides"), "{err}");
    assert!(err.contains("Options"), "{err}");
    assert_eq!(std::fs::read(&sb.gitconfig).unwrap(), global_before);
    assert!(!sb.reporinse_home.join("git-hooks").exists());

    // Repo scope would write into the shared directory: refused as well.
    let out = repo.reporinse(&install_args("repo", &[]));
    assert_code(&out, 4);
    assert!(stderr(&out).contains("affect every"));
    assert!(!corp.join("commit-msg").exists());
}

#[test]
fn only_selection_and_partial_uninstall() {
    let sb = Sandbox::new();
    let repo = sb.repo("only");
    // --all with everything excepted is an empty selection.
    let out = repo.reporinse(&[
        "install",
        "--provider",
        "git",
        "--all",
        "--except",
        "git.ai-attribution",
        "--yes",
    ]);
    assert_code(&out, 2);
    assert_code(
        &repo.reporinse(&["install", "--provider", "git", "--all", "--yes"]),
        0,
    );
    assert!(
        manifest(&repo)["modules"]
            .as_array()
            .unwrap()
            .contains(&"git.ai-attribution".into())
    );

    // Removing the only module that needs commit-msg removes the whole installation.
    let out = repo.reporinse(&[
        "uninstall",
        "--provider",
        "git",
        "--only",
        "git.ai-attribution",
        "--yes",
    ]);
    assert_code(&out, 0);
    assert!(!repo.hooks_dir().join("commit-msg").exists());
    assert!(!repo.git_dir().join("reporinse").exists());
}

#[test]
fn cli_errors_have_documented_exit_codes() {
    let sb = Sandbox::new();
    let repo = sb.repo("cli errors");
    let out = repo.reporinse(&["install", "--provider", "git", "--only", "nope.rule"]);
    assert_code(&out, 2);
    assert!(stderr(&out).contains("unknown module 'nope.rule'"));
    let out = repo.reporinse(&[
        "install",
        "--provider",
        "claude",
        "--only",
        "git.ai-attribution",
    ]);
    assert_code(&out, 69);
    assert!(stderr(&out).contains("not supported by this build yet"));
    assert_code(&repo.reporinse(&["install", "--provider", "nothing"]), 2);
    assert_code(&repo.reporinse(&["install", "--scope", "planet"]), 2);
    // Tests run without a TTY on stdin.
    let out = repo.reporinse(&["install", "--interactive"]);
    assert_code(&out, 2);
    assert!(stderr(&out).contains("needs a terminal"));
    let out = repo.reporinse(&["uninstall", "--force"]);
    assert_code(&out, 2);
    // Uninstall with nothing installed is fine.
    let out = repo.reporinse(&["uninstall", "--provider", "git", "--yes"]);
    assert_code(&out, 0);
    assert!(stdout(&out).contains("not installed"));
}

#[test]
fn json_output_is_the_install_result() {
    let sb = Sandbox::new();
    let repo = sb.repo("json");
    let mut args = install_args("repo", &["--dry-run"]);
    args.push("--json");
    let out = repo.reporinse(&args);
    assert_code(&out, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["conflicts"], serde_json::json!([]));
    assert!(
        v["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["kind"] == "create-file")
    );
}
