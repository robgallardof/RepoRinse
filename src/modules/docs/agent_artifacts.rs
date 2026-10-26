use crate::core::{
    model::*,
    rule::{Rule, RuleContext},
};
pub struct AgentArtifacts;
static META: RuleMetadata = RuleMetadata {
    id: "docs.agent-artifacts",
    version: "1.0.0",
    category: "docs",
    summary: "Review newly added Markdown plans and agent summaries; never delete them.",
    risk: RiskLevel::Low,
    priority: Priority::P0,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block],
    default_mode: RuleMode::Warn,
    targets: &[TargetKind::ChangeSet],
    providers: &["git", "cli"],
};
impl Rule for AgentArtifacts {
    fn metadata(&self) -> &RuleMetadata {
        &META
    }
    fn validate_options(&self, options: &toml::Table) -> Vec<String> {
        options
            .keys()
            .map(|k| format!("unknown option '{k}'"))
            .collect()
    }
    fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult {
        let mut result = RuleResult::default();
        let Some(changes) = ctx.changes else {
            return result;
        };
        if ctx.settings.mode == RuleMode::Off {
            return result;
        }
        for p in &changes.paths {
            if p.status != ChangeStatus::Added {
                continue;
            }
            let path = p.path.to_lowercase();
            if path.starts_with("docs/") || path.starts_with(".github/") {
                continue;
            }
            if matches!(
                path.rsplit('/').next(),
                Some(
                    "plan.md"
                        | "summary.md"
                        | "implementation_plan.md"
                        | "walkthrough.md"
                        | "task.md"
                        | "agent-notes.md"
                        | "claude-summary.md"
                )
            ) {
                result.findings.push(Finding::new(META.id,ctx.settings.severity(),"New plan or agent summary; confirm this document belongs in the repository").at(&p.path,None,None));
            }
        }
        result
    }
}
