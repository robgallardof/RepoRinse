use clap::Args;

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

pub fn run(args: &CheckArgs, out: &mut Output) -> i32 {
    if !matches!(args.format.as_str(), "human" | "plain" | "json") {
        out.error("unknown output format");
        return 2;
    }
    out.json |= args.format == "json";
    super::scan::run(
        &args.files,
        args.message.as_deref(),
        args.staged,
        &args.only,
        false,
        false,
        out,
    )
}
