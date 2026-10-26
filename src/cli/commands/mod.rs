//! One file per command: its clap `Args` and a `run` returning an exit code.
mod agent_hook;

pub mod check;
pub mod config;
pub mod doctor;
pub mod fix;
pub mod hook;
pub mod init;
pub mod install;
pub mod modules;
pub mod rule;
mod scan;
pub mod status;
