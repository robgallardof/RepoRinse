//! `reporinse install` / `reporinse uninstall`: resolve the module selection against the registry and hand it
//! to the provider's installer. Only providers with a real installer are accepted (today: git).

use std::io::{BufRead, IsTerminal};

use clap::Args;

use crate::cli::output::Output;
use crate::core::integration::{InstallActionKind, InstallRequest, InstallResult, InstallScope};
use crate::core::model::{Availability, RiskLevel, RuleMode, exit_codes};
use crate::core::registry::RuleRegistry;

#[derive(Debug, Clone, Args)]
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
    /// Overwrite RepoRinse hooks that changed since they were installed.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub selection: Selection,
    /// Remove files even if they changed since RepoRinse wrote them (asks first unless --yes).
    #[arg(long)]
    pub force: bool,
}

fn parse_scope(s: &str) -> Option<InstallScope> {
    match s {
        "repo" => Some(InstallScope::Repo),
        "user" | "global" => Some(InstallScope::User),
        _ => None,
    }
}

/// Modules that `--all` picks for a provider: implemented or experimental, triggered by that provider, and not
/// a high-risk rule whose default mode modifies files.
pub fn all_for_provider(registry: &RuleRegistry, provider: &str) -> Vec<String> {
    registry
        .all()
        .map(|r| r.metadata())
        .filter(|m| m.providers.contains(&provider))
        .filter(|m| m.availability != Availability::Planned)
        .filter(|m| !(m.risk == RiskLevel::High && m.default_mode == RuleMode::Fix))
        .map(|m| m.id.to_string())
        .collect()
}

/// Resolves `--all/--only/--except` to module ids. `default_all`: what an empty selection means.
fn resolve_modules(
    sel: &Selection,
    registry: &RuleRegistry,
    provider: &str,
    default_all: bool,
) -> Result<Vec<String>, String> {
    for id in sel.only.iter().chain(&sel.except) {
        let Some(rule) = registry.find(id) else {
            return Err(format!(
                "unknown module '{id}'. Run 'reporinse modules list' to see the available ones."
            ));
        };
        if !rule.metadata().providers.contains(&provider) {
            return Err(format!(
                "module '{id}' does not run on provider '{provider}' (providers: {})",
                rule.metadata().providers.join(", ")
            ));
        }
    }
    if !sel.only.is_empty() {
        let mut ids = Vec::new();
        for id in &sel.only {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        return Ok(ids);
    }
    if sel.all || default_all {
        return Ok(all_for_provider(registry, provider)
            .into_iter()
            .filter(|id| !sel.except.contains(id))
            .collect());
    }
    Ok(Vec::new())
}

/// Common checks; Err is the exit code after printing the problem.
fn prepare(
    sel: &Selection,
    out: &mut Output,
) -> Result<(InstallScope, Box<dyn crate::core::integration::Installer>), i32> {
    let Some(scope) = parse_scope(&sel.scope) else {
        out.error(format!(
            "unknown scope '{}': use repo or user (alias global)",
            sel.scope
        ));
        return Err(exit_codes::USAGE);
    };
    let unsupported: Vec<&String> = sel
        .provider
        .iter()
        .filter(|p| crate::integrations::find_installer(p).is_none())
        .collect();
    if let Some(p) = unsupported.first() {
        let known = crate::integrations::PROVIDERS.contains(&p.to_lowercase().as_str());
        out.error(if known {
            format!(
                "provider '{p}' has no verified native installer; use --provider git for Antigravity"
            )
        } else {
            format!(
                "unknown provider '{p}' (known: {})",
                crate::integrations::PROVIDERS.join(", ")
            )
        });
        return Err(if known {
            exit_codes::NOT_IMPLEMENTED
        } else {
            exit_codes::USAGE
        });
    }
    if sel.provider.len() != 1 {
        out.error("pass exactly one provider (--provider git)");
        return Err(exit_codes::USAGE);
    }
    let installer = crate::integrations::find_installer(&sel.provider[0]).expect("checked above");
    if !installer.supported_scopes().contains(&scope) {
        out.error(format!(
            "provider '{}' does not support scope '{}'",
            installer.provider(),
            sel.scope
        ));
        return Err(exit_codes::USAGE);
    }
    Ok((scope, installer))
}

fn verb(kind: InstallActionKind) -> &'static str {
    match kind {
        InstallActionKind::CreateFile => "create",
        InstallActionKind::UpdateFile => "update",
        InstallActionKind::DeleteFile => "delete",
        InstallActionKind::RestoreFile => "restore",
        InstallActionKind::SetConfig => "set",
        InstallActionKind::UnsetConfig => "unset",
        InstallActionKind::MergeJson => "merge",
        InstallActionKind::Skip => "keep",
    }
}

fn report(out: &mut Output, result: &InstallResult, dry_run: bool, what: &str) -> i32 {
    if out.json {
        out.print_json(result);
    } else {
        if dry_run {
            out.line(out.bold(&format!("{what} plan (dry run, nothing is written):")));
        }
        for a in &result.actions {
            let v = format!("{:<8}", verb(a.kind));
            let v = if a.kind == InstallActionKind::Skip {
                out.dim(&v)
            } else {
                out.green(&v)
            };
            out.line(format!("  {v}{}  {}", a.target, out.dim(&a.description)));
        }
        for c in &result.conflicts {
            out.error(c);
        }
        for n in &result.notes {
            out.line(format!("note: {n}"));
        }
        if result.succeeded() && !dry_run {
            out.line(out.green(&format!("{what} done.")));
        }
    }
    if result.succeeded() {
        exit_codes::OK
    } else {
        exit_codes::CONFLICT
    }
}

