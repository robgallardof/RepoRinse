mod support;
use support::*;
#[test]
fn staged_worktree_rollback_keeps_index_and_original_files() {
    let sb = Sandbox::new();
    let repo = sb.repo("rollback");
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"text.whitespace\"]\nmode=\"fix\"\n",
    );
    repo.write("a.txt", "a \n");
    repo.write("z.txt", "z \n");
    repo.git(&["add", "a.txt", "z.txt"]);
    let index = std::fs::read(repo.git_dir().join("index")).unwrap();
    let path = repo.path("z.txt");
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    let original_permissions = permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let output = repo.reporinse(&["fix", "--staged"]);
    std::fs::set_permissions(path, original_permissions).unwrap();
    assert_code(&output, 4);
    assert_eq!(std::fs::read_to_string(repo.path("a.txt")).unwrap(), "a \n");
    assert_eq!(std::fs::read_to_string(repo.path("z.txt")).unwrap(), "z \n");
    assert_eq!(std::fs::read(repo.git_dir().join("index")).unwrap(), index);
    assert!(!repo.git_dir().join("index.lock").exists());
}
#[test]
fn multi_provider_preflight_conflict_writes_nothing() {
    let sb = Sandbox::new();
    let repo = sb.repo("preflight");
    repo.write(".claude/settings.json", "invalid JSON");
    let output = repo.reporinse(&["install", "--provider", "git,claude", "--all", "--yes"]);
    assert_code(&output, 4);
    assert!(!repo.hooks_dir().join("commit-msg").exists());
    assert!(!repo.path(".claude/reporinse-manifest.json").exists());
    assert_eq!(
        std::fs::read_to_string(repo.path(".claude/settings.json")).unwrap(),
        "invalid JSON"
    );
}
#[cfg(unix)]
#[test]
fn links_cannot_redirect_fixes_or_provider_installation() {
    use std::os::unix::fs::symlink;
    let sb = Sandbox::new();
    let repo = sb.repo("links");
    let outside = sb.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("a.txt"), "keep \n").unwrap();
    symlink(&outside, repo.path("linked")).unwrap();
    assert_code(&repo.reporinse(&["fix", "--files", "linked/a.txt"]), 2);
    symlink(&outside, repo.path(".claude")).unwrap();
    assert_code(
        &repo.reporinse(&["install", "--provider", "claude", "--all", "--yes"]),
        4,
    );
    assert!(!outside.join("settings.json").exists());
    assert_eq!(
        std::fs::read_to_string(outside.join("a.txt")).unwrap(),
        "keep \n"
    );
}
