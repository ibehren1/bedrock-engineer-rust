//! The Files panel's file access: everything under the project folder (store key
//! `projectPath`), and nothing outside it.
//!
//! Every path the renderer sends goes through [`resolve_in_root`], which accepts a
//! root-relative or absolute path, resolves symlinks and `..`, and rejects anything that ends up
//! outside the root. Paths sent back are root-relative with forward slashes (`""` is the root).
//!
//! - [`list_dir`]: one level of a folder (the panel's tree loads lazily), dirs first, with the
//!   [`SKIP_NAMES`] and the root `.gitignore` left out.
//! - [`read_text`] / [`write_text`]: the text viewer and editor (2 MB cap, binary refused,
//!   atomic writes with an mtime conflict check).
//! - [`exists`]: batch check for file paths mentioned in chat.
//! - [`serve`]: the `workspacefile://` scheme (raw bytes for the PDF / DOCX / image viewers).
//! - [`watch::ProjectWatcher`]: debounced recursive change notifications.

pub mod serve;
pub mod watch;

#[cfg(test)]
mod tests;

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tools::GitignoreLikeMatcher;

/// Folder / file names never listed or watched, at any depth.
pub const SKIP_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".DS_Store",
    "docker-sandboxes",
];

/// [`read_text`]'s default cap.
pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// How much of a file is checked for NUL bytes.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("No project folder is set")]
    NoRoot,
    #[error("Project folder not found: {0}")]
    RootMissing(String),
    #[error("Path is outside the project folder")]
    Outside,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Not a folder: {0}")]
    NotADir(String),
    #[error("Not a file: {0}")]
    NotAFile(String),
    #[error("Not a text file")]
    NotText,
    /// The file's mtime is not the one the editor loaded.
    #[error("CONFLICT: {0} changed on disk since it was opened")]
    Conflict(String),
    #[error("{0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for String {
    fn from(e: Error) -> String {
        e.to_string()
    }
}

fn io_err(what: &Path, e: std::io::Error) -> Error {
    Error::Io(format!("{}: {e}", what.display()))
}

/// `fs::canonicalize` without Windows' `\\?\` prefix on plain drive paths, so results compare,
/// display and hand to the shell like ordinary paths.
fn canonical(p: &Path) -> std::io::Result<PathBuf> {
    fs::canonicalize(p).map(simplify)
}

#[cfg(windows)]
fn simplify(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest)
            if rest.len() >= 2 && rest.as_bytes()[1] == b':' && !rest.starts_with("UNC\\") =>
        {
            PathBuf::from(rest.to_string())
        }
        _ => p,
    }
}

#[cfg(not(windows))]
fn simplify(p: PathBuf) -> PathBuf {
    p
}

/// The canonical project folder.
pub fn canonical_root(root: &Path) -> Result<PathBuf> {
    if root.as_os_str().is_empty() {
        return Err(Error::NoRoot);
    }
    match canonical(root) {
        Ok(p) if p.is_dir() => Ok(p),
        _ => Err(Error::RootMissing(root.display().to_string())),
    }
}

/// `p` relative to the canonical `root`, with forward slashes (`""` for the root itself), or
/// `None` when it isn't under it.
pub fn relative_to(root: &Path, p: &Path) -> Option<String> {
    let rest = p.strip_prefix(root).ok()?;
    let parts: Vec<String> = rest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// Resolve `input` (root-relative or absolute) to a canonical path under `root` (itself
/// canonical, see [`canonical_root`]).
///
/// Existing paths are canonicalized (so symlinks pointing out of the root are rejected). A path
/// that doesn't exist yet (a write target) resolves through its canonical parent, which must
/// exist; its last component must be a plain name.
pub fn resolve_in_root(root: &Path, input: &str) -> Result<PathBuf> {
    let input_path = Path::new(input);
    let candidate = if input.is_empty() {
        root.to_path_buf()
    } else if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        root.join(input_path)
    };
    let inside = |p: PathBuf| {
        if p.starts_with(root) {
            Ok(p)
        } else {
            Err(Error::Outside)
        }
    };
    match canonical(&candidate) {
        Ok(p) => return inside(p),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            // E.g. a component that is a file: treat like a missing path, after the lexical check.
            if lexically_escapes(root, &candidate) {
                return Err(Error::Outside);
            }
            return Err(io_err(&candidate, e));
        }
        Err(_) => {}
    }
    // Not there (yet): resolve the parent; the name must be a plain file name.
    let name = match candidate.components().next_back() {
        Some(Component::Normal(n)) => n.to_os_string(),
        _ => {
            return Err(if lexically_escapes(root, &candidate) {
                Error::Outside
            } else {
                Error::NotFound(input.to_string())
            })
        }
    };
    let parent = candidate.parent().unwrap_or(root);
    match canonical(parent) {
        Ok(p) => inside(p.join(name)),
        Err(_) if lexically_escapes(root, &candidate) => Err(Error::Outside),
        Err(_) => Err(Error::NotFound(input.to_string())),
    }
}

