mod support;
use support::Sandbox;

#[test]
fn unicode_findings_never_echo_source_and_do_not_change_files() {
    let sb = Sandbox::new();
    let repo = sb.repo("unicode");
    let source = "private-token-123\u{202e}\n";
    repo.write("input.txt", source);
    let output = repo.reporinse(&["check", "--files", "input.txt", "--json"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["findings"][0]["rule_id"], "text.invisible-unicode");
    assert_eq!(report["findings"][0]["column"], 18);
    assert!(report["fixes"].as_array().unwrap().is_empty());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-token"));
    assert!(
        repo.reporinse(&["fix", "--files", "input.txt"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("input.txt")).unwrap(),
        source
    );
    repo.write(
        "input.txt",
        "caf\u{e9} \u{1f469}\u{200d}\u{1f4bb} \u{0645}\u{0631}\u{062d}\u{0628}\u{0627}\n",
    );
    let output = repo.reporinse(&["check", "--files", "input.txt", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["findings"].as_array().unwrap().is_empty());
}

#[test]
fn doctor_detects_drift_without_leaking_home_or_workspace() {
    let sb = Sandbox::new();
    let repo = sb.repo("private workspace");
    assert!(
        repo.reporinse(&["install", "--only", "git.ai-attribution"])
            .status
            .success()
    );
    assert!(repo.reporinse(&["doctor"]).status.success());
    std::fs::write(repo.hooks_dir().join("commit-msg"), "changed").unwrap();
    let output = repo.reporinse(&["doctor", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["integrations"][0]["problems"][0]
            .as_str()
            .unwrap()
            .contains("changed")
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!text.contains("private workspace"));
    assert!(!text.contains(&sb.home.to_string_lossy().replace('\\', "/")));
    assert!(repo.reporinse(&["status", "--json"]).status.success());
}

#[test]
fn rule_explanations_are_bundled_in_binary() {
    let sb = Sandbox::new();
    let repo = sb.repo("explanations");
    for id in [
        "git.ai-attribution",
        "text.whitespace",
        "text.invisible-unicode",
    ] {
        let output = repo.reporinse(&["rule", "explain", id, "--json"]);
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(value["documentation"].as_str().unwrap().contains("Espa"));
    }
    assert_eq!(
        repo.reporinse(&["rule", "explain", "unknown.rule"])
            .status
            .code(),
        Some(2)
    );
}
