//! What RepoRinse installed for Git, so uninstall removes only its own files and restores the rest exactly.
//!
//! Location: repo scope `<git-common-dir>/reporinse/git-manifest.json` (outside any versioned hooks directory,
//! shared by linked worktrees); user scope `<reporinse-home>/git-hooks/.reporinse-manifest.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FORMAT: u32 = 1;

/// A hook file RepoRinse wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedFile {
    pub hook: String,
    /// File name inside `hooks_dir`.
    pub path: String,
    pub sha256: String,
}

/// A hook that existed before and now runs first through the shim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainedHook {
    pub hook: String,
    /// Original file name inside `hooks_dir` (where it is restored).
    pub original: String,
    /// Where it was moved, inside `hooks_dir`.
    pub stored_as: String,
    pub sha256: String,
    /// Unix permission bits at install time (`None` on Windows). Restoring is a rename, so they are kept anyway.
    pub mode: Option<u32>,
    /// Tool that generated it, when recognised (husky, lefthook, pre-commit, no-ai-coauthor).
    pub tool: Option<String>,
}

/// A Git configuration value RepoRinse set, with what was there before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigChange {
    /// `global` (user scope) or `local`.
    pub scope: String,
    pub key: String,
    pub value: String,
    /// `None` = the key was unset; uninstall unsets it again.
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub tool: String,
    /// RepoRinse version that wrote it.
    pub version: String,
    pub provider: String,
    /// `repo` or `user`.
    pub scope: String,
    /// Absolute hooks directory, forward slashes.
    pub hooks_dir: String,
    /// Binary recorded in the shims.
    pub binary: String,
    /// Modules the hooks run.
    pub modules: Vec<String>,
    /// RFC 3339 UTC time of the last change.
    pub installed_at: String,
    pub files: Vec<ManagedFile>,
    pub chained: Vec<ChainedHook>,
    pub config: Vec<ConfigChange>,
    /// Hook managers recognised in the repository at install time.
    pub detected: Vec<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Option<Manifest>, String> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{} is not a valid RepoRinse manifest: {e}", path.display()))?;
        if manifest.format != FORMAT {
            return Err(format!(
                "{} has manifest format {}, this build understands {FORMAT}",
                path.display(),
                manifest.format
            ));
        }
        Ok(Some(manifest))
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("serializable manifest");
        text.push('\n');
        text
    }

    pub fn file(&self, hook: &str) -> Option<&ManagedFile> {
        self.files.iter().find(|f| f.hook == hook)
    }

    pub fn chained(&self, hook: &str) -> Option<&ChainedHook> {
        self.chained.iter().find(|c| c.hook == hook)
    }

    /// Same installation apart from the timestamp.
    pub fn same_content(&self, other: &Manifest) -> bool {
        let mut a = self.clone();
        a.installed_at = other.installed_at.clone();
        &a == other
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ` (no date crate needed).
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_752), (2026, 10, 26));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn round_trip() {
        let m = Manifest {
            format: FORMAT,
            tool: "reporinse".into(),
            version: "0.1.0".into(),
            provider: "git".into(),
            scope: "repo".into(),
            hooks_dir: "/r/.git/hooks".into(),
            binary: "/bin/reporinse".into(),
            modules: vec!["git.ai-attribution".into()],
            installed_at: now_rfc3339(),
            files: vec![],
            chained: vec![],
            config: vec![],
            detected: vec![],
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        std::fs::write(&path, m.to_json()).unwrap();
        assert_eq!(Manifest::load(&path).unwrap(), Some(m));
        assert_eq!(Manifest::load(&dir.path().join("none.json")).unwrap(), None);
    }
}
