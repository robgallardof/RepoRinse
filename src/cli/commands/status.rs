use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct StatusArgs {}

pub fn run(_args: &StatusArgs, out: &mut Output) -> i32 {
    not_implemented(out, "status")
}
