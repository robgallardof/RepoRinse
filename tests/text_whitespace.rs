mod support;
use support::*;
#[test]
fn safe_fix_preserves_bom_crlf_and_markdown_breaks() {
    let sb = Sandbox::new();
    let repo = sb.repo("text");
    repo.write(
        ".reporinse.toml",
        "version = 1\n[rules.\"text.whitespace\"]\nmode = \"fix\"\n",
    );
    repo.write("a.md", b"\xef\xbb\xbfhello  \r\nworld\t\r\nlast ");
    assert_code(&repo.reporinse(&["fix", "--files", "a.md", "--dry-run"]), 0);
    assert_eq!(
        std::fs::read(repo.path("a.md")).unwrap(),
        b"\xef\xbb\xbfhello  \r\nworld\t\r\nlast "
    );
    assert_code(&repo.reporinse(&["fix", "--files", "a.md"]), 0);
    let expected = b"\xef\xbb\xbfhello  \r\nworld\r\nlast";
    assert_eq!(std::fs::read(repo.path("a.md")).unwrap(), expected);
    assert_code(&repo.reporinse(&["fix", "--files", "a.md"]), 0);
    assert_eq!(std::fs::read(repo.path("a.md")).unwrap(), expected);
}
#[test]
fn block_json_exclusions_and_staged_content() {
    let sb = Sandbox::new();
    let repo = sb.repo("staged");
    repo.write(
        ".reporinse.toml",
        "version=1\nexclude=[\"skip.txt\"]\n[rules.\"text.whitespace\"]\nmode=\"block\"\n",
    );
    repo.write("a.txt", "bad \n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "clean\n");
    let result = repo.reporinse(&["check", "--staged", "--format", "json"]);
    assert_code(&result, 1);
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["findings"][0]["rule_id"], "text.whitespace");
    assert_code(&repo.reporinse(&["check", "--files", "a.txt"]), 0);
    repo.write("skip.txt", "bad \n");
    assert_code(&repo.reporinse(&["check", "--files", "skip.txt"]), 0);
    assert_code(
        &repo.reporinse(&["check", "--files", "a.txt", "--only", "missing.rule"]),
        2,
    );
}
#[test]
fn stale_write_and_binary_are_refused() {
    use reporinse::core::safety;
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("a.txt");
    std::fs::write(&file, b"first").unwrap();
    let (_, bytes) = safety::read(&file, "a.txt").unwrap();
    std::fs::write(&file, b"other").unwrap();
    assert!(safety::replace(temp.path(), &file, &bytes, "fixed", false).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"other");
    std::fs::write(&file, b"a\0b").unwrap();
    assert!(safety::read(&file, "a.txt").is_err());
    assert!(safety::resolve(temp.path(), &temp.path().join("../outside")).is_err());
}
