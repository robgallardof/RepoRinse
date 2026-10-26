use crate::core::{
    model::*,
    planning,
    rule::{Rule, RuleContext},
};
pub struct Whitespace;
static META: RuleMetadata = RuleMetadata {
    id: "text.whitespace",
    version: "1.0.0",
    category: "text",
    summary: "Report trailing spaces and tabs; preserve Markdown hard breaks.",
    risk: RiskLevel::Low,
    priority: Priority::P0,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block, RuleMode::Fix],
    default_mode: RuleMode::Warn,
    targets: &[TargetKind::File],
    providers: &["cli", "git"],
};
impl Rule for Whitespace {
    fn metadata(&self) -> &RuleMetadata {
        &META
    }
    fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult {
        let mut result = RuleResult::default();
        let Some(doc) = ctx.document else {
            return result;
        };
        if ctx.settings.mode == RuleMode::Off {
            return result;
        }
        let markdown = matches!(doc.extension().as_str(), "md" | "markdown" | "mdx");
        let mut offset = 0;
        let mut edits = Vec::new();
        for (line, segment) in doc.text.split_inclusive('\n').enumerate() {
            let body = segment.strip_suffix('\n').unwrap_or(segment);
            let body = body.strip_suffix('\r').unwrap_or(body);
            let trimmed = body.trim_end_matches([' ', '\t']);
            let trailing = &body[trimmed.len()..];
            // Two or more spaces encode a Markdown hard break. Preserve them conservatively.
            if !trailing.is_empty()
                && !(markdown && trailing.bytes().all(|b| b == b' ') && trailing.len() >= 2)
            {
                result.findings.push(
                    Finding::new(META.id, ctx.settings.severity(), "Trailing whitespace")
                        .at(&doc.path, Some(line + 1), Some(trimmed.chars().count() + 1))
                        .fixable(),
                );
                edits.push(TextEdit {
                    start: offset + trimmed.len(),
                    len: trailing.len(),
                    replacement: String::new(),
                });
            }
            offset += segment.len();
        }
        if ctx.settings.wants_fix() && !edits.is_empty() {
            result.fixes.push(FixPlan {
                rule_id: META.id.into(),
                target: doc.path.clone(),
                description: "Remove trailing spaces and tabs".into(),
                edits,
                precondition_hash: planning::hash(&doc.text),
            });
        }
        result
    }
    fn validate_options(&self, options: &toml::Table) -> Vec<String> {
        options
            .iter()
            .filter_map(|(key, value)| {
                if key != "honor_editorconfig" {
                    Some(format!("unknown option '{key}'"))
                } else if !value.is_bool() {
                    Some("honor_editorconfig must be a boolean".into())
                } else {
                    None
                }
            })
            .collect()
    }
}
