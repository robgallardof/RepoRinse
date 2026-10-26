use reporinse::core::{config::EffectiveConfig, engine, model::*, rule::RuleSettings};
#[test]
fn changeset_rules_have_negative_cases_and_honor_exclusions() {
    let registry = reporinse::modules::registry();
    let mut config = EffectiveConfig::built_in(&registry);
    let changes = ChangeSet {
        staged: true,
        paths: vec![
            ChangedPath {
                path: "Cargo.toml".into(),
                status: ChangeStatus::Modified,
                added_lines: 600,
                deleted_lines: 0,
                binary: false,
            },
            ChangedPath {
                path: "SUMMARY.md".into(),
                status: ChangeStatus::Added,
                added_lines: 1,
                deleted_lines: 0,
                binary: false,
            },
        ],
    };
    let report = engine::run_changes(&registry, &config, &changes, &[]);
    for id in [
        "git.diff-scope",
        "git.dependency-watch",
        "docs.agent-artifacts",
    ] {
        assert!(report.findings.iter().any(|f| f.rule_id == id));
    }
    config.exclude = vec!["Cargo.toml".into(), "SUMMARY.md".into()];
    assert!(
        engine::run_changes(&registry, &config, &changes, &[])
            .findings
            .is_empty()
    );
    assert!(
        engine::run_changes(
            &registry,
            &config,
            &ChangeSet {
                staged: true,
                paths: vec![]
            },
            &[]
        )
        .findings
        .is_empty()
    );
}
#[test]
fn explicit_guards_do_not_match_clean_actions_and_do_not_leak_secrets() {
    let registry = reporinse::modules::registry();
    let mut config = EffectiveConfig::built_in(&registry);
    let mut settings = RuleSettings::with_mode(RuleMode::Block);
    settings
        .options
        .insert("deny".into(), toml::Value::Array(vec!["*.git/*".into()]));
    config.set("agent.path-scope", settings, "test");
    let mut event = NormalizedEvent {
        provider: "claude".into(),
        original_event: "PreToolUse".into(),
        stage: EventStage::Before,
        action: EventAction::FileWrite,
        working_directory: Some("/repo".into()),
        paths: vec!["/repo/.git/config".into()],
        command: None,
        session_id: None,
        tool_name: Some("Write".into()),
    };
    assert!(engine::run_event(&registry, &config, &event, &[]).blocked());
    event.paths = vec!["/repo/src/a.rs".into()];
    assert!(!engine::run_event(&registry, &config, &event, &[]).blocked());
}
