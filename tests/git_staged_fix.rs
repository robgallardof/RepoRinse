mod support;
use support::*;
fn config(repo: &TestRepo<'_>) {
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"text.whitespace\"]\nmode=\"fix\"\n",
    );
}
#[test]
fn staged_fix_changes_only_selected_index_entries_and_is_idempotent() {
    let sb = Sandbox::new();
    let repo = sb.repo("staged fixes");
    config(&repo);
    repo.write("a.txt", b"\xef\xbb\xbfx \r\n");
    repo.write("other.txt", "keep\n");
    repo.git(&["add", "a.txt", "other.txt"]);
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_code(&repo.reporinse(&["fix", "--staged", "--dry-run"]), 0);
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
    assert_code(&repo.reporinse(&["fix", "--staged"]), 0);
    assert_eq!(
        std::fs::read(repo.path("a.txt")).unwrap(),
        b"\xef\xbb\xbfx\r\n"
    );
    assert_eq!(repo.git(&["show", ":a.txt"]), "\u{feff}x");
    assert_eq!(repo.git(&["show", ":other.txt"]), "keep");
    assert_code(&repo.reporinse(&["fix", "--staged"]), 0);
    assert!(!repo.git_dir().join("index.lock").exists());
}
#[test]
fn partial_staging_and_index_locks_refuse_without_changes() {
    let sb = Sandbox::new();
    let repo = sb.repo("partial");
    config(&repo);
    repo.write("a.txt", "stage \n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "working \n");
    let index = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_code(&repo.reporinse(&["fix", "--staged"]), 4);
    assert_eq!(std::fs::read(repo.git_dir().join("index")).unwrap(), index);
    assert_eq!(
        std::fs::read_to_string(repo.path("a.txt")).unwrap(),
        "working \n"
    );
    repo.write("a.txt", "stage \n");
    repo.write(".git/index.lock", "foreign lock");
    assert_code(&repo.reporinse(&["fix", "--staged"]), 4);
    assert_eq!(
        std::fs::read_to_string(repo.git_dir().join("index.lock")).unwrap(),
        "foreign lock"
    );
}
