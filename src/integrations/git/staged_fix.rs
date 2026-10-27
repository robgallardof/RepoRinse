//! Explicit staged corrections. Hold Git's index lock; refuse partial staging and stale inputs.
use super::{command::Git, staged};
use crate::core::{
    config::EffectiveConfig,
    engine::{self, EngineReport},
    planning,
    registry::RuleRegistry,
    safety,
};
use std::{fs, io::Write, path::Path};

pub fn run(
    git: &Git,
    registry: &RuleRegistry,
    config: &EffectiveConfig,
    only: &[String],
    dry_run: bool,
) -> Result<(EngineReport, String), String> {
    let input = staged::collect(git)?;
    let mut report = EngineReport::default();
    report.diagnostics.extend(input.skipped);
    let mut updates = Vec::new();
    let mut preview = String::new();
    for mut doc in input.documents {
        let before = doc.text.clone();
        let initial = staged::analyze(git, registry, config, &doc, only)?;
        for rule in registry.all() {
            let id = rule.metadata().id;
            if !only.is_empty() && !only.iter().any(|s| s == id) {
                continue;
            }
            let step = staged::analyze(git, registry, config, &doc, &[id.into()])?;
            for plan in step.fixes {
                doc.text = planning::apply(&doc.text, &plan)?;
            }
        }
        if doc.text != before {
            let path = safety::resolve(git.dir(), Path::new(&doc.path))?;
            let (_, original) = safety::read(&path, &doc.path)?;
            let mut staged_bytes = if doc.format.has_bom {
                b"\xef\xbb\xbf".to_vec()
            } else {
                Vec::new()
            };
            staged_bytes.extend_from_slice(before.as_bytes());
            if original != staged_bytes
                || !git
                    .bytes(
                        &[
                            // Diff's stat refresh must not write before the index transaction.
                            "-c",
                            "diff.autoRefreshIndex=false",
                            "diff",
                            "--no-ext-diff",
                            "--no-textconv",
                            "--numstat",
                            "--",
                            &doc.path,
                        ],
                        None,
                    )?
                    .is_empty()
            {
                return Err(format!(
                    "{}: unstaged changes or checkout encoding differ; staged fix refused",
                    doc.path
                ));
            }
            preview.push_str(
                &similar::TextDiff::from_lines(&before, &doc.text)
                    .unified_diff()
                    .header(&doc.path, &doc.path)
                    .to_string(),
            );
            let mode = git.bytes(&["ls-files", "--stage", "-z", "--", &doc.path], None)?;
            let mode = std::str::from_utf8(&mode)
                .map_err(|_| "invalid index entry")?
                .split(' ')
                .next()
                .ok_or("missing index mode")?
                .to_string();
            let mut replacement = if doc.format.has_bom {
                b"\xef\xbb\xbf".to_vec()
            } else {
                Vec::new()
            };
            replacement.extend_from_slice(doc.text.as_bytes());
            updates.push((
                path,
                doc.path.clone(),
                original,
                replacement,
                doc.text.clone(),
                doc.format.has_bom,
                mode,
            ));
            report.diagnostics.push(format!(
                "{}: {}",
                doc.path,
                if dry_run {
                    "would fix staged content"
                } else {
                    "fixed staged content"
                }
            ));
        }
        report.merge(if dry_run {
            initial
        } else {
            staged::analyze(git, registry, config, &doc, only)?
        });
    }
    if let Some(changes) = input.changes {
        report.merge(engine::run_changes(registry, config, &changes, only));
    }
    if dry_run || updates.is_empty() {
        return Ok((report, preview));
    }
    let index = std::path::PathBuf::from(git.text(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "index",
    ])?);
    let lock_path = index.with_file_name(format!(
        "{}.lock",
        index.file_name().ok_or("index path")?.to_string_lossy()
    ));
    let lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|_| "Git index is locked; retry after the other Git operation")?;
    struct Lock(std::path::PathBuf, Option<fs::File>);
    impl Drop for Lock {
        fn drop(&mut self) {
            self.1.take();
            let _ = fs::remove_file(&self.0);
        }
    }
    let _lock = Lock(lock_path, Some(lock));
    let original_index = fs::read(&index).map_err(|e| e.to_string())?;
    // Validate snapshot again under the lock; another process may have staged changes while rules ran.
    for (_, label, original, _, _, _, mode) in &updates {
        if git.bytes(&["show", &format!(":{label}")], None)? != *original {
            return Err("index changed since analysis; staged fix refused".into());
        }
        let entry = git.bytes(&["ls-files", "--stage", "-z", "--", label], None)?;
        if !entry.starts_with(format!("{mode} ").as_bytes()) {
            return Err("index mode changed since analysis".into());
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(index.parent().ok_or("index parent")?)
        .map_err(|e| e.to_string())?;
    temporary
        .write_all(&original_index)
        .map_err(|e| e.to_string())?;
    let temporary_git = git.clone().env("GIT_INDEX_FILE", temporary.path());
    for (_, label, _, replacement, _, _, mode) in &updates {
        let blob = git.bytes(&["hash-object", "-w", "--stdin"], Some(replacement))?;
        let blob = std::str::from_utf8(&blob)
            .map_err(|_| "invalid blob id")?
            .trim();
        temporary_git.bytes(
            &["update-index", "--add", "--cacheinfo", mode, blob, label],
            None,
        )?;
    }
    let mut applied = Vec::new();
    let mut apply = || -> Result<(), String> {
        for (path, _, original, _, text, bom, _) in &updates {
            safety::replace(git.dir(), path, original, text, *bom)?;
            applied.push(path.clone());
        }
        if fs::read(&index).map_err(|e| e.to_string())? != original_index {
            return Err("index changed while locked".into());
        }
        // update-index may have replaced the temp path, so read its current bytes rather than its old handle.
        let bytes = fs::read(temporary.path()).map_err(|e| e.to_string())?;
        let mut final_index =
            tempfile::NamedTempFile::new_in(index.parent().ok_or("index parent")?)
                .map_err(|e| e.to_string())?;
        final_index.write_all(&bytes).map_err(|e| e.to_string())?;
        final_index
            .as_file()
            .set_permissions(
                fs::metadata(&index)
                    .map_err(|e| e.to_string())?
                    .permissions(),
            )
            .map_err(|e| e.to_string())?;
        final_index
            .as_file()
            .sync_all()
            .map_err(|e| e.to_string())?;
        final_index.persist(&index).map_err(|e| e.to_string())?;
        Ok(())
    };
    if let Err(error) = apply() {
        let mut failures = Vec::new();
        for (path, label, original, replacement, _, bom, _) in &updates {
            if !applied.contains(path) {
                continue;
            }
            if fs::read(path).ok().as_deref() == Some(replacement.as_slice()) {
                let body = if *bom {
                    &original[3..]
                } else {
                    original.as_slice()
                };
                if let Err(e) = safety::replace(
                    git.dir(),
                    path,
                    replacement,
                    std::str::from_utf8(body).map_err(|_| "invalid original text")?,
                    *bom,
                ) {
                    failures.push(format!("{label}: {e}"));
                }
            }
        }
        return Err(format!(
            "{error}; rollback diagnostics: {}",
            failures.join("; ")
        ));
    }
    Ok((report, preview))
}
