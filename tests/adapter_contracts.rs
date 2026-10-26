use reporinse::{
    core::{
        integration::{Adapter, HookDecision},
        model::*,
    },
    integrations::agent_adapter::HostAdapter,
};
#[test]
fn four_protocols_parse_and_block_without_autoapproval() {
    for (host, event, tool) in [
        ("claude", "PreToolUse", "Bash"),
        ("codex", "PreToolUse", "Bash"),
        ("gemini", "BeforeTool", "run_shell_command"),
        ("cursor", "preToolUse", "Shell"),
    ] {
        let adapter = HostAdapter(host);
        let payload=serde_json::json!({"hook_event_name":event,"tool_name":tool,"tool_input":{"command":"echo secret"},"cwd":"/repo","session_id":"s"}).to_string();
        let parsed = adapter.parse(&payload).unwrap();
        assert_eq!(parsed.command.as_deref(), Some("echo secret"));
        assert_eq!(parsed.stage, EventStage::Before);
        let finding = Finding::new("agent.command-guard", Severity::Error, "secret token=abc");
        let response = adapter.render(&parsed, HookDecision::Block, &[finding]);
        assert_eq!(response.exit_code, 0);
        let value: serde_json::Value = serde_json::from_str(&response.stdout).unwrap();
        assert!(!response.stdout.contains("token=abc"));
        match host {
            "claude" | "codex" => {
                assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny")
            }
            "gemini" => assert_eq!(value["decision"], "deny"),
            _ => assert_eq!(value["permission"], "deny"),
        }
        let allowed = adapter.render(&parsed, HookDecision::Allow, &[]);
        assert!(!allowed.stdout.contains("permissionDecision"));
        assert!(!allowed.stdout.contains("approve"));
    }
}
#[test]
fn post_and_lifecycle_events_never_block_or_loop() {
    for (host, event) in [
        ("claude", "PostToolUse"),
        ("codex", "Stop"),
        ("gemini", "AfterTool"),
        ("cursor", "afterFileEdit"),
    ] {
        let adapter = HostAdapter(host);
        let parsed = adapter
            .parse(&serde_json::json!({"hook_event_name":event}).to_string())
            .unwrap();
        let response = adapter.render(
            &parsed,
            HookDecision::Block,
            &[Finding::new("agent.path-scope", Severity::Error, "policy")],
        );
        assert_eq!(response.exit_code, 0);
        assert!(!response.stdout.contains("deny"));
        assert!(!response.stdout.contains("followup_message"));
        assert!(!response.stdout.contains("\"block\""));
    }
}
#[test]
fn foreign_cursor_payload_and_bounded_messages() {
    let adapter = HostAdapter("claude");
    let parsed=adapter.parse(r#"{"cursor_version":"1.7.2","hook_event_name":"beforeShellExecution","command":"echo x","cwd":"/repo"}"#).unwrap();
    assert_eq!(parsed.provider, "cursor");
    assert_eq!(parsed.command.as_deref(), Some("echo x"));
    let findings = (0..1000)
        .map(|_| Finding::new("agent.command-guard", Severity::Error, "x".repeat(100_000)))
        .collect::<Vec<_>>();
    let response = adapter.render(&parsed, HookDecision::Block, &findings);
    assert!(response.stdout.len() < 4500);
    serde_json::from_str::<serde_json::Value>(&response.stdout).unwrap();
    for payload in [
        String::new(),
        "[]".into(),
        "{".into(),
        "[".repeat(10000),
        "x".repeat(1024 * 1024 + 1),
    ] {
        assert!(adapter.parse(&payload).is_err());
    }
}
