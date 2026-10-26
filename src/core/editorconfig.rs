//! Resolve trim_trailing_whitespace without moving filesystem I/O into detectors.
use super::{
    config::EffectiveConfig,
    model::{RuleMode, SourceDocument},
};
use crate::integrations::git::command::Git;
use std::path::Path;

pub fn for_document(
    config: &EffectiveConfig,
    root: &Path,
    doc: &SourceDocument,
) -> Result<EffectiveConfig, String> {
    let mut effective = config.clone();
    let settings = config.settings("text.whitespace");
    if settings.mode == RuleMode::Off || settings.bool("honor_editorconfig") == Some(false) {
        return Ok(effective);
    }
    let mut ancestors = Vec::new();
    let mut dir = Path::new(&doc.path).parent();
    while let Some(p) = dir {
        ancestors.push(p.to_path_buf());
        dir = p.parent();
    }
    ancestors.reverse();
    let mut trim = None;
    for directory in ancestors {
        let relative = directory.join(".editorconfig");
        let contents = if doc.staged {
            Git::at(root)
                .bytes(
                    &[
                        "show",
                        &format!(":{}", relative.to_string_lossy().replace('\\', "/")),
                    ],
                    None,
                )
                .ok()
        } else {
            let candidate = root.join(&relative);
            match std::fs::symlink_metadata(&candidate) {
                Ok(_) => {
                    let path = super::safety::resolve(root, &candidate)?;
                    Some(super::safety::read(&path, ".editorconfig")?.1)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            }
        };
        let Some(bytes) = contents else { continue };
        if bytes.len() > 65536 {
            return Err("EditorConfig exceeds 64 KiB".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "EditorConfig is not UTF-8")?;
        let relative_file = Path::new(&doc.path)
            .strip_prefix(&directory)
            .map_err(|_| "invalid EditorConfig path")?
            .to_string_lossy()
            .replace('\\', "/");
        let mut matches = false;
        let mut in_section = false;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(['#', ';']) {
                continue;
            }
            if let Some(pattern) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                in_section = true;
                if pattern.contains("..") && pattern.contains('{') {
                    return Err("numeric EditorConfig ranges are unsupported; disable honor_editorconfig to proceed".into());
                }
                let pattern = pattern.trim_start_matches('/');
                let target = if pattern.contains('/') {
                    relative_file.as_str()
                } else {
                    relative_file.rsplit('/').next().unwrap_or("")
                };
                matches = globset::GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .build()
                    .map_err(
                        |_| "unsupported EditorConfig glob; disable honor_editorconfig to proceed",
                    )?
                    .compile_matcher()
                    .is_match(target);
            } else if let Some((key, value)) = line.split_once('=') {
                if !in_section
                    && key.trim().eq_ignore_ascii_case("root")
                    && value.trim().eq_ignore_ascii_case("true")
                {
                    trim = None;
                }
                if key.trim().eq_ignore_ascii_case("trim_trailing_whitespace") && matches {
                    trim = match value.trim().to_lowercase().as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        "unset" => None,
                        _ => return Err("invalid EditorConfig trim_trailing_whitespace".into()),
                    };
                }
            }
        }
    }
    if trim == Some(false) {
        let mut settings = settings;
        settings.mode = RuleMode::Off;
        effective.set("text.whitespace", settings, "editorconfig");
    }
    Ok(effective)
}
