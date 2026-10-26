//! Ordered file/config steps with rollback: if any step fails, the steps already done are undone in
//! reverse order, so an install or uninstall never leaves a half-written hooks directory.

use std::path::{Path, PathBuf};

use super::command::Git;

#[derive(Debug, Clone)]
pub enum Step {
    /// Atomic write (temp file + rename); `executable` sets 0755 on Unix.
    Write {
        path: PathBuf,
        bytes: Vec<u8>,
        executable: bool,
    },
    /// Rename inside the same directory (keeps bytes, mode and timestamps).
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Delete {
        path: PathBuf,
    },
    /// `git config --global <key> <value>`.
    SetGlobalConfig {
        key: String,
        value: String,
    },
    /// `git config --global --unset <key>`.
    UnsetGlobalConfig {
        key: String,
    },
    RemoveDirIfEmpty {
        path: PathBuf,
    },
}

enum Undo {
    Restore {
        path: PathBuf,
        previous: Option<Vec<u8>>,
        permissions: Option<std::fs::Permissions>,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Config {
        key: String,
        previous: Option<String>,
    },
    CreateDir(PathBuf),
}

/// Writes `bytes` to `path` through a sibling temp file and a rename.
pub fn atomic_write(path: &Path, bytes: &[u8], executable: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
    temporary.write_all(bytes)?;
    if let Ok(meta) = std::fs::metadata(path) {
        temporary.as_file().set_permissions(meta.permissions())?;
    }
    if executable {
        set_executable(temporary.path())?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Unix permission bits, `None` elsewhere.
pub fn file_mode(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .ok()
            .map(|m| m.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

pub fn read_optional(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn global_value(git: &Git, key: &str) -> Result<Option<String>, String> {
    Ok(git
        .config_values(&["--global", "--get", key])?
        .into_iter()
        .next())
}

fn run_step(step: &Step, git: &Git) -> Result<Undo, String> {
    let io = |e: std::io::Error, what: &Path| format!("{}: {e}", what.display());
    match step {
        Step::Write {
            path,
            bytes,
            executable,
        } => {
            let previous = read_optional(path).map_err(|e| io(e, path))?;
            let permissions = std::fs::metadata(path).ok().map(|m| m.permissions());
            atomic_write(path, bytes, *executable).map_err(|e| io(e, path))?;
            Ok(Undo::Restore {
                path: path.clone(),
                previous,
                permissions,
            })
        }
        Step::Move { from, to } => {
            if to.exists() {
                return Err(format!("{} already exists", to.display()));
            }
            std::fs::rename(from, to).map_err(|e| io(e, from))?;
            Ok(Undo::Move {
                from: to.clone(),
                to: from.clone(),
            })
        }
        Step::Delete { path } => {
            let previous = read_optional(path).map_err(|e| io(e, path))?;
            let permissions = std::fs::metadata(path).ok().map(|m| m.permissions());
            if previous.is_some() {
                std::fs::remove_file(path).map_err(|e| io(e, path))?;
            }
            Ok(Undo::Restore {
                path: path.clone(),
                previous,
                permissions,
            })
        }
        Step::SetGlobalConfig { key, value } => {
            let previous = global_value(git, key)?;
            git.text(&["config", "--global", key, value])?;
            Ok(Undo::Config {
                key: key.clone(),
                previous,
            })
        }
        Step::UnsetGlobalConfig { key } => {
            let previous = global_value(git, key)?;
            if previous.is_some() {
                git.text(&["config", "--global", "--unset-all", key])?;
            }
            Ok(Undo::Config {
                key: key.clone(),
                previous,
            })
        }
        Step::RemoveDirIfEmpty { path } => {
            let empty = std::fs::read_dir(path)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if empty {
                std::fs::remove_dir(path).map_err(|e| io(e, path))?;
                Ok(Undo::CreateDir(path.clone()))
            } else {
                Ok(Undo::CreateDir(PathBuf::new()))
            }
        }
    }
}

fn undo(action: Undo, git: &Git) {
    match action {
        Undo::Restore {
            path,
            previous,
            permissions,
        } => match previous {
            Some(bytes) => {
                if atomic_write(&path, &bytes, false).is_ok()
                    && let Some(p) = permissions
                {
                    let _ = std::fs::set_permissions(&path, p);
                }
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        },
        Undo::Move { from, to } => {
            let _ = std::fs::rename(from, to);
        }
        Undo::Config { key, previous } => {
            let _ = match previous {
                Some(v) => git.text(&["config", "--global", &key, &v]),
                None => git.text(&["config", "--global", "--unset-all", &key]),
            };
        }
        Undo::CreateDir(path) => {
            if !path.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(path);
            }
        }
    }
}

/// Runs `steps` in order; on the first failure undoes what was done and returns the error.
pub fn execute(steps: &[Step], git: &Git) -> Result<(), String> {
    let mut done = Vec::new();
    for step in steps {
        match run_step(step, git) {
            Ok(u) => done.push(u),
            Err(e) => {
                for u in done.into_iter().rev() {
                    undo(u, git);
                }
                return Err(format!("{e} (every change was rolled back)"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_back_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, "original").unwrap();
        std::fs::write(&b, "occupied").unwrap();
        let steps = vec![
            Step::Write {
                path: a.clone(),
                bytes: b"new".to_vec(),
                executable: true,
            },
            Step::Write {
                path: dir.path().join("c"),
                bytes: b"c".to_vec(),
                executable: false,
            },
            // Fails: destination exists.
            Step::Move {
                from: a.clone(),
                to: b.clone(),
            },
        ];
        let err = execute(&steps, &Git::at(dir.path())).unwrap_err();
        assert!(err.contains("rolled back"));
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "original");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "occupied");
        assert!(!dir.path().join("c").exists());
    }
}
