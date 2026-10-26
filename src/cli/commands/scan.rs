use crate::{
    cli::output::Output,
    core::{
        config,
        engine::{self, EngineReport},
        model::{TargetKind, exit_codes},
        planning, safety,
    },
    integrations::git::{command::Git, staged},
    modules,
};
use std::path::{Path, PathBuf};

pub fn run(
    files: &[String],
    message: Option<&str>,
    staged_input: bool,
    only: &[String],
    fix: bool,
    dry_run: bool,
    out: &mut Output,
) -> i32 {
    match execute(files, message, staged_input, only, fix, dry_run, out) {
        Ok(report) => {
            if out.json {
                out.print_json(&report);
            } else {
                for f in &report.findings {
                    if out.quiet && f.severity != crate::core::model::Severity::Error {
                        continue;
                    }
                    out.important(format!(
                        "{}:{}: {}: {}",
                        f.path.as_deref().unwrap_or("COMMIT_EDITMSG"),
                        f.line.unwrap_or(1),
                        f.rule_id,
                        f.message
                    ));
                }
                for d in &report.diagnostics {
                    out.warn(d);
                }
                if report.findings.is_empty() {
                    out.line("No findings.");
                }
            }
            if report.blocked() {
                exit_codes::BLOCKED
            } else {
                exit_codes::OK
            }
        }
        Err((code, error)) => {
            out.error(error);
            code
        }
    }
}
type Failure = (i32, String);
fn execute(
    files: &[String],
    message: Option<&str>,
    staged_input: bool,
    only: &[String],
    fix: bool,
    dry_run: bool,
    out: &mut Output,
) -> Result<EngineReport, Failure> {
    let usage = |e: &str| (exit_codes::USAGE, e.to_string());
    if usize::from(!files.is_empty()) + usize::from(message.is_some()) + usize::from(staged_input)
        != 1
    {
        return Err(usage(
            "select exactly one of --files, --message or --staged",
        ));
    }
    let cwd = std::env::current_dir().map_err(|e| (exit_codes::INTERNAL, e.to_string()))?;
    let root = Git::at(&cwd)
        .text(&["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or(cwd.clone());
    let root = root
        .canonicalize()
        .map_err(|e| (exit_codes::INTERNAL, e.to_string()))?;
    let registry = modules::registry();
    for id in only {
        if registry.find(id).is_none() {
            return Err(usage(&format!("unknown rule '{id}'")));
        }
    }
    let config = config::load(&registry, Some(&root), &config::CliOverrides::default())
        .map_err(|e| (exit_codes::CONFIG_ERROR, e.to_string()))?;
    let mut report = EngineReport::default();
    if staged_input && fix {
        let (report, preview) = crate::integrations::git::staged_fix::run(
            &Git::at(&root),
            &registry,
            &config,
            only,
            dry_run,
        )
        .map_err(|e| (exit_codes::CONFLICT, e))?;
        if dry_run && !out.json {
            out.raw(preview);
        }
        return Ok(report);
    }
    if staged_input {
        let input = staged::collect(&Git::at(&root)).map_err(|e| (exit_codes::INTERNAL, e))?;
        report.diagnostics.extend(input.skipped);
        for doc in input.documents {
            report.merge(
                staged::analyze(&Git::at(&root), &registry, &config, &doc, only)
                    .map_err(|e| (exit_codes::CONFIG_ERROR, e))?,
            );
        }
        if let Some(changes) = input.changes {
            report.merge(engine::run_changes(&registry, &config, &changes, only));
        }
        return Ok(report);
    }
    let inputs: Vec<&str> =
        message.map_or_else(|| files.iter().map(String::as_str).collect(), |m| vec![m]);
    for input in inputs {
        let given = Path::new(input);
        let path = if given.is_absolute() {
            given.to_path_buf()
        } else {
            cwd.canonicalize()
                .map_err(|e| (exit_codes::INTERNAL, e.to_string()))?
                .join(given)
        };
        let path = safety::resolve(&root, &path).map_err(|e| usage(&format!("{input}: {e}")))?;
        let label = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let (mut doc, bytes) =
            safety::read(&path, &label).map_err(|e| usage(&format!("{input}: {e}")))?;
        if message.is_some() {
            doc.kind = TargetKind::CommitMessage;
        }
        let config = crate::core::editorconfig::for_document(&config, &root, &doc)
            .map_err(|e| (exit_codes::CONFIG_ERROR, e))?;
        let initial = engine::run_document(&registry, &config, &doc, only);
        if !fix {
            report.merge(initial);
            continue;
        }
        // Each rule sees the previous rule's output, so plan hashes and offsets stay valid.
        let original = doc.text.clone();
        for rule in registry.all() {
            if !only.is_empty() && !only.iter().any(|id| id == rule.metadata().id) {
                continue;
            }
            let single =
                engine::run_document(&registry, &config, &doc, &[rule.metadata().id.to_string()]);
            for plan in single.fixes {
                doc.text =
                    planning::apply(&doc.text, &plan).map_err(|e| (exit_codes::INTERNAL, e))?;
            }
        }
        if doc.text != original {
            if dry_run {
                if !out.json {
                    out.raw(
                        similar::TextDiff::from_lines(&original, &doc.text)
                            .unified_diff()
                            .header(&label, &label)
                            .to_string(),
                    );
                }
                report.merge(initial);
                continue;
            }
            safety::replace(&root, &path, &bytes, &doc.text, doc.format.has_bom)
                .map_err(|e| (exit_codes::CONFLICT, e))?;
            report.diagnostics.push(format!("{label}: fixed"));
        }
        let effective = crate::core::editorconfig::for_document(&config, &root, &doc)
            .map_err(|e| (exit_codes::CONFIG_ERROR, e))?;
        report.merge(engine::run_document(&registry, &effective, &doc, only));
    }
    Ok(report)
}
