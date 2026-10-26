use clap::Args;

use crate::cli::output::Output;
use crate::core::{config, model::exit_codes};
use crate::integrations::git::command::Git;
use std::io::Write;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// minimal, balanced, strict or custom.
    #[arg(long, default_value = "balanced")]
    pub preset: String,
    /// Overwrite an existing .reporinse.toml.
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: &InitArgs, out: &mut Output) -> i32 {
    if let Err(e) = config::validate_preset(&args.preset) {
        out.error(e);
        return exit_codes::USAGE;
    }
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
    let path = root.join(config::REPO_FILE);
    // Even --force refuses symlinks: never overwrite a file outside the repository.
    if let Ok(meta) = std::fs::symlink_metadata(&path)
        && (meta.file_type().is_symlink() || !meta.is_file())
    {
        out.error("configuration target is not a regular file");
        return exit_codes::CONFLICT;
    }
    let content = format!(
        "version = 1\npreset = \"{}\"\n\n[providers]\ngit = true\n",
        args.preset
    );
    let result = std::fs::OpenOptions::new()
        .write(true)
        .create(args.force)
        .truncate(args.force)
        .create_new(!args.force)
        .open(&path)
        .and_then(|mut file| file.write_all(content.as_bytes()));
    match result {
        Ok(()) => {
            if out.json {
                out.print_json(&serde_json::json!({"created": path}));
            } else {
                out.line(format!("Created {}", path.display()));
            }
            exit_codes::OK
        }
        Err(e) => {
            out.error(format!("{}: {e}", path.display()));
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                exit_codes::CONFLICT
            } else {
                exit_codes::CONFIG_ERROR
            }
        }
    }
}
