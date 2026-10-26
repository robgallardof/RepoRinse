//! What `reporinse hook git <name>` does, without terminal I/O so it can be unit tested.

use super::command::Git;
use super::staged;
use crate::core::config::EffectiveConfig;
use crate::core::engine::{self, EngineReport};
use crate::core::model::SourceDocument;
use crate::core::planning;
use crate::core::registry::RuleRegistry;

/// Upper bound of fix rounds (one rule's plan per round, re-analysing in between).
const MAX_FIX_ROUNDS: usize = 16;

/// Result of the `commit-msg` hook.
#[derive(Debug, Default)]
pub struct CommitMsgOutcome {
    /// Findings of the first analysis (what the user sees).
    pub report: EngineReport,
    /// New message text when at least one fix applied.
    pub fixed: Option<String>,
    /// Fix plans refused by `planning::apply`.
    pub errors: Vec<String>,
}

/// Analyses a commit message and applies the fix plans of rules in `fix` mode, one rule at a time.
pub fn commit_msg(
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    text: &str,
    only: &[String],
) -> CommitMsgOutcome {
    let mut outcome = CommitMsgOutcome::default();
    let mut current = text.to_string();
    let mut applied_rules: Vec<String> = Vec::new();
    for round in 0..MAX_FIX_ROUNDS {
        let report = engine::run_document(
            registry,
            config,
            &SourceDocument::commit_message(current.clone()),
            only,
        );
        let next = report
            .fixes
            .iter()
            .find(|p| !applied_rules.contains(&p.rule_id))
            .cloned();
        if round == 0 {
            outcome.report = report;
        }
        let Some(plan) = next else {
            break;
        };
        applied_rules.push(plan.rule_id.clone());
        match planning::apply(&current, &plan) {
            Ok(new) => current = new,
            Err(e) => outcome.errors.push(e),
        }
    }
    if current != text {
        outcome.fixed = Some(current);
    }
    outcome
}

/// Analyses the staged content (index, not working tree). Never modifies anything: fix plans are dropped.
pub fn pre_commit(
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    git: &Git,
    only: &[String],
) -> Result<EngineReport, String> {
    let staged = staged::collect(git)?;
    let mut report = EngineReport::default();
    for document in &staged.documents {
        report.merge(engine::run_document(registry, config, document, only));
    }
    if let Some(changes) = &staged.changes {
        report.merge(engine::run_changes(registry, config, changes, only));
    }
    report.fixes.clear();
    report
        .diagnostics
        .extend(staged.skipped.into_iter().map(|s| format!("skipped {s}")));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{
        Availability, Finding, Priority, RiskLevel, RuleMetadata, RuleMode, RuleResult, TargetKind,
    };
    use crate::core::rule::{Rule, RuleContext, RuleSettings};

    #[test]
    fn commit_msg_applies_fixes_and_keeps_findings() {
        let registry = crate::modules::registry();
        let config = EffectiveConfig::built_in(&registry);
        let text = "feat: x\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";
        let out = commit_msg(&registry, &config, text, &[]);
        assert_eq!(out.fixed.as_deref(), Some("feat: x\n"));
        assert_eq!(out.report.findings.len(), 1);
        assert!(!out.report.blocked());

        let clean = commit_msg(&registry, &config, "feat: x\n", &[]);
        assert!(clean.fixed.is_none() && clean.report.findings.is_empty());

        let other_only = commit_msg(&registry, &config, text, &["text.none".to_string()]);
        assert!(other_only.fixed.is_none());
    }

    #[test]
    fn block_mode_blocks_without_fixing() {
        let registry = crate::modules::registry();
        let mut config = EffectiveConfig::built_in(&registry);
        config.set(
            "git.ai-attribution",
            RuleSettings::with_mode(RuleMode::Block),
            "test",
        );
        let out = commit_msg(
            &registry,
            &config,
            "feat: x\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
            &[],
        );
        assert!(out.report.blocked());
        assert!(out.fixed.is_none());
    }

    /// A file rule that blocks on the word FORBIDDEN, to exercise pre-commit without other workstreams.
    struct Forbidden;
    static META: RuleMetadata = RuleMetadata {
        id: "text.forbidden-test",
        version: "0",
        category: "text",
        summary: "test",
        risk: RiskLevel::Low,
        priority: Priority::P2,
        availability: Availability::Experimental,
        modes: &[RuleMode::Warn, RuleMode::Block],
        default_mode: RuleMode::Block,
        targets: &[TargetKind::File],
        providers: &["git"],
    };
    impl Rule for Forbidden {
        fn metadata(&self) -> &RuleMetadata {
            &META
        }
        fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult {
            let doc = ctx.document.unwrap();
            let mut r = RuleResult::empty();
            if doc.text.contains("FORBIDDEN") {
                r.findings.push(
                    Finding::new(META.id, ctx.settings.severity(), "forbidden word").at(
                        &doc.path,
                        Some(1),
                        None,
                    ),
                );
            }
            r
        }
    }

    #[test]
    fn pre_commit_reads_the_index_not_the_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let git = Git::at(&repo)
            .env("HOME", dir.path())
            .env("XDG_CONFIG_HOME", dir.path())
            .env("GIT_CONFIG_GLOBAL", dir.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        git.text(&["init", "-q"]).unwrap();
        std::fs::write(repo.join("a.txt"), "FORBIDDEN\n").unwrap();
        git.text(&["add", "a.txt"]).unwrap();
        // Working tree is clean of the word, the index is not.
        std::fs::write(repo.join("a.txt"), "fine\n").unwrap();

        let registry = RuleRegistry::new(vec![Box::new(Forbidden)]);
        let config = EffectiveConfig::built_in(&registry);
        let report = pre_commit(&registry, &config, &git, &[]).unwrap();
        assert!(report.blocked());
        assert_eq!(report.findings[0].path.as_deref(), Some("a.txt"));
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "fine\n"
        );
        // The index still holds the staged version.
        assert_eq!(git.text(&["show", ":a.txt"]).unwrap(), "FORBIDDEN");
    }
}
