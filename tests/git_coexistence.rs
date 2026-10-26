//! Spec case E: RepoRinse coexists with hooks that were there before (plain, Husky, pre-commit, Lefthook),
//! both run, a foreign failure stops the commit, and uninstall restores everything byte-for-byte.

mod support;

use support::{Sandbox, TestRepo, assert_code, snapshot, stderr, stdout};

const AI_TRAILER: &str = "Co-Authored-By: Claude <noreply@anthropic.com>";

fn install(repo: &TestRepo<'_>) -> std::process::Output {
    repo.reporinse(&[
        "install",
        "--provider",
        "git",
        "--scope",
        "repo",
        "--only",
        "git.ai-attribution",
        "--yes",
    ])
}

fn uninstall(repo: &TestRepo<'_>) -> std::process::Output {
    repo.reporinse(&["uninstall", "--provider", "git", "--scope", "repo", "--yes"])
}

/// A foreign commit-msg hook that records its arguments and appends a sign-off.
const FOREIGN: &str = "#!/bin/sh\n\
printf '%s\\n' \"foreign ran with $# arg(s)\" >> \"$(git rev-parse --git-dir)/foreign-ran.txt\"\n\
printf 'Signed-off-by: Foreign Tool <tool@example.com>\\n' >> \"$1\"\n";

