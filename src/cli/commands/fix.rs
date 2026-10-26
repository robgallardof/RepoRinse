use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct FixArgs {
    /// Fix staged files (refuses files that also have unstaged changes).
    #[arg(long, conflicts_with = "files")]
    pub staged: bool,
    /// Fix these files.
    #[arg(long, num_args = 1.., value_delimiter = ',')]
    pub files: Vec<String>,
    /// Fix a commit message file in place.
    #[arg(long)]
    pub message: Option<String>,
    /// Only run these rule ids.
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// Show the diff without writing.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(_args: &FixArgs, out: &mut Output) -> i32 {
    not_implemented(out, "fix")
}
