//! `reporinse hook <provider> <event> [args…]`: the entry point installed hooks call. Never prompts.
//!
//! Git: `commit-msg <file>` fixes the message in place (atomic write, newline style kept) and exits 1 only when
//! a rule in `block` mode matched; `pre-commit` checks the staged content and never modifies files or the index.
//! Configuration or I/O problems fail open (exit 0 with a message): cleanup rules must not make commits
//! impossible. Agent providers are filled in by their adapters.

use std::path::{Path, PathBuf};

use clap::Args;

use crate::cli::output::Output;
use crate::core::config::{self, CliOverrides, EffectiveConfig};
use crate::core::engine::EngineReport;
use crate::core::model::{Finding, Severity, exit_codes};
use crate::core::registry::RuleRegistry;
use crate::integrations::git::command::Git;
use crate::integrations::git::{hooks, shim, transaction};
use crate::modules::git::ai_attribution;

#[derive(Debug, Args)]
pub struct HookArgs {
    /// Only run these rule ids (written into the shims by `install --only`).
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// Provider that invoked the hook: git, claude, codex, gemini, cursor.
    pub provider: String,
    /// Hook/event name (e.g. commit-msg, pre-commit, PreToolUse).
    pub event: String,
    /// Extra arguments passed by the host (e.g. the commit message file).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

pub fn run(args: &HookArgs, out: &mut Output) -> i32 {
    if !args
        .provider
        .eq_ignore_ascii_case(crate::integrations::git::PROVIDER)
    {
        return super::agent_hook::run(args, out);
    }
    match args.event.as_str() {
        shim::COMMIT_MSG => commit_msg(args, out),
        shim::PRE_COMMIT => pre_commit(args, out),
        other => {
            out.raw_err(format!(
                "reporinse: Git hook '{other}' has no RepoRinse rules; nothing was checked.\n"
            ));
            exit_codes::OK
        }
    }
}

/// Valid `--only` ids; unknown ones are reported and ignored rather than failing the commit.
fn only_ids(args: &HookArgs, registry: &RuleRegistry, out: &mut Output) -> Vec<String> {
    let mut ids = Vec::new();
    for id in args.only.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if registry.contains(id) {
            ids.push(id.to_string());
        } else {
            out.raw_err(format!(
                "reporinse: unknown module '{id}' in the hook; ignored (reinstall the hooks)\n"
            ));
        }
    }
    ids
}

