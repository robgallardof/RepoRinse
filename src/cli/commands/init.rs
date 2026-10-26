use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// minimal, balanced, strict or custom.
    #[arg(long, default_value = "balanced")]
    pub preset: String,
    /// Overwrite an existing .reporinse.toml.
    #[arg(long)]
    pub force: bool,
}

pub fn run(_args: &InitArgs, out: &mut Output) -> i32 {
    not_implemented(out, "init")
}