fn request(
    scope: InstallScope,
    modules: Vec<String>,
    dry_run: bool,
    force: bool,
) -> Result<InstallRequest, String> {
    let root =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    Ok(InstallRequest {
        scope,
        root,
        modules,
        dry_run,
        force,
    })
}

pub fn run_install(args: &InstallArgs, out: &mut Output) -> i32 {
    operate(&args.selection, args.force, false, out)
}

/// `--force` deletes files a person may have edited: confirm unless `--yes`.
fn confirm_force(out: &mut Output) -> bool {
    if !std::io::stdin().is_terminal() {
        out.error("--force without --yes needs a terminal to confirm; add --yes to proceed");
        return false;
    }
    out.raw_err("Remove RepoRinse files even if they were edited since install? [y/N] ");
    let mut answer = String::new();
    let _ = std::io::stdin().lock().read_line(&mut answer);
    matches!(answer.trim(), "y" | "Y" | "yes")
}

pub fn run_uninstall(args: &UninstallArgs, out: &mut Output) -> i32 {
    operate(&args.selection, args.force, true, out)
}

fn operate(selection: &Selection, force: bool, remove: bool, out: &mut Output) -> i32 {
    if selection.interactive && !std::io::stdin().is_terminal() {
        out.error("--interactive needs a terminal; use --all or --only <ids> instead");
        return exit_codes::USAGE;
    }
    if force && !selection.yes && !selection.dry_run && !confirm_force(out) {
        return exit_codes::USAGE;
    }
    let registry = crate::modules::registry();
    let mut requests = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for provider in &selection.provider {
        let mut sel = selection.clone();
        sel.provider = vec![provider.to_lowercase()];
        if !seen.insert(sel.provider[0].clone()) {
            continue;
        }
        let (scope, installer) = match prepare(&sel, out) {
            Ok(v) => v,
            Err(code) => return code,
        };
        if sel.interactive {
            let available = all_for_provider(&registry, installer.provider());
            out.raw_err(format!(
                "{} modules: {}\nEnter comma-separated module IDs (empty = all): ",
                installer.provider(),
                available.join(", ")
            ));
            let mut answer = String::new();
            if std::io::stdin().lock().read_line(&mut answer).is_err() {
                out.error("cannot read selection");
                return exit_codes::USAGE;
            }
            if !answer.trim().is_empty() {
                sel.only = answer
                    .trim()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                sel.all = false;
                sel.except.clear();
            }
        }
        let modules = if remove && sel.all {
            Vec::new()
        } else {
            match resolve_modules(&sel, &registry, installer.provider(), !remove) {
                Ok(v) => v,
                Err(e) => {
                    out.error(e);
                    return exit_codes::USAGE;
                }
            }
        };
        if !remove && modules.is_empty() {
            out.error("no module selected; use --all or --only <ids>");
            return exit_codes::USAGE;
        }
        let req = match request(scope, modules, true, force) {
            Ok(v) => v,
            Err(e) => {
                out.error(e);
                return exit_codes::INTERNAL;
            }
        };
        requests.push((installer, req));
    }
    let mut plan = InstallResult::default();
    for (installer, req) in &requests {
        let result = if remove {
            installer.uninstall(req)
        } else {
            installer.install(req)
        };
        plan.actions.extend(result.actions);
        plan.notes.extend(result.notes);
        plan.conflicts.extend(result.conflicts);
    }
    let operation = if remove { "uninstall" } else { "install" };
    if selection.dry_run || !plan.succeeded() {
        return report(out, &plan, true, operation);
    }
    let mut result = InstallResult::default();
    for (installer, mut req) in requests {
        req.dry_run = false;
        let step = if remove {
            installer.uninstall(&req)
        } else {
            installer.install(&req)
        };
        let succeeded = step.succeeded();
        result.actions.extend(step.actions);
        result.notes.extend(step.notes);
        result.conflicts.extend(step.conflicts);
        if !succeeded {
            result.notes.push("Earlier providers may already have completed; inspect status. Each provider operation rolls back its own failed writes.".into());
            break;
        }
    }
    report(out, &result, false, operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(all: bool, only: &[&str], except: &[&str]) -> Selection {
        Selection {
            all,
            only: only.iter().map(|s| s.to_string()).collect(),
            except: except.iter().map(|s| s.to_string()).collect(),
            provider: vec!["git".into()],
            scope: "repo".into(),
            dry_run: false,
            yes: false,
            interactive: false,
        }
    }

    #[test]
    fn resolves_selection() {
        let registry = crate::modules::registry();
        let all = resolve_modules(&sel(true, &[], &[]), &registry, "git", false).unwrap();
        assert!(all.contains(&"git.ai-attribution".to_string()));
        let none = resolve_modules(
            &sel(true, &[], &["git.ai-attribution"]),
            &registry,
            "git",
            false,
        )
        .unwrap();
        assert!(!none.contains(&"git.ai-attribution".to_string()));
        let only = resolve_modules(
            &sel(false, &["git.ai-attribution", "git.ai-attribution"], &[]),
            &registry,
            "git",
            false,
        )
        .unwrap();
        assert_eq!(only, vec!["git.ai-attribution".to_string()]);
        assert!(resolve_modules(&sel(false, &["nope.x"], &[]), &registry, "git", false).is_err());
        assert!(
            resolve_modules(&sel(false, &[], &[]), &registry, "git", false)
                .unwrap()
                .is_empty()
        );
    }
}
