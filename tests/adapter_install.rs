mod support;
use support::*;
#[test]
fn installers_merge_idempotently_restore_exact_bytes_and_refuse_drift() {
    for host in ["claude", "codex", "gemini", "cursor"] {
        let sb = Sandbox::new();
        let repo = sb.repo(host);
        let filename = if matches!(host, "claude" | "gemini") {
            "settings.json"
        } else {
            "hooks.json"
        };
        let file = format!(".{host}/{filename}");
        let original = b"{\n  \"foreignSetting\": true, \"hooks\": {}\n}\n";
        repo.write(&file, original);
        let install = ["install", "--provider", host, "--all", "--yes"];
        assert_code(
            &repo.reporinse(&["install", "--provider", host, "--all", "--dry-run"]),
            0,
        );
        assert_eq!(std::fs::read(repo.path(&file)).unwrap(), original);
        assert_code(&repo.reporinse(&install), 0);
        let installed = std::fs::read(repo.path(&file)).unwrap();
        assert_code(&repo.reporinse(&install), 0);
        assert_eq!(std::fs::read(repo.path(&file)).unwrap(), installed);
        let value: serde_json::Value = serde_json::from_slice(&installed).unwrap();
        assert_eq!(value["foreignSetting"], true);
        if host == "claude" {
            assert_eq!(
                value["attribution"],
                serde_json::json!({"commit":"", "pr":""})
            );
        }
        let mut changed = installed.clone();
        changed.push(b' ');
        repo.write(&file, &changed);
        assert_code(
            &repo.reporinse(&["uninstall", "--provider", host, "--all", "--yes"]),
            4,
        );
        assert_eq!(std::fs::read(repo.path(&file)).unwrap(), changed);
        repo.write(&file, &installed);
        assert_code(
            &repo.reporinse(&["uninstall", "--provider", host, "--all", "--yes"]),
            0,
        );
        assert_eq!(std::fs::read(repo.path(&file)).unwrap(), original);
        assert!(
            !repo
                .path(&format!(".{host}/reporinse-manifest.json"))
                .exists()
        );
    }
}
#[test]
fn user_scope_and_multi_provider_dry_run_are_isolated() {
    let sb = Sandbox::new();
    let repo = sb.repo("multiple");
    assert_code(
        &repo.reporinse(&[
            "install",
            "--provider",
            "claude,codex,gemini,cursor",
            "--all",
            "--scope",
            "user",
            "--yes",
        ]),
        0,
    );
    for host in ["claude", "codex", "gemini", "cursor"] {
        assert!(
            sb.home
                .join(format!(".{host}/reporinse-manifest.json"))
                .exists()
        );
    }
    assert_code(
        &repo.reporinse(&[
            "uninstall",
            "--provider",
            "claude,codex,gemini,cursor",
            "--all",
            "--scope",
            "user",
            "--yes",
        ]),
        0,
    );
    assert_code(
        &repo.reporinse(&["install", "--provider", "git,claude", "--all", "--dry-run"]),
        0,
    );
    assert!(!repo.path(".claude").exists());
    assert!(!repo.hooks_dir().join("commit-msg").exists());
}
