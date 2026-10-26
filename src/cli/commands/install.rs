use clap::Args;

use super::not_implemented;
use crate::cli::output::Output;

#[derive(Debug, Args)]
pub struct Selection {
    /// Every module applicable to the provider (never enables high-risk modifying modes).
    #[arg(long, conflicts_with = "only")]
    pub all: bool,
    /// Only these module ids (comma separated).
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// Exclude these module ids from --all.
    #[arg(long, value_delimiter = ',', requires = "all")]
    pub except: Vec<String>,
    /// Providers: git, claude, codex, gemini, cursor (comma separated).
    #[arg(long, value_delimiter = ',', default_value = "git")]
    pub provider: Vec<String>,
    /// repo or user (alias: global).
    #[arg(long, default_value = "repo")]
    pub scope: String,
    /// Show the plan, write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Do not ask for confirmation.
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// Pick modules/providers interactively (requires a terminal).
    #[arg(long)]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[command(flatten)]
    pub selection: Selection,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub selection: Selection,
    /// Remove files even if they changed since RepoRinse wrote them (asks first unless --yes).
    #[arg(long)]
    pub force: bool,
}

pub fn run_install(_args: &InstallArgs, out: &mut Output) -> i32 {
    not_implemented(out, "install")
}

pub fn run_uninstall(_args: &UninstallArgs, out: &mut Output) -> i32 {
    not_implemented(out, "uninstall")
}
