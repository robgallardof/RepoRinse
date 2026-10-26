//! End-to-end: real `git commit` through installed RepoRinse hooks, in isolated temp repositories.

mod support;

use support::{Sandbox, TestRepo, assert_code, snapshot, stderr};

const AI_TRAILER: &str = "Co-Authored-By: Claude <noreply@anthropic.com>";
const FOOTER: &str = "🤖 Generated with [Claude Code](https://claude.com/claude-code)";

fn install(repo: &TestRepo<'_>, extra: &[&str]) {
    let mut args = vec!["install", "--provider", "git", "--scope", "repo", "--yes"];
    args.extend_from_slice(extra);
    let out = repo.reporinse(&args);
    assert_code(&out, 0);
}

fn install_only(repo: &TestRepo<'_>) {
    install(repo, &["--only", "git.ai-attribution"]);
}

#[test]
fn case_a_ai_removed_human_kept_and_uninstall_leaves_nothing() {
    let sb = Sandbox::new();
    let repo = sb.repo("case a");
    repo.commit_ok("initial\n", &[]);
    let before = snapshot(&repo.git_dir().join("hooks"));
    let config_before = std::fs::read(repo.git_dir().join("config")).unwrap();

    install_only(&repo);
    assert!(repo.hooks_dir().join("commit-msg").is_file());
    assert!(
        !repo.hooks_dir().join("pre-commit").exists(),
        "--only installs only what it needs"
    );

    repo.commit_ok(
        &format!("feat: login\n\nBody text.\n\n{FOOTER}\n\n{AI_TRAILER}\nCo-authored-by: Ana Pérez <ana@example.com>\n"),
        &[],
    );
    assert_eq!(
        repo.last_message(),
        "feat: login\n\nBody text.\n\nCo-authored-by: Ana Pérez <ana@example.com>\n"
    );
    // Author/committer untouched.
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae>|%cn <%ce>"]),
        "Test User <test@example.com>|Test User <test@example.com>"
    );

    let out = repo.reporinse(&["uninstall", "--provider", "git", "--scope", "repo", "--yes"]);
    assert_code(&out, 0);
    assert_eq!(snapshot(&repo.git_dir().join("hooks")), before);
    assert!(!repo.git_dir().join("reporinse").exists());
    assert_eq!(
        std::fs::read(repo.git_dir().join("config")).unwrap(),
        config_before
    );

    // After uninstall the trailer is no longer removed.
    repo.commit_ok(&format!("chore: x\n\n{AI_TRAILER}\n"), &[]);
    assert!(repo.last_message().contains(AI_TRAILER));
}

#[test]
fn ported_no_ai_coauthor_cases() {
    // The four cases of no-ai-coauthor's test.sh, through real commits.
    let sb = Sandbox::new();
    let repo = sb.repo("ported");
    install_only(&repo);
    let cases = [
        (
            format!("feat: login\n\n{FOOTER}\n\n{AI_TRAILER}"),
            "feat: login\n",
        ),
        (
            "fix: typo\n\nCo-authored-by: Copilot <copilot@github.com>\nCo-authored-by: Ana <ana@example.com>".to_string(),
            "fix: typo\n\nCo-authored-by: Ana <ana@example.com>\n",
        ),
        (
            "docs: mention Claude in the README".to_string(),
            "docs: mention Claude in the README\n",
        ),
        (
            format!(
                "fix: strip the \"🤖 Generated with Claude Code\" line\n\nThe line \"Generated with [Claude Code](...)\" stayed.\n\nGenerated with Claude Code.\n\n{AI_TRAILER}"
            ),
            "fix: strip the \"🤖 Generated with Claude Code\" line\n\nThe line \"Generated with [Claude Code](...)\" stayed.\n",
        ),
    ];
    for (message, expected) in cases {
        repo.commit_ok(&message, &[]);
        assert_eq!(repo.last_message(), expected, "for message {message:?}");
    }
}

#[test]
fn human_named_claude_is_kept() {
    let sb = Sandbox::new();
    let repo = sb.repo("claude human");
    install_only(&repo);
    let msg = "feat: x\n\nCo-authored-by: Claude Dupont <claude@dupont.fr>\nCo-authored-by: Claude <claude@dupont.fr>\n";
    repo.commit_ok(msg, &[]);
    assert_eq!(repo.last_message(), msg);
}

