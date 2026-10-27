//! Project-only native host bridges, installed transactionally with drift protection.
use super::agent_install::{digest, read, safe_path};
use super::git::{
    command::Git,
    transaction::{self, Step},
};
use crate::core::integration::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Installer for OpenCode 1.x plugins and DeepSeek Harness Cordis overlays.
pub struct BridgeInstaller(pub &'static str);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owned {
    name: String,
    original: Option<Vec<u8>>,
    hash: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    provider: String,
    modules: Vec<String>,
    files: Vec<Owned>,
}
impl BridgeInstaller {
    fn directory(&self, root: &Path) -> PathBuf {
        if self.0 == "opencode" {
            root.join(".opencode/plugins")
        } else {
            root.join(".dsh")
        }
    }
    fn names(&self) -> Vec<&'static str> {
        if self.0 == "opencode" {
            vec!["reporinse.js"]
        } else {
            vec!["reporinse.mjs", "reporinse.cordis.yml"]
        }
    }
    fn manifest(&self, dir: &Path) -> Result<Option<Manifest>, String> {
        read(&dir.join("reporinse-manifest.json"))?
            .map(|bytes| {
                let m: Manifest =
                    serde_json::from_slice(&bytes).map_err(|_| "invalid ownership manifest")?;
                let names = self.names();
                if m.version != 1
                    || m.provider != self.0
                    || m.files.len() != names.len()
                    || m.files.iter().zip(names).any(|(f, n)| f.name != n)
                {
                    return Err("invalid bridge ownership manifest".into());
                }
                Ok(m)
            })
            .transpose()
    }
    fn change(&self, req: &InstallRequest, remove: bool) -> Result<InstallResult, String> {
        if req.scope != InstallScope::Repo {
            return Err("native bridges support repo scope only".into());
        }
        let root = Git::at(&req.root)
            .text(&["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .map_err(|e| e.to_string())?;
        let dir = self.directory(&root);
        let manifest_path = dir.join("reporinse-manifest.json");
        let snapshot = read(&manifest_path)?;
        let previous = self.manifest(&dir)?;
        let mut current = Vec::new();
        for name in self.names() {
            let bytes = read(&dir.join(name))?;
            if let Some(m) = &previous {
                let owned = m
                    .files
                    .iter()
                    .find(|f| f.name == name)
                    .ok_or("manifest file missing")?;
                if bytes.as_deref().map(digest).as_deref() != Some(&owned.hash) && !req.force {
                    return Err("bridge changed since install; refusing to overwrite".into());
                }
            } else if bytes.is_some() {
                return Err(
                    "foreign bridge file exists; choose another name before installing".into(),
                );
            }
            current.push(bytes);
        }
        if remove && previous.is_none() {
            return Ok(InstallResult::default());
        }
        let mut modules = previous
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
            modules.extend(req.modules.clone());
            modules.sort();
            modules.dedup();
        }
        let mut steps = Vec::new();
        let mut result = InstallResult::default();
        let names = self.names();
        if modules.is_empty() {
            for name in names {
                let original = previous
                    .as_ref()
                    .and_then(|m| m.files.iter().find(|f| f.name == name))
                    .and_then(|f| f.original.clone());
                steps.push(match original {
                    Some(bytes) => Step::Write {
                        path: dir.join(name),
                        bytes,
                        executable: false,
                    },
                    None => Step::Delete {
                        path: dir.join(name),
                    },
                });
            }
            steps.push(Step::Delete {
                path: manifest_path.clone(),
            });
        } else {
            let binary = std::env::current_exe().map_err(|e| e.to_string())?;
            let settings = json!({"binary":binary,"modules":modules,"root":root});
            let source = include_str!("agent_bridge.js");
            let split = source
                .find("/** DeepSeek Harness")
                .ok_or("bridge template marker missing")?;
            let common_end = source
                .find("/** OpenCode")
                .ok_or("bridge template marker missing")?;
            let body = if self.0 == "opencode" {
                source[..split].to_string()
            } else {
                format!("{}{}", &source[..common_end], &source[split..])
            };
            let javascript = format!("const SETTINGS = {};\n{}", settings, body);
            let mut files = vec![javascript.into_bytes()];
            if self.0 == "deepseek" {
                let url = url_for_file(&dir.join("reporinse.mjs"))?;
                files.push(
                    serde_json::to_vec_pretty(&json!([{"id":"reporinse", "name":url}]))
                        .map_err(|e| e.to_string())?,
                );
                result.notes.push("Start DeepSeek with dsh web --patch .dsh/reporinse.cordis.yml; install @deepseek-ai/dsh 0.2.1-rc.2. Paths and policies use this repository root.".into());
            }
            let owned = names
                .iter()
                .zip(files.iter())
                .map(|(name, bytes)| Owned {
                    name: (*name).into(),
                    original: None,
                    hash: digest(bytes),
                })
                .collect();
            for (name, bytes) in names.iter().zip(files) {
                steps.push(Step::Write {
                    path: dir.join(name),
                    bytes,
                    executable: false,
                });
            }
            let manifest = Manifest {
                version: 1,
                provider: self.0.into(),
                modules,
                files: owned,
            };
            steps.push(Step::Write {
                path: manifest_path.clone(),
                bytes: serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
                executable: false,
            });
        }
        for step in &steps {
            let path = match step {
                Step::Write { path, .. } | Step::Delete { path } => path,
                _ => return Err("unexpected bridge operation".into()),
            };
            safe_path(path)?;
            result.actions.push(InstallAction {
                kind: if remove {
                    InstallActionKind::DeleteFile
                } else {
                    InstallActionKind::CreateFile
                },
                target: path.display().to_string(),
                description: "Owned native host bridge; drift protected".into(),
            });
        }
        if !req.dry_run {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let lock_path = dir.join("reporinse-install.lock");
            let lock = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
                .map_err(|_| "bridge installation locked")?;
            struct Lock(PathBuf, Option<fs::File>);
            impl Drop for Lock {
                fn drop(&mut self) {
                    self.1.take();
                    let _ = fs::remove_file(&self.0);
                }
            }
            let _lock = Lock(lock_path, Some(lock));
            if read(&manifest_path)? != snapshot {
                return Err("manifest changed while planning".into());
            }
            for (name, before) in self.names().iter().zip(current) {
                if read(&dir.join(name))? != before {
                    return Err("bridge changed while planning".into());
                }
            }
            transaction::execute(&steps, &Git::at(&root))?;
        }
        Ok(result)
    }
}
// File URL escaping avoids interpreting spaces, #, %, or non-ASCII paths as loader syntax.
fn url_for_file(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or("plugin path is not UTF-8")?
        .replace('\\', "/");
    let mut url = if text.starts_with('/') {
        "file://".to_string()
    } else {
        "file:///".to_string()
    };
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~:".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(url)
}
impl Installer for BridgeInstaller {
    fn provider(&self) -> &'static str {
        self.0
    }
    fn supported_scopes(&self) -> &'static [InstallScope] {
        &[InstallScope::Repo]
    }
    fn status(&self, scope: InstallScope, root: &Path) -> IntegrationStatus {
        let dir = self.directory(root);
        let mut status = IntegrationStatus {
            provider: self.0.into(),
            scope,
            detected: dir.exists(),
            installed: false,
            modules: vec![],
            problems: vec![],
        };
        if scope != InstallScope::Repo {
            return status;
        }
        match self.manifest(&dir) {
            Ok(Some(m)) => {
                status.installed = true;
                status.modules = m.modules;
                for f in m.files {
                    if read(&dir.join(f.name))
                        .ok()
                        .flatten()
                        .as_deref()
                        .map(digest)
                        .as_deref()
                        != Some(&f.hash)
                    {
                        status.problems.push("bridge drifted".into());
                    }
                }
            }
            Ok(None) => (),
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
