//! Merge only owned hook entries. A drifted settings file is never silently overwritten.
use super::git::{
    command::{Git, slash_path},
    transaction::{self, Step},
};
use crate::core::integration::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub struct AgentInstaller(pub &'static str);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    provider: String,
    original: Option<Vec<u8>>,
    installed_hash: String,
    modules: Vec<String>,
}
pub(super) fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or("cannot resolve user home".into())
}
fn locations(
    provider: &str,
    scope: InstallScope,
    root: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    let base = if scope == InstallScope::Repo {
        Git::at(root)
            .text(&["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or(root.to_path_buf())
    } else {
        home()?
    };
    let directory = if scope == InstallScope::User {
        match provider {
            "claude" => std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            "codex" => std::env::var_os("CODEX_HOME").map(PathBuf::from),
            _ => None,
        }
        .unwrap_or_else(|| base.join(format!(".{provider}")))
    } else {
        base.join(format!(".{provider}"))
    };
    let filename = if matches!(provider, "claude" | "gemini") {
        "settings.json"
    } else {
        "hooks.json"
    };
    Ok((
        directory.join(filename),
        directory.join("reporinse-manifest.json"),
    ))
}
pub(super) fn safe_path(path: &Path) -> Result<(), String> {
    let mut cursor = Some(path);
    while let Some(p) = cursor {
        match fs::symlink_metadata(p) {
            Ok(m) if crate::core::safety::is_link(&m) => {
                return Err("symlink settings paths are refused".into());
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => (),
        };
        cursor = p.parent();
    }
    Ok(())
}
pub(super) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    safe_path(path)?;
    match fs::metadata(path) {
        Ok(m) if m.len() > 1024 * 1024 => return Err("settings or manifest exceeds 1 MiB".into()),
        _ => (),
    }
    transaction::read_optional(path).map_err(|e| e.to_string())
}
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
fn powershell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn windows_command(script: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        encoded.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        encoded.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    format!("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}")
}
fn registration(provider: &str, modules: &[String]) -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let binary = slash_path(&exe);
    let events: &[&str] = match provider {
        "claude" | "codex" => &["PreToolUse", "PostToolUse"],
        "gemini" => &["BeforeTool", "AfterTool"],
        "cursor" => &["beforeShellExecution", "beforeReadFile", "afterFileEdit"],
        _ => return Err("unsupported provider".into()),
    };
    let mut hooks = serde_json::Map::new();
    for event in events {
        let args = vec![
            "hook".to_string(),
            "--only".into(),
            modules.join(","),
            provider.into(),
            event.to_string(),
        ];
        let posix = std::iter::once(binary.as_str())
            .chain(args.iter().map(String::as_str))
            .map(shell_quote)
            .collect::<Vec<_>>()
            .join(" ");
        let ps = format!(
            "& {}",
            std::iter::once(binary.as_str())
                .chain(args.iter().map(String::as_str))
                .map(powershell_quote)
                .collect::<Vec<_>>()
                .join(" ")
        );
        let ps = windows_command(&format!("{ps}; exit $LASTEXITCODE"));
        let handler = match provider {
            "claude" => json!({"type":"command","command":exe,"args":args,"timeout":10}),
            "codex" => json!({"type":"command","command":posix,"commandWindows":ps,"timeout":10}),
            "gemini" => {
                json!({"type":"command","name":"RepoRinse","command":if cfg!(windows){ps}else{posix},"timeout":10000})
            }
            _ => json!({"command":if cfg!(windows){ps}else{posix},"timeout":10}),
        };
        let entry = if provider == "cursor" {
            handler
        } else {
            json!({"matcher":if provider=="gemini" {".*"} else {"*"},"hooks":[handler]})
        };
        hooks.insert(event.to_string(), json!([entry]));
    }
    Ok(Value::Object(hooks))
}
impl AgentInstaller {
    fn change(&self, req: &InstallRequest, remove: bool) -> Result<InstallResult, String> {
        let (settings, manifest_path) = locations(self.0, req.scope, &req.root)?;
        let current = read(&settings)?;
        let manifest_bytes = read(&manifest_path)?;
        let manifest: Option<Manifest> = manifest_bytes
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()
            .map_err(|_| "invalid ownership manifest")?;
        if let Some(m) = &manifest {
            if m.version != 1 || m.provider != self.0 {
                return Err("ownership manifest does not match provider".into());
            }
            if current.as_deref().map(digest).as_deref() != Some(&m.installed_hash) && !req.force {
                return Err("settings changed since install; refusing to overwrite; inspect changes or use --force".into());
            }
        }
        if remove && manifest.is_none() {
            return Ok(InstallResult {
                notes: vec!["RepoRinse is not installed".into()],
                ..Default::default()
            });
        }
        let original = manifest
            .as_ref()
            .map(|m| m.original.clone())
            .unwrap_or_else(|| current.clone());
        let mut modules = manifest
            .as_ref()
            .map(|m| m.modules.clone())
            .unwrap_or_default();
        if remove {
            if req.modules.is_empty() {
                modules.clear();
            } else {
                modules.retain(|m| !req.modules.contains(m));
            }
        } else {
            for id in &req.modules {
                if !modules.contains(id) {
                    modules.push(id.clone());
                }
            }
        }
        modules.sort();
        modules.dedup();
        let mut steps = Vec::new();
        let mut result = InstallResult::default();
        if modules.is_empty() {
            if let Some(bytes) = original {
                steps.push(Step::Write {
                    path: settings.clone(),
                    bytes,
                    executable: false,
                });
            } else {
                steps.push(Step::Delete {
                    path: settings.clone(),
                });
            }
            steps.push(Step::Delete {
                path: manifest_path.clone(),
            });
            result.actions.push(InstallAction {
                kind: InstallActionKind::RestoreFile,
                target: settings.display().to_string(),
                description: "Restore exact settings bytes from before installation".into(),
            });
        } else {
            let mut value: Value = original
                .as_deref()
                .map(|b| serde_json::from_slice(b).map_err(|_| "settings contain invalid JSON"))
                .transpose()?
                .unwrap_or(json!({}));
            let object = value
                .as_object_mut()
                .ok_or("settings must be a JSON object")?;
            if self.0 == "claude" {
                object.insert("attribution".into(), json!({"commit":"", "pr":""}));
            }
            if self.0 == "cursor" {
                if object.get("version").is_some_and(|v| v != &json!(1)) {
                    return Err("unsupported Cursor hooks version".into());
                }
                object.insert("version".into(), json!(1));
            }
            let hooks = object
                .entry("hooks")
                .or_insert(json!({}))
                .as_object_mut()
                .ok_or("settings hooks must be an object")?;
            for (event, entries) in registration(self.0, &modules)?
                .as_object()
                .ok_or("invalid registration")?
            {
                let list = hooks
                    .entry(event.clone())
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or("hook event entries must be arrays")?;
                list.extend(
                    entries
                        .as_array()
                        .ok_or("invalid hook entries")?
                        .iter()
                        .cloned(),
                );
            }
            let mut bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            let m = Manifest {
                version: 1,
                provider: self.0.into(),
                original,
                installed_hash: digest(&bytes),
                modules,
            };
            steps.push(Step::Write {
                path: settings.clone(),
                bytes,
                executable: false,
            });
            steps.push(Step::Write {
                path: manifest_path.clone(),
                bytes: serde_json::to_vec_pretty(&m).map_err(|e| e.to_string())?,
                executable: false,
            });
            result.actions.push(InstallAction {
                kind: InstallActionKind::MergeJson,
                target: settings.display().to_string(),
                description: "Merge RepoRinse hooks; preserve foreign hooks and settings".into(),
            });
        }
        if self.0 == "codex" {
            result.notes.push("Review and trust each installed hook in Codex /hooks; project hooks also require a trusted project. RepoRinse never changes trust settings.".into());
        }
        if matches!(self.0, "claude" | "cursor") {
            result.notes.push("Cursor can also load Claude hooks: avoid installing both for the same rules, or disable third-party hook loading in Cursor.".into());
        }
        result.notes.push("Use block mode explicitly to veto matched pre-tool actions. Fail-closed is not enabled; guardrails do not replace the host sandbox or approvals.".into());
        if !req.dry_run {
            fs::create_dir_all(settings.parent().ok_or("settings parent")?)
                .map_err(|e| e.to_string())?;
            let lock_path = settings.with_extension("reporinse.lock");
            let lock = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
                .map_err(|_| "settings install is locked")?;
            struct Lock(PathBuf, Option<fs::File>);
            impl Drop for Lock {
                fn drop(&mut self) {
                    self.1.take();
                    let _ = fs::remove_file(&self.0);
                }
            }
            let _lock = Lock(lock_path, Some(lock));
            if read(&settings)? != current || read(&manifest_path)? != manifest_bytes {
                return Err("settings changed while planning".into());
            }
            transaction::execute(&steps, &Git::at(&req.root))?;
        }
        Ok(result)
    }
}
impl Installer for AgentInstaller {
    fn provider(&self) -> &'static str {
        self.0
    }
    fn supported_scopes(&self) -> &'static [InstallScope] {
        &[InstallScope::Repo, InstallScope::User]
    }
    fn status(&self, scope: InstallScope, root: &Path) -> IntegrationStatus {
        let mut status = IntegrationStatus {
            provider: self.0.into(),
            scope,
            detected: false,
            installed: false,
            modules: Vec::new(),
            problems: Vec::new(),
        };
        match locations(self.0, scope, root).and_then(|(settings, manifest)| {
            status.detected = settings.exists();
            if let Some(bytes) = read(&manifest)? {
                let m: Manifest =
                    serde_json::from_slice(&bytes).map_err(|_| "invalid ownership manifest")?;
                if m.version != 1 || m.provider != self.0 {
                    return Err("ownership manifest does not match provider".into());
                }
                status.installed = true;
                status.modules = m.modules;
                if read(&settings)?.as_deref().map(digest).as_deref() != Some(&m.installed_hash) {
                    status
                        .problems
                        .push("settings drifted since installation".into());
                }
            }
            Ok(())
        }) {
            Ok(()) => (),
            Err(e) => status.problems.push(e),
        }
        status
    }
    fn install(&self, req: &InstallRequest) -> InstallResult {
        self.change(req, false).unwrap_or_else(|e| InstallResult {
            conflicts: vec![e],
            ..Default::default()
        })
    }
    fn uninstall(&self, req: &InstallRequest) -> InstallResult {
        self.change(req, true).unwrap_or_else(|e| InstallResult {
            conflicts: vec![e],
            ..Default::default()
        })
    }
}