#[test]
fn amend_and_merge_messages_are_cleaned() {
    let sb = Sandbox::new();
    let repo = sb.repo("amend merge");
    install_only(&repo);
    repo.commit_ok("base\n", &[]);
    repo.commit_ok(&format!("amended\n\n{AI_TRAILER}\n"), &["--amend"]);
    assert_eq!(repo.last_message(), "amended\n");
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "1");

    repo.git(&["checkout", "-q", "-b", "topic"]);
    repo.commit_ok("topic work\n", &[]);
    repo.git(&["checkout", "-q", "main"]);
    repo.commit_ok("main work\n", &[]);
    repo.git(&[
        "merge",
        "--no-ff",
        "-q",
        "-m",
        "Merge topic",
        "-m",
        AI_TRAILER,
        "topic",
    ]);
    assert_eq!(repo.git(&["rev-list", "--count", "--merges", "HEAD"]), "1");
    assert_eq!(repo.last_message(), "Merge topic\n");
}

#[test]
fn no_verify_skips_the_hook() {
    let sb = Sandbox::new();
    let repo = sb.repo("no verify");
    install_only(&repo);
    repo.commit_ok(&format!("x\n\n{AI_TRAILER}\n"), &["--no-verify"]);
    assert!(
        repo.last_message().contains(AI_TRAILER),
        "--no-verify is honoured (documented)"
    );
}

#[test]
fn crlf_and_unicode_messages_keep_their_bytes() {
    let sb = Sandbox::new();
    let repo = sb.repo("crlf ünïcödé");
    install_only(&repo);
    let msg =
        format!("feat: ñandú 🦤 日本語\r\n\r\nCuerpo\u{200d} con acentos\r\n\r\n{AI_TRAILER}\r\n");
    repo.commit_ok(&msg, &["--cleanup=verbatim"]);
    assert_eq!(
        repo.last_message(),
        "feat: ñandú 🦤 日本語\r\n\r\nCuerpo\u{200d} con acentos\r\n"
    );

    // The hook command on a file directly: CRLF + BOM preserved, idempotent.
    let file = repo.path("MSG FILE ñ.txt");
    std::fs::write(&file, format!("\u{feff}subject\r\n\r\n{AI_TRAILER}\r\n")).unwrap();
    let path = file.to_string_lossy().to_string();
    assert_code(&repo.reporinse(&["hook", "git", "commit-msg", &path]), 0);
    assert_eq!(
        std::fs::read(&file).unwrap(),
        "\u{feff}subject\r\n".as_bytes()
    );
    assert_code(&repo.reporinse(&["hook", "git", "commit-msg", &path]), 0);
    assert_eq!(
        std::fs::read(&file).unwrap(),
        "\u{feff}subject\r\n".as_bytes()
    );
}

#[test]
fn comments_are_left_to_git() {
    let sb = Sandbox::new();
    let repo = sb.repo("comments");
    let file = repo.path("COMMIT_EDITMSG");
    let msg =
        format!("subject\n\n{AI_TRAILER}\n# Please enter the commit message\n# {AI_TRAILER}\n");
    std::fs::write(&file, &msg).unwrap();
    let path = file.to_string_lossy().to_string();
    assert_code(&repo.reporinse(&["hook", "git", "commit-msg", &path]), 0);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        format!("subject\n# Please enter the commit message\n# {AI_TRAILER}\n")
    );

    // core.commentChar is honoured: ';' lines are comments, '#' lines are text.
    repo.git(&["config", "core.commentChar", ";"]);
    let msg = format!("subject\n\n#hashtag body\n; {AI_TRAILER}\n");
    std::fs::write(&file, &msg).unwrap();
    assert_code(&repo.reporinse(&["hook", "git", "commit-msg", &path]), 0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), msg);
}

#[test]
fn unknown_only_ids_never_break_a_commit() {
    let sb = Sandbox::new();
    let repo = sb.repo("only flag");
    let file = repo.path("msg.txt");
    std::fs::write(&file, format!("s\n\n{AI_TRAILER}\n")).unwrap();
    let path = file.to_string_lossy().to_string();
    let out = repo.reporinse(&["hook", "--only", "nope.rule", "git", "commit-msg", &path]);
    assert_code(&out, 0);
    assert!(stderr(&out).contains("unknown module 'nope.rule'"));
}

