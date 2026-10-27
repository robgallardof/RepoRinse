//! Explicit string policies, not a shell parser or a security sandbox.
use crate::core::{
    model::*,
    rule::{Rule, RuleContext},
};
pub struct Guard(pub bool);
static COMMAND: RuleMetadata = RuleMetadata {
    id: "agent.command-guard",
    version: "1.0.0",
    category: "agent",
    summary: "Match configured command globs without executing commands.",
    risk: RiskLevel::Low,
    priority: Priority::P0,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block],
    default_mode: RuleMode::Warn,
    targets: &[TargetKind::AgentEvent],
    providers: &[
        "claude", "codex", "gemini", "cursor", "opencode", "deepseek", "cli",
    ],
};
static PATH: RuleMetadata = RuleMetadata {
    id: "agent.path-scope",
    summary: "Report file paths matching configured protected globs.",
    ..COMMAND
};
impl Rule for Guard {
    fn metadata(&self) -> &RuleMetadata {
        if self.0 { &COMMAND } else { &PATH }
    }
    fn validate_options(&self, options: &toml::Table) -> Vec<String> {
        let mut errors = Vec::new();
        for (key, value) in options {
            if key != "deny" {
                errors.push(format!("unknown option '{key}'"));
                continue;
            }
            match value.as_array() {
                Some(items) => {
                    for item in items {
                        match item.as_str() {
                            Some(s) => {
                                if let Err(e) = globset::Glob::new(s) {
                                    errors.push(format!("invalid deny glob: {e}"));
                                }
                            }
                            None => errors.push("deny must contain strings".into()),
                        }
                    }
                }
                None => errors.push("deny must be an array of strings".into()),
            }
        }
        errors
    }
    fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult {
        let mut result = RuleResult::default();
        let Some(event) = ctx.event else {
            return result;
        };
        if ctx.settings.mode == RuleMode::Off {
            return result;
        }
        let patterns = ctx.settings.strings("deny");
        let matched = patterns
            .iter()
            .filter_map(|p| globset::Glob::new(p).ok())
            .map(|g| g.compile_matcher())
            .any(|g| {
                if self.0 {
                    event.command.as_ref().is_some_and(|c| g.is_match(c))
                } else {
                    event.paths.iter().any(|p| {
                        let path = p.replace('\\', "/");
                        let cwd = event
                            .working_directory
                            .as_deref()
                            .unwrap_or("")
                            .replace('\\', "/");
                        let relative = path
                            .strip_prefix(&format!("{}/", cwd.trim_end_matches('/')))
                            .unwrap_or(&path);
                        let normalized = relative
                            .split('/')
                            .filter(|s| !s.is_empty() && *s != ".")
                            .fold(Vec::new(), |mut parts, s| {
                                if s == ".." {
                                    if parts.last().is_some_and(|x| *x != "..") {
                                        parts.pop();
                                    } else {
                                        parts.push(s);
                                    }
                                } else {
                                    parts.push(s);
                                }
                                parts
                            })
                            .join("/");
                        g.is_match(&path) || g.is_match(relative) || g.is_match(normalized)
                    })
                }
            });
        if matched {
            result.findings.push(Finding::new(
                self.metadata().id,
                ctx.settings.severity(),
                "Configured policy matched; review the requested action",
            ));
        }
        result
    }
}
