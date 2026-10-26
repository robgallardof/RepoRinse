use clap::Args;

use crate::cli::output::Output;
use crate::core::model::exit_codes;

#[derive(Debug, Args)]
pub struct RuleArgs {
    #[command(subcommand)]
    pub action: RuleAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum RuleAction {
    /// Explain what a rule detects, its modes and examples.
    Explain { id: String },
}

pub fn run(args: &RuleArgs, out: &mut Output) -> i32 {
    let RuleAction::Explain { id } = &args.action;
    let registry = crate::modules::registry();
    let Some(rule) = registry.find(id) else {
        out.error(format!("unknown rule '{id}'"));
        return exit_codes::USAGE;
    };
    let docs = match id.as_str() {
        "git.ai-attribution" => include_str!("../../../docs/rules/git.ai-attribution.md"),
        "text.invisible-unicode" => include_str!("../../../docs/rules/text.invisible-unicode.md"),
        "text.whitespace" => include_str!("../../../docs/rules/text.whitespace.md"),
        "agent.command-guard" => include_str!("../../../docs/rules/agent.command-guard.md"),
        "agent.path-scope" => include_str!("../../../docs/rules/agent.path-scope.md"),
        "docs.agent-artifacts" => include_str!("../../../docs/rules/docs.agent-artifacts.md"),
        "git.diff-scope" => include_str!("../../../docs/rules/git.diff-scope.md"),
        "git.dependency-watch" => include_str!("../../../docs/rules/git.dependency-watch.md"),
        "code.duplicate-comments" => include_str!("../../../docs/rules/code.duplicate-comments.md"),
        _ => rule.metadata().summary,
    };
    if out.json {
        out.print_json(&serde_json::json!({"rule": super::modules::ModuleInfo::from(rule.metadata()), "documentation": docs}));
    } else {
        out.line(docs);
    }
    exit_codes::OK
}
