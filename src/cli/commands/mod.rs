//! One file per command: its clap `Args` and a `run` returning an exit code.

pub mod check;
pub mod config;
pub mod doctor;
pub mod fix;
pub mod hook;
pub mod init;
pub mod install;
pub mod modules;
pub mod rule;
pub mod status;

use super::output::Output;
use crate::core::model::exit_codes;

/// Used by commands whose workstream has not landed yet. Never shipped: the release checklist greps for it.
pub(crate) fn not_implemented(out: &mut Output, what: &str) -> i32 {
    out.error(format!("'{what}' is not implemented in this build yet"));
    exit_codes::NOT_IMPLEMENTED
}
