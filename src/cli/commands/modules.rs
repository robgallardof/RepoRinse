use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::output::Output;
use crate::core::model::{RuleMetadata, exit_codes};

#[derive(Debug, Args)]
pub struct ModulesArgs {
    #[command(subcommand)]
    pub action: Option<ModulesAction>,
}

#[derive(Debug, Subcommand)]
pub enum ModulesAction {
    /// List every module with availability, default mode and risk.
    List,
    /// Show one module's details.
    Info { id: String },
}

/// Stable JSON shape of a module for `modules list/info --json`.
#[derive(Debug, Serialize)]
pub struct ModuleInfo {
    pub id: &'static str,
    pub version: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
    pub availability: String,
    pub priority: String,
    pub risk: String,
    pub default_mode: &'static str,
    pub modes: Vec<&'static str>,
    pub targets: Vec<String>,
    pub providers: Vec<&'static str>,
    pub docs: String,
}

impl ModuleInfo {
    pub fn from(m: &RuleMetadata) -> ModuleInfo {
        let json = |v: &dyn erased::Ser| v.to_plain();
        ModuleInfo {
            id: m.id,
            version: m.version,
            category: m.category,
            summary: m.summary,
            availability: json(&m.availability),
            priority: json(&m.priority),
            risk: json(&m.risk),
            default_mode: m.default_mode.as_str(),
            modes: std::iter::once("off")
                .chain(m.modes.iter().map(|x| x.as_str()).filter(|x| *x != "off"))
                .collect(),
            targets: m.targets.iter().map(|t| json(t)).collect(),
            providers: m.providers.to_vec(),
            docs: m.docs_path(),
        }
    }
}

mod erased {
    /// Renders a unit enum through its serde name (e.g. `Availability::Implemented` → `implemented`).
    pub trait Ser {
        fn to_plain(&self) -> String;
    }

    impl<T: serde::Serialize> Ser for T {
        fn to_plain(&self) -> String {
            serde_json::to_value(self)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        }
    }
}

pub fn run(args: &ModulesArgs, out: &mut Output) -> i32 {
    let registry = crate::modules::registry();
    match &args.action {
        None | Some(ModulesAction::List) => {
            let modules: Vec<ModuleInfo> = registry
                .all()
                .map(|r| ModuleInfo::from(r.metadata()))
                .collect();
            if out.json {
                out.print_json(&modules);
                return exit_codes::OK;
            }
            if modules.is_empty() {
                out.line("No modules are available in this build.");
                return exit_codes::OK;
            }
            let width = modules.iter().map(|m| m.id.len()).max().unwrap_or(0) + 2;
            out.line(out.bold(&format!(
                "{:width$}{:<13}{:<8}{:<8}{}",
                "MODULE", "STATUS", "DEFAULT", "RISK", "SUMMARY"
            )));
            for m in &modules {
                out.line(format!(
                    "{:width$}{:<13}{:<8}{:<8}{}",
                    m.id, m.availability, m.default_mode, m.risk, m.summary
                ));
            }
            exit_codes::OK
        }
        Some(ModulesAction::Info { id }) => {
            let Some(rule) = registry.find(id) else {
                out.error(format!(
                    "unknown module '{id}'. Run 'reporinse modules list'."
                ));
                return exit_codes::USAGE;
            };
            let info = ModuleInfo::from(rule.metadata());
            if out.json {
                out.print_json(&info);
                return exit_codes::OK;
            }
            out.line(format!("{}  {}", out.bold(info.id), info.summary));
            out.line(format!("  category:     {}", info.category));
            out.line(format!("  availability: {}", info.availability));
            out.line(format!("  priority:     {}", info.priority));
            out.line(format!("  risk:         {}", info.risk));
            out.line(format!(
                "  modes:        {} (default: {})",
                info.modes.join(", "),
                info.default_mode
            ));
            out.line(format!("  targets:      {}", info.targets.join(", ")));
            out.line(format!("  providers:    {}", info.providers.join(", ")));
            out.line(format!("  docs:         {}", info.docs));
            exit_codes::OK
        }
    }
}
