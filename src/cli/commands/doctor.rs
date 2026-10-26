use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct DoctorArgs {}

pub fn run(_args: &DoctorArgs, out: &mut Output) -> i32 {
    not_implemented(out, "doctor")
}
