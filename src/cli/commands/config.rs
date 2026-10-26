use clap::Args;

use crate::cli::output::Output;
use crate::core::{config, model::exit_codes};
use crate::integrations::git::command::Git;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum ConfigAction {
    /// Validate the effective configuration.
    Validate,
    /// Show every effective value and where it comes from.
    Explain,
}

pub fn run(args: &ConfigArgs, out: &mut Output) -> i32 {
    let root = Git::at(".")
        .text(&["rev-parse", "--show-toplevel"])
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::current_dir().map_err(|e| e.to_string()));
    let root = match root {
        Ok(root) => root,
        Err(e) => {
            out.error(e);
            return exit_codes::CONFIG_ERROR;
        }
    };
    match config::load(
        &crate::modules::registry(),
        Some(&root),
        &config::CliOverrides::default(),
    ) {
        Err(e) => {
            out.error(e.to_string());
            exit_codes::CONFIG_ERROR
        }
        Ok(config) => {
            match args.action {
                ConfigAction::Validate => {
                    if out.json {
                        out.print_json(&serde_json::json!({"valid": true}));
                    } else {
                        out.line("Configuration is valid.");
                    }
                }
                ConfigAction::Explain => {
                    let rules: std::collections::BTreeMap<_, _> = crate::modules::registry().all().map(|r| {
                        let id = r.metadata().id;
                        let s = config.settings(id);
                        (id.to_string(), serde_json::json!({"mode": s.mode, "exclude": s.exclude, "options": s.options}))
                    }).collect();
                    let value = serde_json::json!({"preset": config.preset, "exclude": config.exclude,
                        "providers": config.providers, "rules": rules, "origins": config.origins()});
                    if out.json {
                        out.print_json(&value);
                    } else {
                        out.line(
                            serde_json::to_string_pretty(&value)
                                .expect("serializable configuration"),
                        );
                    }
                }
            }
            exit_codes::OK
        }
    }
}
