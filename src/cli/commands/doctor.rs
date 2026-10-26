use crate::cli::output::Output;
use clap::Args;
#[derive(Debug, Args)]
pub struct DoctorArgs {}
pub fn run(_args: &DoctorArgs, out: &mut Output) -> i32 {
    super::status::inspect(true, out)
}
