//! Report invisible control characters without guessing whether they are intentional.
use crate::core::{
    model::*,
    rule::{Rule, RuleContext},
};
pub struct InvisibleUnicode;
static META: RuleMetadata = RuleMetadata {
    id: "text.invisible-unicode",
    version: "1.0.0",
    category: "text",
    summary: "Report bidi controls, zero-width spaces and embedded byte-order marks without deleting them.",
    risk: RiskLevel::Low,
    priority: Priority::P0,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block],
    default_mode: RuleMode::Warn,
    targets: &[TargetKind::File],
    providers: &["cli", "git"],
};
impl Rule for InvisibleUnicode {
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
        let (mut line, mut column) = (1, 1);
        for ch in doc.text.chars() {
            if matches!(ch, '\u{061c}' | '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
            {
                result.findings.push(
                    Finding::new(
                        META.id,
                        ctx.settings.severity(),
                        format!(
                            "Invisible control character U+{:04X}; review its intent",
                            ch as u32
                        ),
                    )
                    .at(&doc.path, Some(line), Some(column)),
                );
            }
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        result
    }
    fn validate_options(&self, options: &toml::Table) -> Vec<String> {
        options
            .keys()
            .map(|key| format!("unknown option '{key}'"))
            .collect()
    }
}
