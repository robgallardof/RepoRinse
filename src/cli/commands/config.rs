use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum ConfigAction {
    /// Validate the effective configuration.
    Validate,
    /// Show every effective value and where it comes from.
    Explain,
}

pub fn run(_args: &ConfigArgs, out: &mut Output) -> i32 {
    not_implemented(out, "config")
}