#[test]
fn missing_binary_fails_open() {
    let sb = Sandbox::new();
    let repo = sb.repo("missing bin");
    // Install with a copy of the binary, then delete the copy.
    let bin_dir = sb.path().join("bin copy ñ");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let name = if cfg!(windows) {
        "reporinse.exe"
    } else {
        "reporinse"
    };
    let copy = bin_dir.join(name);
    std::fs::copy(support::reporinse_bin(), &copy).unwrap();
    let out = sb.reporinse_with(
        &copy,
        &repo.dir,
        &[
            "install",
            "--provider",
            "git",
            "--only",
            "git.ai-attribution",
            "--yes",
        ],
    );
    assert_code(&out, 0);
    let shim = std::fs::read_to_string(repo.hooks_dir().join("commit-msg")).unwrap();
    assert!(
        shim.contains("bin copy ñ"),
        "absolute path recorded: {shim}"
    );
    std::fs::remove_file(&copy).unwrap();

    let out = repo.commit(&format!("x\n\n{AI_TRAILER}\n"), &[]);
    assert_code(&out, 0);
    // Only meaningful when no other `reporinse` is on the developer's PATH.
    let on_path = std::process::Command::new("reporinse")
        .arg("--version")
        .output()
        .is_ok();
    if !on_path {
        assert!(
            stderr(&out).contains("binary not found"),
            "{}",
            stderr(&out)
        );
        assert!(repo.last_message().contains(AI_TRAILER));
    }
}

#[test]
fn hook_for_other_providers_is_a_clear_no_op() {
    let sb = Sandbox::new();
    let out = sb.reporinse(sb.path(), &["hook", "claude", "PreToolUse"]);
    assert_code(&out, 0);
    assert!(stderr(&out).contains("not supported by this build yet"));
    let out = sb.reporinse(sb.path(), &["hook", "git", "pre-push", "origin", "url"]);
    assert_code(&out, 0);
}

#[test]
fn pre_commit_hook_reads_index_and_changes_nothing() {
    let sb = Sandbox::new();
    let repo = sb.repo("pre commit");
    repo.write("a file.txt", "staged  \n");
    repo.git(&["add", "a file.txt"]);
    repo.write("a file.txt", "unstaged change\n");
    repo.write("bin.dat", [0u8, 1, 2, 3]);
    repo.git(&["add", "bin.dat"]);
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    let out = repo.reporinse(&["hook", "git", "pre-commit"]);
    assert_code(&out, 0);
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("a file.txt")).unwrap(),
        "unstaged change\n"
    );
    assert_eq!(repo.git(&["show", ":a file.txt"]), "staged");
}

#[test]
fn pre_commit_shim_runs_through_git_and_chains() {
    use reporinse::core::integration::InstallScope;
    use reporinse::integrations::git::shim::{ShimSpec, render};

    let sb = Sandbox::new();
    let repo = sb.repo("pre commit shim");
    // A foreign pre-commit (already chained) that fails when a file named STOP is staged.
    repo.write_hook(
        ".git/hooks/pre-commit.reporinse-chained",
        "#!/bin/sh\nif git diff --cached --name-only | grep -q STOP; then echo 'foreign: STOP staged' >&2; exit 5; fi\n",
    );
    let bin = support::reporinse_bin()
        .to_string_lossy()
        .replace('\\', "/");
    let modules = vec!["git.ai-attribution".to_string()];
    let shim = render(&ShimSpec {
        hook: "pre-commit",
        scope: InstallScope::Repo,
        binary: &bin,
        modules: &modules,
    });
    repo.write_hook(".git/hooks/pre-commit", &shim);

    repo.write("ok.txt", "fine\n");
    repo.git(&["add", "ok.txt"]);
    repo.commit_ok("feat: ok\n", &[]);
    assert_eq!(repo.git(&["status", "--porcelain"]), "");

    repo.write("STOP", "x\n");
    repo.git(&["add", "STOP"]);
    let out = repo.commit("feat: blocked\n", &[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("foreign: STOP staged"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        repo.git(&["status", "--porcelain"]),
        "A  STOP",
        "index left as it was"
    );

    // The dispatcher returns the foreign exit code itself.
    let mut cmd = std::process::Command::new("sh");
    cmd.current_dir(&repo.dir)
        .arg(repo.hooks_dir().join("pre-commit"));
    assert_eq!(
        sb.isolate(&mut cmd).output().unwrap().status.code(),
        Some(5)
    );
}