/// Whether `p` (absolute), with `.` / `..` applied lexically, leaves `root`.
fn lexically_escapes(root: &Path, p: &Path) -> bool {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    !out.starts_with(root)
}

/// Milliseconds since the epoch of `meta`'s mtime (0 if unavailable).
fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lowercase extension without the dot (`""` if none).
pub fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: Kind,
    pub size: u64,
    pub modified_ms: u64,
    pub ext: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// The canonical project folder (absolute).
    pub root: String,
    pub entries: Vec<Entry>,
}

/// The root `.gitignore`'s patterns (comments and blank lines dropped).
pub fn gitignore_matcher(root: &Path) -> GitignoreLikeMatcher {
    let text = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    let patterns: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    GitignoreLikeMatcher::new(&patterns)
}

/// Whether the root-relative `rel` is hidden from the panel: a [`SKIP_NAMES`] component, or
/// matched by the root `.gitignore`.
pub fn is_skipped(matcher: &GitignoreLikeMatcher, rel: &str) -> bool {
    rel.split('/').any(|part| SKIP_NAMES.contains(&part))
        || (!rel.is_empty() && matcher.is_ignored(rel))
}

/// One level of the folder `dir` (root-relative): dirs first, then by name, case-insensitively.
pub fn list_dir(root: &Path, dir: &str) -> Result<Listing> {
    let root = canonical_root(root)?;
    let abs = resolve_in_root(&root, dir)?;
    if !abs.is_dir() {
        return Err(if abs.exists() {
            Error::NotADir(dir.to_string())
        } else {
            Error::NotFound(dir.to_string())
        });
    }
    let base = relative_to(&root, &abs).ok_or(Error::Outside)?;
    let matcher = gitignore_matcher(&root);
    let mut entries = Vec::new();
    for item in fs::read_dir(&abs).map_err(|e| io_err(&abs, e))? {
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        let path = if base.is_empty() {
            name.clone()
        } else {
            format!("{base}/{name}")
        };
        if is_skipped(&matcher, &path) {
            continue;
        }
        // Follow symlinks for the kind; a broken link is listed as a file.
        let meta = match fs::metadata(item.path()).or_else(|_| item.metadata()) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let kind = if meta.is_dir() { Kind::Dir } else { Kind::File };
        entries.push(Entry {
            ext: if kind == Kind::File {
                extension_of(&name)
            } else {
                String::new()
            },
            size: if kind == Kind::File { meta.len() } else { 0 },
            modified_ms: modified_ms(&meta),
            name,
            path,
            kind,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != Kind::Dir)
            .cmp(&(b.kind != Kind::Dir))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(Listing {
        root: root.display().to_string(),
        entries,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    pub content: String,
    /// The file is larger than the cap; `content` is its first part.
    pub truncated: bool,
    pub size: u64,
    pub modified_ms: u64,
}

/// Whether `bytes` (a file's start) looks binary: a NUL byte in the first 8 KB.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SNIFF_BYTES)].contains(&0)
}

/// Decode `bytes` as UTF-8. When `truncated`, an incomplete character at the end is dropped;
/// anything else that isn't UTF-8 (or a NUL in the first 8 KB) is [`Error::NotText`].
pub fn decode_text(mut bytes: Vec<u8>, truncated: bool) -> Result<String> {
    if looks_binary(&bytes) {
        return Err(Error::NotText);
    }
    match std::str::from_utf8(&bytes) {
        Ok(_) => {}
        Err(e) if truncated && e.error_len().is_none() => bytes.truncate(e.valid_up_to()),
        Err(_) => return Err(Error::NotText),
    }
    String::from_utf8(bytes).map_err(|_| Error::NotText)
}

/// The text of the file `path` (root-relative), at most `max_bytes` of it.
pub fn read_text(root: &Path, path: &str, max_bytes: usize) -> Result<TextFile> {
    let root = canonical_root(root)?;
    let abs = resolve_in_root(&root, path)?;
    let meta = fs::metadata(&abs).map_err(|_| Error::NotFound(path.to_string()))?;
    if !meta.is_file() {
        return Err(Error::NotAFile(path.to_string()));
    }
    let mut bytes = Vec::new();
    fs::File::open(&abs)
        .map_err(|e| io_err(&abs, e))?
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_err(&abs, e))?;
    let truncated = bytes.len() > max_bytes;
    bytes.truncate(max_bytes);
    Ok(TextFile {
        content: decode_text(bytes, truncated)?,
        truncated,
        size: meta.len(),
        modified_ms: modified_ms(&meta),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    pub modified_ms: u64,
}

/// Write `content` to `path` (root-relative; created if missing, its folder must exist).
///
/// With `expected_modified_ms`, the file's current mtime must match (the editor loaded that
/// version), else [`Error::Conflict`]; a file that was deleted meanwhile is also a conflict. The
/// write goes to a temp file in the same folder that is renamed over the target, so readers never
/// see a half-written file; an existing file's permissions are kept.
pub fn write_text(
    root: &Path,
    path: &str,
    content: &str,
    expected_modified_ms: Option<f64>,
) -> Result<Written> {
    let root = canonical_root(root)?;
    let abs = resolve_in_root(&root, path)?;
    if abs == root {
        return Err(Error::NotAFile(path.to_string()));
    }
    let existing = fs::metadata(&abs).ok();
    if existing.as_ref().is_some_and(|m| !m.is_file()) {
        return Err(Error::NotAFile(path.to_string()));
    }
    if let Some(expected) = expected_modified_ms {
        let current = existing.as_ref().map(modified_ms);
        if current != Some(expected.round() as u64) {
            return Err(Error::Conflict(path.to_string()));
        }
    }
    let dir = abs
        .parent()
        .ok_or_else(|| Error::NotFound(path.to_string()))?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".bedrock-engineer-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| io_err(dir, e))?;
    tmp.write_all(content.as_bytes())
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|e| io_err(tmp.path(), e))?;
    if let Some(meta) = &existing {
        // Best effort: a failure here shouldn't lose the user's edit.
        let _ = fs::set_permissions(tmp.path(), meta.permissions());
    }
    tmp.persist(&abs).map_err(|e| io_err(&abs, e.error))?;
    let meta = fs::metadata(&abs).map_err(|e| io_err(&abs, e))?;
    Ok(Written {
        modified_ms: modified_ms(&meta),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// Root-relative.
    pub path: String,
    pub kind: Kind,
}

/// For each input (absolute or root-relative): where it is under `root`, or `None` when it is
/// missing or outside. Keyed by the input string.
pub fn exists(root: &Path, inputs: &[String]) -> Result<BTreeMap<String, Option<Found>>> {
    let root = canonical_root(root)?;
    Ok(inputs
        .iter()
        .map(|input| {
            let found = resolve_in_root(&root, input).ok().and_then(|abs| {
                let meta = fs::metadata(&abs).ok()?;
                Some(Found {
                    path: relative_to(&root, &abs)?,
                    kind: if meta.is_dir() { Kind::Dir } else { Kind::File },
                })
            });
            (input.clone(), found)
        })
        .collect())
}

/// The `Content-Type` [`serve`] sends for a file name. HTML (and anything unknown) is never
/// served as a document type, so the scheme can't run script.
pub fn mime_for(name: &str) -> &'static str {
    match extension_of(name).as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "html" | "htm" | "xhtml" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
