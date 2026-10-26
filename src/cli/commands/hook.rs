use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct HookArgs {
    /// Provider that invoked the hook: git, claude, codex, gemini, cursor.
    pub provider: String,
    /// Hook/event name (e.g. commit-msg, pre-commit, PreToolUse).
    pub event: String,
    /// Extra arguments passed by the host (e.g. the commit message file).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

pub fn run(_args: &HookArgs, out: &mut Output) -> i32 {
    not_implemented(out, "hook")
}