/// Worktree top when run inside a repository (hooks run there).
fn repo_top() -> Option<PathBuf> {
    Git::at(".")
        .text(&["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from)
}

fn load_config(
    registry: &RuleRegistry,
    top: Option<&Path>,
    out: &mut Output,
) -> Option<EffectiveConfig> {
    match config::load(registry, top, &CliOverrides::default()) {
        Ok(c) => {
            if !c.providers.get("git").copied().unwrap_or(true) {
                return None;
            }
            Some(c)
        }
        Err(e) => {
            out.raw_err(format!(
                "reporinse: configuration error, hook skipped (commit allowed): {e}\n\
                 reporinse: run 'reporinse config validate' to fix it.\n"
            ));
            None
        }
    }
}

/// Git's comment character, so commented lines are never treated as message text.
fn apply_comment_char(config: &mut EffectiveConfig, top: Option<&Path>) {
    let mut settings = config.settings(ai_attribution::ID);
    if settings.str("comment_char").is_some() {
        return;
    }
    let git = Git::at(top.unwrap_or(Path::new(".")));
    let value = ["core.commentString", "core.commentChar"]
        .iter()
        .find_map(|key| git.config_values(&["--get", key]).ok()?.into_iter().next());
    if let Some(c) = value.filter(|v| v.chars().count() == 1) {
        settings.options.insert("comment_char".into(), c.into());
        let origin = config
            .origin(&format!("rules.{}.mode", ai_attribution::ID))
            .unwrap_or("built-in")
            .to_string();
        config.set(ai_attribution::ID, settings, &origin);
    }
}

fn print_findings(out: &mut Output, findings: &[Finding], fixed: bool) {
    for f in findings {
        let level = match f.severity {
            Severity::Error => "blocked",
            _ if fixed && f.fixable => "removed",
            _ => "warning",
        };
        let place = match (&f.path, f.line) {
            (Some(p), Some(l)) => format!("{p}:{l}: "),
            (Some(p), None) => format!("{p}: "),
            (None, Some(l)) => format!("line {l}: "),
            (None, None) => String::new(),
        };
        out.raw_err(format!(
            "reporinse: {level}: {place}{} [{}]\n",
            f.message, f.rule_id
        ));
    }
}

fn print_diagnostics(out: &mut Output, report: &EngineReport) {
    if out.verbose {
        for d in &report.diagnostics {
            out.raw_err(format!("reporinse: note: {d}\n"));
        }
    }
}

fn commit_msg(args: &HookArgs, out: &mut Output) -> i32 {
    let Some(file) = args.rest.first() else {
        out.raw_err(
            "reporinse: commit-msg needs the message file argument; nothing was checked.\n",
        );
        return exit_codes::OK;
    };
    let path = Path::new(file);
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            out.raw_err(format!(
                "reporinse: cannot read {file}: {e}; nothing was checked.\n"
            ));
            return exit_codes::OK;
        }
    };
    let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, bytes.as_slice()),
    };
    let Ok(text) = std::str::from_utf8(body) else {
        out.raw_err("reporinse: the commit message is not UTF-8; nothing was checked.\n");
        return exit_codes::OK;
    };

    let registry = crate::modules::registry();
    let only = only_ids(args, &registry, out);
    let top = repo_top();
    let Some(mut config) = load_config(&registry, top.as_deref(), out) else {
        return exit_codes::OK;
    };
    apply_comment_char(&mut config, top.as_deref());

    let outcome = hooks::commit_msg(&registry, &config, text, &only);
    for e in &outcome.errors {
        out.raw_err(format!("reporinse: fix skipped: {e}\n"));
    }
    let mut fixed = false;
    if let Some(new_text) = &outcome.fixed {
        let mut new_bytes = Vec::with_capacity(new_text.len() + 3);
        if bom {
            new_bytes.extend_from_slice(b"\xEF\xBB\xBF");
        }
        new_bytes.extend_from_slice(new_text.as_bytes());
        match transaction::atomic_write(path, &new_bytes, false) {
            Ok(()) => fixed = true,
            Err(e) => out.raw_err(format!(
                "reporinse: cannot write {file}: {e}; message left unchanged.\n"
            )),
        }
    }
    print_findings(out, &outcome.report.findings, fixed);
    print_diagnostics(out, &outcome.report);
    if outcome.report.blocked() {
        out.raw_err(
            "reporinse: commit blocked by a rule in 'block' mode. Edit the message, or change the rule's mode \
             in .reporinse.toml.\n",
        );
        return exit_codes::BLOCKED;
    }
    exit_codes::OK
}

fn pre_commit(args: &HookArgs, out: &mut Output) -> i32 {
    let registry = crate::modules::registry();
    let only = only_ids(args, &registry, out);
    let top = repo_top();
    let Some(config) = load_config(&registry, top.as_deref(), out) else {
        return exit_codes::OK;
    };
    let git = Git::at(top.unwrap_or_else(|| PathBuf::from(".")));
    let report = match hooks::pre_commit(&registry, &config, &git, &only) {
        Ok(r) => r,
        Err(e) => {
            out.raw_err(format!(
                "reporinse: cannot read the staged changes: {e}; nothing was checked.\n"
            ));
            return exit_codes::OK;
        }
    };
    print_findings(out, &report.findings, false);
    print_diagnostics(out, &report);
    if report.blocked() {
        out.raw_err(
            "reporinse: commit blocked: fix the staged files (or run 'reporinse check --staged' for details).\n",
        );
        return exit_codes::BLOCKED;
    }
    exit_codes::OK
}
