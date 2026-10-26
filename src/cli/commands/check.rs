use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Check the staged (index) content of staged files.
    #[arg(long, conflicts_with = "files")]
    pub staged: bool,
    /// Check these files (working tree).
    #[arg(long, num_args = 1.., value_delimiter = ',')]
    pub files: Vec<String>,
    /// Check a commit message file (as passed to the commit-msg hook).
    #[arg(long)]
    pub message: Option<String>,
    /// Only run these rule ids.
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// Output format: human, json or plain.
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run(_args: &CheckArgs, out: &mut Output) -> i32 {
    not_implemented(out, "check")
}