#[test]
fn plain_foreign_hook_is_chained_and_restored_exactly() {
    let sb = Sandbox::new();
    let repo = sb.repo("foreign hook");
    repo.write_hook(".git/hooks/commit-msg", FOREIGN);
    let before = snapshot(&repo.git_dir().join("hooks"));

    let out = install(&repo);
    assert_code(&out, 0);
    assert!(
        repo.hooks_dir()
            .join("commit-msg.reporinse-chained")
            .is_file()
    );

    repo.commit_ok(&format!("feat: both\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(
        repo.last_message(),
        "feat: both\n\nSigned-off-by: Foreign Tool <tool@example.com>\n",
        "foreign hook ran first, then RepoRinse removed only the AI trailer"
    );
    let ran = std::fs::read_to_string(repo.git_dir().join("foreign-ran.txt")).unwrap();
    assert_eq!(
        ran, "foreign ran with 1 arg(s)\n",
        "same arguments, ran exactly once"
    );

    let m: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo.git_dir().join("reporinse/git-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(m["chained"][0]["stored_as"], "commit-msg.reporinse-chained");
    assert_eq!(m["chained"][0]["sha256"].as_str().unwrap().len(), 64);

    // Reinstall does not chain our own dispatcher a second time.
    assert_code(&install(&repo), 0);
    assert!(
        !repo
            .hooks_dir()
            .join("commit-msg.reporinse-chained.reporinse-chained")
            .exists()
    );

    assert_code(&uninstall(&repo), 0);
    assert_eq!(
        snapshot(&repo.git_dir().join("hooks")),
        before,
        "byte-for-byte restore"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(repo.hooks_dir().join("commit-msg"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "mode restored");
    }
}

#[test]
fn foreign_failure_propagates_and_reporinse_does_not_run() {
    let sb = Sandbox::new();
    let repo = sb.repo("foreign fails");
    repo.commit_ok("base\n", &[]);
    repo.write_hook(
        ".git/hooks/commit-msg",
        "#!/bin/sh\necho 'commitlint: subject must be lower case' >&2\nexit 3\n",
    );
    assert_code(&install(&repo), 0);
    let out = repo.commit(&format!("BAD subject\n\n{AI_TRAILER}\n"), &[]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("commitlint: subject must be lower case"),
        "{err}"
    );
    assert!(
        !err.contains("reporinse: removed"),
        "RepoRinse must not run after a foreign failure: {err}"
    );
    assert_eq!(
        repo.git(&["rev-list", "--count", "HEAD"]),
        "1",
        "no commit created"
    );

    // The exit code itself is propagated by the dispatcher.
    let file = repo.path("msg.txt");
    std::fs::write(&file, "x\n").unwrap();
    let mut cmd = std::process::Command::new("sh");
    cmd.current_dir(&repo.dir)
        .arg(repo.hooks_dir().join("commit-msg"))
        .arg(&file);
    let out = sb.isolate(&mut cmd).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
}

/// Husky v9 layout: core.hooksPath=.husky/_, generated `_/<hook>` scripts source `_/h`, which finds the
/// user's `.husky/<hook>` from "$0".
fn husky_repo(sb: &Sandbox) -> TestRepo<'_> {
    let repo = sb.repo("husky project");
    repo.write(".husky/_/.gitignore", "*\n");
    repo.write_hook(
        ".husky/_/h",
        "#!/usr/bin/env sh\nn=$(basename \"$0\")\ns=$(dirname \"$(dirname \"$0\")\")/$n\n[ ! -f \"$s\" ] && exit 0\nsh -e \"$s\" \"$@\"\nc=$?\n[ $c != 0 ] && echo \"husky - $n script failed (code $c)\"\nexit $c\n",
    );
    repo.write_hook(
        ".husky/_/commit-msg",
        "#!/usr/bin/env sh\n. \"$(dirname \"$0\")/h\"\n",
    );
    repo.write_hook(
        ".husky/commit-msg",
        "echo husky-user-hook >> \"$(git rev-parse --git-dir)/husky-ran.txt\"\n",
    );
    repo.write("package.json", "{\"name\":\"demo\"}\n");
    repo.git(&["add", ".husky/commit-msg", "package.json"]);
    repo.git(&["config", "core.hooksPath", ".husky/_"]);
    repo.commit_ok("chore: husky\n", &[]);
    std::fs::remove_file(repo.git_dir().join("husky-ran.txt")).unwrap();
    repo
}

#[test]
fn husky_v9_layout_keeps_working() {
    let sb = Sandbox::new();
    let repo = husky_repo(&sb);
    let before = snapshot(&repo.path(".husky"));
    let tracked_before = repo.git(&["status", "--porcelain"]);

    let out = install(&repo);
    assert_code(&out, 0);
    assert!(stdout(&out).contains("husky detected"), "{}", stdout(&out));
    assert!(repo.path(".husky/_/commit-msg.reporinse-chained").is_file());
    assert_eq!(
        repo.git(&["config", "core.hooksPath"]),
        ".husky/_",
        "core.hooksPath untouched"
    );

    repo.commit_ok(&format!("feat: husky and reporinse\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "feat: husky and reporinse\n");
    let ran = std::fs::read_to_string(repo.git_dir().join("husky-ran.txt")).unwrap();
    assert_eq!(
        ran, "husky-user-hook\n",
        "the user's .husky/commit-msg ran once through Husky's dispatcher"
    );
    assert_eq!(
        repo.git(&["status", "--porcelain"]),
        tracked_before,
        "no tracked file changed"
    );

    assert_code(&uninstall(&repo), 0);
    assert_eq!(snapshot(&repo.path(".husky")), before);
    assert_eq!(repo.git(&["config", "core.hooksPath"]), ".husky/_");
}

#[test]
fn pre_commit_framework_style_hook_is_chained() {
    let sb = Sandbox::new();
    let repo = sb.repo("pre-commit framework");
    repo.write(".pre-commit-config.yaml", "repos: []\n");
    // Shape of the script `pre-commit install --hook-type commit-msg` writes (bash, uses dirname "$0").
    let script = "#!/usr/bin/env bash\n# File generated by pre-commit: https://pre-commit.com\n\
ARGS=(hook-impl --hook-type=commit-msg)\nHERE=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
echo \"pre-commit ran ${ARGS[1]} from $(basename \"$0\") in ${HERE##*/}\" >> \"$(git rev-parse --git-dir)/pc-ran.txt\"\n";
    repo.write_hook(".git/hooks/commit-msg", script);
    let before = snapshot(&repo.git_dir().join("hooks"));

    let out = install(&repo);
    assert_code(&out, 0);
    assert!(
        stdout(&out).contains("pre-commit detected"),
        "{}",
        stdout(&out)
    );
    repo.commit_ok(&format!("feat: pc\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "feat: pc\n");
    let ran = std::fs::read_to_string(repo.git_dir().join("pc-ran.txt")).unwrap();
    assert_eq!(
        ran, "pre-commit ran --hook-type=commit-msg from commit-msg in hooks\n",
        "$0 is the original path"
    );

    assert_code(&uninstall(&repo), 0);
    assert_eq!(snapshot(&repo.git_dir().join("hooks")), before);
}

#[test]
fn lefthook_style_hook_is_chained() {
    let sb = Sandbox::new();
    let repo = sb.repo("lefthook");
    repo.write("lefthook.yml", "commit-msg:\n  commands: {}\n");
    repo.write_hook(
        ".git/hooks/commit-msg",
        "#!/bin/sh\n# lefthook generated hook\ncall_lefthook() { echo lefthook-ran >> \"$(git rev-parse --git-dir)/lh.txt\"; }\ncall_lefthook run \"commit-msg\" \"$@\"\n",
    );
    let before = snapshot(&repo.git_dir().join("hooks"));
    let out = install(&repo);
    assert_code(&out, 0);
    assert!(stdout(&out).contains("lefthook detected"));
    repo.commit_ok(&format!("feat: lh\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "feat: lh\n");
    assert_eq!(
        std::fs::read_to_string(repo.git_dir().join("lh.txt")).unwrap(),
        "lefthook-ran\n"
    );
    assert_code(&uninstall(&repo), 0);
    assert_eq!(snapshot(&repo.git_dir().join("hooks")), before);
}

#[test]
fn drifted_chained_hook_blocks_uninstall() {
    let sb = Sandbox::new();
    let repo = sb.repo("chained drift");
    repo.write_hook(".git/hooks/commit-msg", FOREIGN);
    assert_code(&install(&repo), 0);
    let chained = repo.hooks_dir().join("commit-msg.reporinse-chained");
    std::fs::write(&chained, "#!/bin/sh\necho changed\n").unwrap();
    let out = uninstall(&repo);
    assert_code(&out, 4);
    assert!(
        stderr(&out).contains("restore it by hand"),
        "{}",
        stderr(&out)
    );
    assert!(chained.exists());
    assert!(
        repo.hooks_dir().join("commit-msg").exists(),
        "nothing removed on conflict"
    );
}
