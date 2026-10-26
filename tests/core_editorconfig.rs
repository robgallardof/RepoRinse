mod support;
use support::*;
#[test]
fn nested_editorconfig_and_staged_snapshot_are_respected() {
    let sb = Sandbox::new();
    let repo = sb.repo("editorconfig");
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"text.whitespace\"]\nmode=\"fix\"\n",
    );
    repo.write(
        ".editorconfig",
        "root=true\n[*]\ntrim_trailing_whitespace=false\n",
    );
    repo.write(
        "src/.editorconfig",
        "[*.txt]\ntrim_trailing_whitespace=true\n",
    );
    repo.write("root.txt", "keep \n");
    repo.write("src/a.txt", "trim \n");
    assert_code(
        &repo.reporinse(&["fix", "--files", "root.txt", "src/a.txt"]),
        0,
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("root.txt")).unwrap(),
        "keep \n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("src/a.txt")).unwrap(),
        "trim\n"
    );
    repo.git(&["add", ".editorconfig", "root.txt"]);
    repo.write(".editorconfig", "[*]\ntrim_trailing_whitespace=true\n");
    assert_code(&repo.reporinse(&["fix", "--staged"]), 0);
    assert_eq!(
        std::fs::read_to_string(repo.path("root.txt")).unwrap(),
        "keep \n"
    );
}
#[test]
fn staged_fixes_leave_historical_lines_untouched() {
    let sb = Sandbox::new();
    let repo = sb.repo("changed lines");
    repo.write("a.txt", "old \nclean\n");
    repo.git(&["add", "a.txt"]);
    repo.commit_ok("initial\n", &[]);
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"text.whitespace\"]\nmode=\"fix\"\n",
    );
    repo.write("a.txt", "old \nchanged \n");
    repo.git(&["add", "a.txt"]);
    assert_code(&repo.reporinse(&["fix", "--staged"]), 0);
    assert_eq!(
        std::fs::read_to_string(repo.path("a.txt")).unwrap(),
        "old \nchanged\n"
    );
}
