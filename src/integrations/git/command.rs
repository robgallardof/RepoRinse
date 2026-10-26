//! Running `git` as a child process. Every call is explicit about its directory; nothing is run through a shell.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `git -C <dir> …` runner.
#[derive(Debug, Clone)]
pub struct Git {
    dir: PathBuf,
    /// Extra environment for every call (tests use it to isolate the Git configuration).
    envs: Vec<(OsString, OsString)>,
}

impl Git {
    pub fn at(dir: impl Into<PathBuf>) -> Git {
        Git {
            dir: dir.into(),
            envs: Vec::new(),
        }
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Git {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.dir).args(args);
        cmd.envs(self.envs.iter().map(|(k, v)| (k, v)));
        cmd
    }

    /// Raw stdout bytes; Err carries git's stderr (or the spawn error).
    pub fn bytes(&self, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let mut cmd = self.command(args);
        cmd.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("cannot run git: {e}"))?;
        if let Some(input) = stdin {
            // Write from a thread so a large stdout cannot deadlock against a full stdin pipe.
            let mut pipe = child.stdin.take().expect("piped stdin");
            let input = input.to_vec();
            let writer = std::thread::spawn(move || pipe.write_all(&input));
            let output = child
                .wait_with_output()
                .map_err(|e| format!("git failed: {e}"))?;
            let _ = writer.join();
            return finish(args, output);
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("git failed: {e}"))?;
        finish(args, output)
    }

    /// Stdout as UTF-8 with the final newline removed.
    pub fn text(&self, args: &[&str]) -> Result<String, String> {
        let bytes = self.bytes(args, None)?;
        let text =
            String::from_utf8(bytes).map_err(|_| "git printed non UTF-8 output".to_string())?;
        Ok(text.strip_suffix('\n').unwrap_or(&text).to_string())
    }

    /// Values of a multi-valued config query; a missing key is an empty list (git exits 1 for it).
    pub fn config_values(&self, args: &[&str]) -> Result<Vec<String>, String> {
        let mut full = vec!["config"];
        full.extend_from_slice(args);
        let output = self
            .command(&full)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        match output.status.code() {
            Some(0) => Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_string)
                .collect()),
            Some(1) => Ok(Vec::new()),
            _ => Err(stderr_of(&output)),
        }
    }
}

fn stderr_of(output: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if err.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        err
    }
}

fn finish(args: &[&str], output: std::process::Output) -> Result<Vec<u8>, String> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!("git {}: {}", args.join(" "), stderr_of(&output)))
    }
}

/// Path as Git and `sh` print it: forward slashes (Windows drive letters stay `C:/…`).
pub fn slash_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    s.replace('\\', "/")
}

/// Comparable form of a path: forward slashes, no trailing slash, case-folded on Windows.
pub fn path_key(path: &str) -> String {
    let mut s = path.replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    if cfg!(windows) { s.to_lowercase() } else { s }
}
