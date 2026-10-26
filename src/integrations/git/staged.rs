//! Read-only view of the index for `pre-commit`: staged paths, their line counts and their *staged* content
//! (never the working tree). Nothing here writes to the index or the working tree.

use super::command::Git;
use crate::core::model::{ChangeSet, ChangeStatus, ChangedPath, SourceDocument};

/// Files larger than this are reported and skipped.
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

/// What `pre-commit` analyses.
#[derive(Debug, Default)]
pub struct Staged {
    pub documents: Vec<SourceDocument>,
    pub changes: Option<ChangeSet>,
    /// Skipped files and why (binary, non UTF-8, too large, symlink, submodule).
    pub skipped: Vec<String>,
}

struct RawEntry {
    new_mode: String,
    blob: String,
    status: ChangeStatus,
    path: String,
}

fn status_of(letter: &str) -> ChangeStatus {
    match letter.chars().next() {
        Some('A') => ChangeStatus::Added,
        Some('M') => ChangeStatus::Modified,
        Some('D') => ChangeStatus::Deleted,
        Some('R') => ChangeStatus::Renamed,
        Some('C') => ChangeStatus::Copied,
        Some('T') => ChangeStatus::TypeChanged,
        _ => ChangeStatus::Unknown,
    }
}

/// Parses `git diff --cached --raw -z --no-renames --no-abbrev`.
fn parse_raw(out: &[u8]) -> Vec<RawEntry> {
    let fields: Vec<&[u8]> = out.split(|b| *b == 0).collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i + 1 < fields.len() {
        let meta = String::from_utf8_lossy(fields[i]);
        let path = String::from_utf8_lossy(fields[i + 1]).to_string();
        i += 2;
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        if parts.len() < 5 {
            continue;
        }
        entries.push(RawEntry {
            new_mode: parts[1].to_string(),
            blob: parts[3].to_string(),
            status: status_of(parts[4]),
            path,
        });
    }
    entries
}

/// Parses `git diff --cached --numstat -z --no-renames` into (added, deleted, binary) per path.
fn parse_numstat(out: &[u8]) -> Vec<(String, usize, usize, bool)> {
    out.split(|b| *b == 0)
        .filter(|f| !f.is_empty())
        .filter_map(|f| {
            let line = String::from_utf8_lossy(f);
            let mut parts = line.splitn(3, '\t');
            let (a, d, p) = (parts.next()?, parts.next()?, parts.next()?);
            let binary = a == "-" || d == "-";
            Some((
                p.to_string(),
                a.parse().unwrap_or(0),
                d.parse().unwrap_or(0),
                binary,
            ))
        })
        .collect()
}

/// Reads blobs with one `git cat-file --batch`.
fn read_blobs(git: &Git, ids: &[String]) -> Result<Vec<Vec<u8>>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let input: String = ids.iter().map(|id| format!("{id}\n")).collect();
    let out = git.bytes(&["cat-file", "--batch"], Some(input.as_bytes()))?;
    let mut blobs = Vec::with_capacity(ids.len());
    let mut pos = 0usize;
    for _ in ids {
        let header_end = out[pos..]
            .iter()
            .position(|b| *b == b'\n')
            .ok_or("truncated cat-file output")?
            + pos;
        let header = String::from_utf8_lossy(&out[pos..header_end]);
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("unexpected cat-file header '{header}'"))?;
        let start = header_end + 1;
        let end = start + size;
        if end > out.len() {
            return Err("truncated cat-file output".to_string());
        }
        blobs.push(out[start..end].to_vec());
        pos = end + 1;
    }
    Ok(blobs)
}

/// Collects the staged documents and change set of the repository at `git`.
pub fn collect(git: &Git) -> Result<Staged, String> {
    let raw = git.bytes(
        &[
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--no-abbrev",
        ],
        None,
    )?;
    let numstat = git.bytes(
        &["diff", "--cached", "--numstat", "-z", "--no-renames"],
        None,
    )?;
    let entries = parse_raw(&raw);
    let counts = parse_numstat(&numstat);

    let mut staged = Staged::default();
    let paths = entries
        .iter()
        .map(|e| {
            let (added, deleted, binary) = counts
                .iter()
                .find(|(p, ..)| *p == e.path)
                .map_or((0, 0, false), |(_, a, d, b)| (*a, *d, *b));
            ChangedPath {
                path: e.path.clone(),
                status: e.status,
                added_lines: added,
                deleted_lines: deleted,
                binary,
            }
        })
        .collect::<Vec<_>>();

    let readable: Vec<&RawEntry> = entries
        .iter()
        .filter(|e| {
            let skip = match e.new_mode.as_str() {
                _ if e.status == ChangeStatus::Deleted => return false,
                "120000" => Some("symlink"),
                "160000" => Some("submodule"),
                _ => None,
            };
            if let Some(why) = skip {
                staged.skipped.push(format!("{}: {why}", e.path));
            }
            skip.is_none()
        })
        .collect();
    let ids: Vec<String> = readable.iter().map(|e| e.blob.clone()).collect();
    for (entry, bytes) in readable.iter().zip(read_blobs(git, &ids)?) {
        if bytes.len() > MAX_FILE_BYTES {
            staged
                .skipped
                .push(format!("{}: larger than 8 MiB", entry.path));
            continue;
        }
        if bytes.iter().take(8000).any(|b| *b == 0) {
            staged.skipped.push(format!("{}: binary", entry.path));
            continue;
        }
        let (has_bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
            Some(rest) => (true, rest),
            None => (false, bytes.as_slice()),
        };
        match std::str::from_utf8(body) {
            Ok(text) => {
                staged
                    .documents
                    .push(SourceDocument::file(&entry.path, text, has_bom, true))
            }
            Err(_) => staged.skipped.push(format!("{}: not UTF-8", entry.path)),
        }
    }
    staged.changes = Some(ChangeSet {
        paths,
        staged: true,
    });
    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_and_numstat() {
        let raw = b":000000 100644 0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 A\0a b.txt\0:100644 000000 2222222222222222222222222222222222222222 0000000000000000000000000000000000000000 D\0gone\0";
        let e = parse_raw(raw);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].path, "a b.txt");
        assert_eq!(e[0].status, ChangeStatus::Added);
        assert_eq!(e[1].status, ChangeStatus::Deleted);
        let n = parse_numstat(b"3\t1\ta b.txt\0-\t-\timg.png\0");
        assert_eq!(n[0], ("a b.txt".to_string(), 3, 1, false));
        assert!(n[1].3);
    }
}
