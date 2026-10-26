use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

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

pub fn run(_args: &RuleArgs, out: &mut Output) -> i32 {
    not_implemented(out, "rule explain")
}
