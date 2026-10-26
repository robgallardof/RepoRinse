//! The `reporinse` command line. Each command lives in `commands/<name>.rs` with its own flags and `run`.

pub mod commands;
pub mod output;

use std::ffi::OsString;
use std::io::IsTerminal;

use clap::{Parser, Subcommand};

use crate::core::model::exit_codes;
use output::Output;

#[derive(Debug, Parser)]
#[command(
    name = "reporinse",
    version,
    about = "Keep the code. Rinse the noise.",
    long_about = "Local, deterministic cleanup and guardrails for Git repositories and coding agents. No AI, network or telemetry at runtime."
)]
pub struct Cli {
    /// Machine-readable JSON on stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Only print blocking findings and errors. Exit codes are unchanged.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Disable colors (also honoured: NO_COLOR, TERM=dumb, non-terminal stdout).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// More detail (effective config origins, skipped files…).
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a commented .reporinse.toml for this repository.
    Init(commands::init::InitArgs),
    /// List modules or show one module's details.
    Modules(commands::modules::ModulesArgs),
    /// Install Git hooks / agent integrations for all or some modules.
    Install(commands::install::InstallArgs),
    /// Remove what RepoRinse installed, restoring what was there before.
    Uninstall(commands::install::UninstallArgs),
    /// Show the effective configuration and installed integrations.
    Status(commands::status::StatusArgs),
    /// Diagnose conflicts (core.hooksPath, foreign hooks, drift, missing providers).
    Doctor(commands::doctor::DoctorArgs),
    /// Analyse files, staged changes or a commit message without modifying anything.
    Check(commands::check::CheckArgs),
    /// Apply safe, deterministic fixes (preview with --dry-run).
    Fix(commands::fix::FixArgs),
    /// Validate or explain configuration.
    Config(commands::config::ConfigArgs),
    /// Explain a rule with examples.
    Rule(commands::rule::RuleArgs),
    /// Entry point called by installed hooks (Git and agents). Not meant to be typed by hand.
    #[command(hide = true)]
    Hook(commands::hook::HookArgs),
}

/// Process entry point.
pub fn main() -> i32 {
    run(
        std::env::args_os(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Runs the CLI with explicit streams so tests can call it in-process.
pub fn run<I, T>(argv: I, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() {
                exit_codes::USAGE
            } else {
                exit_codes::OK
            };
            let rendered = e.render().to_string();
            let _ = if e.use_stderr() {
                write!(stderr, "{rendered}")
            } else {
                write!(stdout, "{rendered}")
            };
            return code;
        }
    };

    let color = !cli.no_color
        && !cli.json
        && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
        && std::env::var("TERM").map_or(true, |t| t != "dumb")
        && std::io::stdout().is_terminal();
    let mut out = Output::new(stdout, stderr, cli.json, cli.quiet, color, cli.verbose);

    match &cli.command {
        Command::Init(a) => commands::init::run(a, &mut out),
        Command::Modules(a) => commands::modules::run(a, &mut out),
        Command::Install(a) => commands::install::run_install(a, &mut out),
        Command::Uninstall(a) => commands::install::run_uninstall(a, &mut out),
        Command::Status(a) => commands::status::run(a, &mut out),
        Command::Doctor(a) => commands::doctor::run(a, &mut out),
        Command::Check(a) => commands::check::run(a, &mut out),
        Command::Fix(a) => commands::fix::run(a, &mut out),
        Command::Config(a) => commands::config::run(a, &mut out),
        Command::Rule(a) => commands::rule::run(a, &mut out),
        Command::Hook(a) => commands::hook::run(a, &mut out),
    }
}
