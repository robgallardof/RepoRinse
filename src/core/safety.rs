//! Bounded UTF-8 input and guarded file replacement.
use super::model::SourceDocument;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Reject links in every component, paths escaping the repository and Git internals.
pub fn resolve(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let supplied_root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(root)
    };
    let root = supplied_root.canonicalize().map_err(|e| e.to_string())?;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let relative = absolute
        .strip_prefix(&root)
        .or_else(|_| absolute.strip_prefix(&supplied_root))
        .map_err(|_| "path is outside repository")?;
    let mut current = root.clone();
    for component in relative.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(name) if !name.to_string_lossy().eq_ignore_ascii_case(".git") => {
                current.push(name)
            }
            _ => return Err("unsafe path component".into()),
        }
        let meta = fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
        if is_link(&meta) {
            return Err("symlink paths are not supported".into());
        }
    }
    if !current.is_file() {
        return Err("target is not a regular file".into());
    }
    let canonical = current.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err("path is outside repository".into());
    }
    Ok(current)
}
pub fn read(path: &Path, label: &str) -> Result<(SourceDocument, Vec<u8>), String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("file exceeds 8 MiB".into());
    }
    if bytes.contains(&0) {
        return Err("binary or unsupported encoding".into());
    }
    let (bom, body) = bytes
        .strip_prefix(b"\xEF\xBB\xBF")
        .map_or((false, bytes.as_slice()), |b| (true, b));
    let text = std::str::from_utf8(body).map_err(|_| "unsupported encoding (expected UTF-8)")?;
    Ok((SourceDocument::file(label, text, bom, false), bytes))
}
/// Temp file lives beside the target; persist replaces it atomically. Recheck original bytes before writing.
pub fn replace(
    root: &Path,
    path: &Path,
    original: &[u8],
    text: &str,
    bom: bool,
) -> Result<(), String> {
    let path = resolve(root, path)?;
    let permissions = fs::metadata(&path)
        .map_err(|e| e.to_string())?
        .permissions();
    if permissions.readonly() {
        return Err("target is read-only".into());
    }
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("missing parent")?)
        .map_err(|e| e.to_string())?;
    if bom {
        temp.write_all(b"\xEF\xBB\xBF").map_err(|e| e.to_string())?;
    }
    temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    temp.as_file()
        .set_permissions(permissions)
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    resolve(root, &path)?;
    if fs::read(&path).map_err(|e| e.to_string())? != original {
        return Err("file changed since analysis; fix refused".into());
    }
    temp.persist(&path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Windows junctions and other reparse points are links too.
pub fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
