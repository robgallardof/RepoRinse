use crate::core::{
    model::*,
    rule::{Rule, RuleContext},
};
pub struct DuplicateComments;
static META: RuleMetadata = RuleMetadata {
    id: "code.duplicate-comments",
    version: "1.0.0",
    category: "code",
    summary: "Opt-in review of consecutive identical line comments.",
    risk: RiskLevel::Medium,
    priority: Priority::P1,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block],
    default_mode: RuleMode::Off,
    targets: &[TargetKind::File],
    providers: &["git", "cli"],
};
impl Rule for DuplicateComments {
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
        let Some(doc) = ctx.document else {
            return result;
        };
        if ctx.settings.mode == RuleMode::Off {
            return result;
        }
        let marker = match doc.extension().as_str() {
            "rs" | "cs" | "js" | "jsx" | "ts" | "tsx" | "java" | "go" | "c" | "cpp" | "h" => "//",
            "py" | "rb" | "sh" => "#",
            _ => return result,
        };
        let mut previous = None;
        for (line, text) in doc.text.lines().enumerate() {
            let text = text.trim();
            if text.starts_with(marker) && text.len() > marker.len() + 2 {
                if previous == Some(text) {
                    result.findings.push(Finding::new(META.id,ctx.settings.severity(),"Consecutive identical comments; review whether the repetition is intentional").at(&doc.path,Some(line+1),None));
                }
                previous = Some(text);
            } else {
                previous = None;
            }
        }
        result
    }
}
