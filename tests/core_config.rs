mod support;

use support::Sandbox;

#[test]
fn init_validates_and_refuses_overwrite() {
    let sb = Sandbox::new();
    let repo = sb.repo("configuration");
    assert!(
        repo.reporinse(&["init", "--preset", "strict"])
            .status
            .success()
    );
    assert!(repo.reporinse(&["config", "validate"]).status.success());
    assert_eq!(repo.reporinse(&["init"]).status.code(), Some(4));
    assert_eq!(
        repo.reporinse(&["init", "--preset", "typo", "--force"])
            .status
            .code(),
        Some(2)
    );
    let out = repo.reporinse(&["--json", "config", "explain"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["preset"], "strict");
    assert_eq!(value["rules"]["git.ai-attribution"]["mode"], "block");
}

#[test]
fn repository_overrides_user_and_options_merge() {
    let sb = Sandbox::new();
    let repo = sb.repo("precedence");
    std::fs::create_dir_all(&sb.reporinse_home).unwrap();
    std::fs::write(sb.reporinse_home.join("config.toml"),
        "version = 1\npreset = 'strict'\n[rules.'git.ai-attribution']\nmode = 'warn'\npolicy = 'keep'\n").unwrap();
    repo.write(
        ".reporinse.toml",
        "version = 1\npreset = 'balanced'\n[rules.'git.ai-attribution']\nmode = 'block'\n",
    );
    let out = repo.reporinse(&["--json", "config", "explain"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["preset"], "balanced");
    assert_eq!(value["rules"]["git.ai-attribution"]["mode"], "block");
    assert_eq!(
        value["rules"]["git.ai-attribution"]["options"]["policy"],
        "keep"
    );
    assert!(
        value["origins"]["rules.git.ai-attribution.mode"]
            .as_str()
            .unwrap()
            .ends_with(".reporinse.toml")
    );
    assert!(
        value["origins"]["rules.git.ai-attribution.policy"]
            .as_str()
            .unwrap()
            .ends_with("config.toml")
    );
}

#[test]
fn invalid_configuration_is_rejected() {
    let sb = Sandbox::new();
    let repo = sb.repo("invalid");
    for invalid in [
        "version = 2",
        "preset = 'typo'",
        "unknown = true",
        "exclude = ['[']",
        "[providers]\ngti = true",
        "[rules.'missing.rule']\nmode = 'warn'",
        "[rules.'git.ai-attribution']\nmode = 'typo'",
        "[rules.'git.ai-attribution']\npolicy = 'typo'",
        "[rules.'git.ai-attribution']\nexclude = [42]",
        "[rules.'git.ai-attribution']\nunknown = true",
    ] {
        repo.write(".reporinse.toml", invalid);
        let out = repo.reporinse(&["config", "validate"]);
        assert_eq!(
            out.status.code(),
            Some(3),
            "{invalid}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn hooks_honor_repository_modes_and_provider_disable() {
    let sb = Sandbox::new();
    let repo = sb.repo("hook config");
    let message = "Change\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";
    repo.write("message.txt", message);
    repo.write(
        ".reporinse.toml",
        "[rules.'git.ai-attribution']\nmode = 'block'\n",
    );
    assert_eq!(
        repo.reporinse(&["hook", "git", "commit-msg", "message.txt"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("message.txt")).unwrap(),
        message
    );
    repo.write(".reporinse.toml", "[providers]\ngit = false\n");
    assert!(
        repo.reporinse(&["hook", "git", "commit-msg", "message.txt"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(repo.path("message.txt")).unwrap(),
        message
    );
}
