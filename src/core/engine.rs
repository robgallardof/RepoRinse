//! Runs the enabled rules over targets and aggregates findings and fix plans.
//!
//! FROZEN API: `EngineReport`, `run_document`, `run_changes`, `run_event`. Detectors stay pure; this module
//! decides which rules run (mode, `only`, exclusions) and drops fix plans of rules not in `Fix` mode.

use serde::Serialize;

use super::config::EffectiveConfig;
use super::model::{
    ChangeSet, Finding, FixPlan, NormalizedEvent, RuleMode, Severity, SourceDocument, TargetKind,
};
use super::registry::RuleRegistry;
use super::rule::{Rule, RuleContext, RuleSettings};

/// Aggregated output of one engine run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EngineReport {
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
    pub diagnostics: Vec<String>,
}

impl EngineReport {
    /// True when a rule in `block` mode reported something.
    pub fn blocked(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    pub fn merge(&mut self, other: EngineReport) {
        self.findings.extend(other.findings);
        self.fixes.extend(other.fixes);
        self.diagnostics.extend(other.diagnostics);
    }
}

/// Rules that apply to `kind`, are not `Off`, and pass the `only` filter (empty = all).
fn selected<'r>(
    registry: &'r RuleRegistry,
    config: &EffectiveConfig,
    kind: TargetKind,
    only: &[String],
) -> Vec<(&'r dyn Rule, RuleSettings)> {
    registry
        .all()
        .filter(|r| r.metadata().targets.contains(&kind))
        .filter(|r| only.is_empty() || only.iter().any(|id| id == r.metadata().id))
        .map(|r| (r, config.settings(r.metadata().id)))
        .filter(|(r, s)| s.mode != RuleMode::Off && r.metadata().supports(s.mode))
        .collect()
}

fn collect(
    report: &mut EngineReport,
    settings: &RuleSettings,
    mut result: super::model::RuleResult,
) {
    if !settings.wants_fix() {
        result.fixes.clear();
    }
    report.findings.append(&mut result.findings);
    report.fixes.append(&mut result.fixes);
    report.diagnostics.append(&mut result.diagnostics);
}

/// Runs every applicable rule over a commit message or file.
pub fn run_document(
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    document: &SourceDocument,
    only: &[String],
) -> EngineReport {
    let mut report = EngineReport::default();
    for (rule, settings) in selected(registry, config, document.kind, only) {
        if document.kind == TargetKind::File
            && config.is_excluded(rule.metadata().id, &document.path)
        {
            continue;
        }
        let ctx = RuleContext::for_document(&settings, document);
        collect(&mut report, &settings, rule.analyze(&ctx));
    }
    report
}

/// Runs change-set rules (diff scope, dependency watch…).
pub fn run_changes(
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    changes: &ChangeSet,
    only: &[String],
) -> EngineReport {
    let mut report = EngineReport::default();
    for (rule, settings) in selected(registry, config, TargetKind::ChangeSet, only) {
        let filtered = ChangeSet {
            paths: changes
                .paths
                .iter()
                .filter(|p| !config.is_excluded(rule.metadata().id, &p.path))
                .cloned()
                .collect(),
            staged: changes.staged,
        };
        let ctx = RuleContext::for_changes(&settings, &filtered);
        collect(&mut report, &settings, rule.analyze(&ctx));
    }
    report
}

/// Runs agent-event rules (command guard, path scope…).
pub fn run_event(
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    event: &NormalizedEvent,
    only: &[String],
) -> EngineReport {
    let mut report = EngineReport::default();
    for (rule, settings) in selected(registry, config, TargetKind::AgentEvent, only) {
        let ctx = RuleContext::for_event(&settings, event);
        collect(&mut report, &settings, rule.analyze(&ctx));
    }
    report
}
