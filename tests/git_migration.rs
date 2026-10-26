//! Migration from no-ai-coauthor: its real hook (tests/fixtures/git/no-ai-coauthor/commit-msg, a byte copy of
//! hooks/commit-msg) installed the way its install.sh does, then RepoRinse on top, then back.

mod support;

use support::{Sandbox, TestRepo, assert_code, make_executable, snapshot, stderr, stdout};

const AI_TRAILER: &str = "Co-Authored-By: Claude <noreply@anthropic.com>";
const OLD_HOOK: &[u8] = include_bytes!("fixtures/git/no-ai-coauthor/commit-msg");

/// What `install.sh --repo PATH` does: copy the hook into the effective hooks dir and make it executable.
fn old_install_repo(repo: &TestRepo<'_>) {
    let target = repo.hooks_dir().join("commit-msg");
    std::fs::write(&target, OLD_HOOK).unwrap();
    make_executable(&target);
}

fn install(repo: &TestRepo<'_>, scope: &str) -> std::process::Output {
    repo.reporinse(&[
        "install",
        "--provider",
        "git",
        "--scope",
        scope,
        "--only",
        "git.ai-attribution",
        "--yes",
    ])
}

#[test]
fn repo_install_on_top_of_no_ai_coauthor_and_back() {
    let sb = Sandbox::new();
    let repo = sb.repo("migración repo");
    old_install_repo(&repo);
    repo.commit_ok(&format!("old hook works\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "old hook works\n");
    let before = snapshot(&repo.git_dir().join("hooks"));

    let out = install(&repo, "repo");
    assert_code(&out, 0);
    assert!(
        stdout(&out).contains("no-ai-coauthor detected"),
        "{}",
        stdout(&out)
    );

    // Both run: the old hook first, then RepoRinse. Human co-authors survive.
    repo.commit_ok(
        &format!(
            "feat: migrated\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n\n{AI_TRAILER}\nCo-authored-by: Ana <ana@example.com>\n"
        ),
        &[],
    );
    assert_eq!(
        repo.last_message(),
        "feat: migrated\n\nCo-authored-by: Ana <ana@example.com>\n"
    );

    // Uninstall restores the old hook byte-for-byte, and it keeps working.
    assert_code(
        &repo.reporinse(&["uninstall", "--provider", "git", "--yes"]),
        0,
    );
    assert_eq!(snapshot(&repo.git_dir().join("hooks")), before);
    assert_eq!(
        std::fs::read(repo.hooks_dir().join("commit-msg")).unwrap(),
        OLD_HOOK
    );
    repo.commit_ok(&format!("old again\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "old again\n");
}

#[test]
fn retiring_the_old_hook_fixes_its_false_positive() {
    let sb = Sandbox::new();
    let repo = sb.repo("retire old");
    old_install_repo(&repo);
    let human = "feat: pair\n\nCo-authored-by: Claude Dupont <claude@dupont.fr>\n";

    // While chained, the old regex still drops the human named Claude (documented difference).
    assert_code(&install(&repo, "repo"), 0);
    repo.commit_ok(human, &[]);
    assert_eq!(repo.last_message(), "feat: pair\n");

    // Retire it: uninstall RepoRinse (restores the old hook), remove the old hook as its
    // `install.sh --uninstall --repo` does, install RepoRinse again.
    assert_code(
        &repo.reporinse(&["uninstall", "--provider", "git", "--yes"]),
        0,
    );
    std::fs::remove_file(repo.hooks_dir().join("commit-msg")).unwrap();
    assert_code(&install(&repo, "repo"), 0);
    repo.commit_ok(human, &[]);
    assert_eq!(repo.last_message(), human);
}

#[test]
fn global_no_ai_coauthor_install_is_reported_not_overridden() {
    let sb = Sandbox::new();
    let repo = sb.repo("global old");
    // What `install.sh` (global) does: ~/.config/git/hooks/commit-msg + global core.hooksPath.
    let old_dir = sb.home.join("config").join("git").join("hooks");
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::write(old_dir.join("commit-msg"), OLD_HOOK).unwrap();
    make_executable(&old_dir.join("commit-msg"));
    let value = old_dir.to_string_lossy().replace('\\', "/");
    sb.git(sb.path(), &["config", "--global", "core.hooksPath", &value]);

    let out = install(&repo, "user");
    assert_code(&out, 4);
    let err = stderr(&out);
    assert!(err.contains("no-ai-coauthor"), "{err}");
    assert!(err.contains("--uninstall"), "{err}");
    assert_eq!(
        sb.global_config("core.hooksPath").as_deref(),
        Some(value.as_str())
    );

    // After the old uninstall (`rm` the hook, unset core.hooksPath) RepoRinse installs cleanly.
    std::fs::remove_file(old_dir.join("commit-msg")).unwrap();
    sb.git(
        sb.path(),
        &["config", "--global", "--unset", "core.hooksPath"],
    );
    assert_code(&install(&repo, "user"), 0);
    repo.commit_ok(&format!("feat: user scope\n\n{AI_TRAILER}\n"), &[]);
    assert_eq!(repo.last_message(), "feat: user scope\n");
    assert_code(
        &repo.reporinse(&["uninstall", "--provider", "git", "--scope", "user", "--yes"]),
        0,
    );
    assert_eq!(sb.global_config("core.hooksPath"), None);
}
