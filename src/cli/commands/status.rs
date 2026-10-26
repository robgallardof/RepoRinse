use crate::{
    cli::output::Output,
    core::{
        config,
        integration::{InstallScope, IntegrationStatus},
        model::exit_codes,
    },
    integrations::git::command::Git,
};
use clap::Args;
use serde::Serialize;

#[derive(Debug, Args)]
pub struct StatusArgs {}

#[derive(Serialize)]
struct Status {
    version: &'static str,
    repository: bool,
    preset: Option<String>,
    rules: std::collections::BTreeMap<String, String>,
    integrations: Vec<IntegrationStatus>,
    problems: Vec<String>,
}

pub fn run(_args: &StatusArgs, out: &mut Output) -> i32 {
    inspect(false, out)
}

pub(crate) fn inspect(doctor: bool, out: &mut Output) -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            out.error("cannot access current directory");
            return exit_codes::INTERNAL;
        }
    };
    let top = Git::at(&cwd)
        .text(&["rev-parse", "--show-toplevel"])
        .ok()
        .map(std::path::PathBuf::from);
    let root = top.as_deref().unwrap_or(&cwd);
    // Reports are shareable: never include absolute workspace or home directories.
    let redact = |text: String| {
        let mut text = text
            .replace(&root.to_string_lossy().replace('\\', "/"), "<repo>")
            .replace(root.to_string_lossy().as_ref(), "<repo>");
        for key in [
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "XDG_CONFIG_HOME",
            "REPORINSE_CONFIG_HOME",
        ] {
            if let Some(value) = std::env::var_os(key) {
                let value = value.to_string_lossy();
                if !value.is_empty() {
                    text = text
                        .replace(&value.replace('\\', "/"), "<user>")
                        .replace(value.as_ref(), "<user>");
                }
            }
        }
        text
    };
    let mut status = Status {
        version: env!("CARGO_PKG_VERSION"),
        repository: top.is_some(),
        preset: None,
        rules: Default::default(),
        integrations: Vec::new(),
        problems: Vec::new(),
    };
    let registry = crate::modules::registry();
    let mut code = exit_codes::OK;
    match config::load(&registry, Some(root), &config::CliOverrides::default()) {
        Ok(config) => {
            status.preset = Some(config.preset.clone());
            status.rules = registry
                .all()
                .map(|r| {
                    (
                        r.metadata().id.to_string(),
                        config.settings(r.metadata().id).mode.as_str().to_string(),
                    )
                })
                .collect();
        }
        Err(e) => {
            status.problems.push(redact(e.to_string()));
            code = exit_codes::CONFIG_ERROR;
        }
    }
    for installer in crate::integrations::installers() {
        for scope in [InstallScope::Repo, InstallScope::User] {
            if scope == InstallScope::Repo && top.is_none() {
                continue;
            }
            let mut integration = installer.status(scope, root);
            integration.problems = integration.problems.into_iter().map(&redact).collect();
            if installer.provider() == "git" && !integration.detected {
                status.problems.push("Git is not available".into());
            }
            if doctor && !integration.problems.is_empty() && code == exit_codes::OK {
                code = exit_codes::CONFLICT;
            }
            status.integrations.push(integration);
        }
    }
    if doctor && !status.problems.is_empty() && code == exit_codes::OK {
        code = exit_codes::CONFLICT;
    }
    if out.json {
        out.print_json(&status);
    } else {
        out.line(format!("RepoRinse {}", status.version));
        out.line(format!("Repository: {}", status.repository));
        if let Some(preset) = &status.preset {
            out.line(format!("Preset: {preset}"));
        }
        for (id, mode) in &status.rules {
            out.line(format!("  {id}: {mode}"));
        }
        for integration in &status.integrations {
            out.line(format!(
                "{} ({:?}): {}",
                integration.provider,
                integration.scope,
                if integration.installed {
                    "installed"
                } else {
                    "not installed"
                }
            ));
            for problem in &integration.problems {
                out.warn(problem);
            }
        }
        for problem in &status.problems {
            out.error(problem);
        }
        if doctor && code == exit_codes::OK {
            out.line("No configuration or installed-hook problems found.");
        }
    }
    code
}
