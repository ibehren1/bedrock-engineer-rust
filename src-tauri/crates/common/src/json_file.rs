//! Data-safe JSON file helpers shared by every store that keeps user data in JSON files
//! (`config.json`, the electron-store side files, chat / background / todo session files).
//!
//! The rules these helpers enforce:
//!
//! - **Parse like `JSON.parse`.** JavaScript strings are UTF-16, so the Electron app can write
//!   lone surrogate escapes (`"\ud83d"`, e.g. from an emoji cut in half). `JSON.parse` accepts
//!   them; `serde_json` rejects the whole file. [`parse_lenient`] first replaces every unpaired
//!   `\uD800`–`\uDFFF` escape with `�` (what `TextDecoder` would produce), so such a file
//!   loads instead of being treated as corrupt.
//! - **Never delete or overwrite a file that fails to parse.** [`quarantine`] renames it to
//!   `<name>.corrupt-<unix-ms>` next to the original, so a later write starts a fresh file while
//!   the old bytes are kept for recovery.
//! - **Write atomically.** [`write_atomic`] writes a unique temp file in the same directory,
//!   fsyncs it and renames it over the target.
//!
//! [`ConfFile`] builds an electron-store (`conf`) compatible file out of these: every read goes
//! to disk and every write is a read-modify-write of the current file contents.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Replace unpaired UTF-16 surrogate escapes (`\uD800`–`\uDFFF` not forming a high+low pair) in
/// JSON text with the escape `\ufffd` (`�`). Everything else, including escaped backslashes (`\\uD800` is the
/// literal text `\uD800`, not an escape), is left byte-for-byte unchanged. Borrows when there is
/// nothing to replace.
pub fn sanitize_lone_surrogates(text: &str) -> Cow<'_, str> {
    let b = text.as_bytes();
    let hex4 = |i: usize| -> Option<u16> {
        let s = b.get(i..i + 4)?;
        if !s.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        u16::from_str_radix(std::str::from_utf8(s).ok()?, 16).ok()
    };
    // `\uXXXX` at `i` (pointing at the backslash).
    let escape_at = |i: usize| -> Option<u16> {
        if b.get(i) == Some(&b'\\') && b.get(i + 1) == Some(&b'u') {
            hex4(i + 2)
        } else {
            None
        }
    };
    let mut out: Option<String> = None;
    let mut copied = 0; // bytes of `text` already copied into `out`
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        let Some(unit) = escape_at(i) else {
            // Some other escape (`\"`, `\\`, `\n`, ...): skip both characters.
            i += 2;
            continue;
        };
        match unit {
            0xD800..=0xDBFF => {
                if matches!(escape_at(i + 6), Some(0xDC00..=0xDFFF)) {
                    i += 12;
                    continue;
                }
            }
            0xDC00..=0xDFFF => {}
            _ => {
                i += 6;
                continue;
            }
        }
        // Lone surrogate: all split points are ASCII, so slicing keeps UTF-8 intact.
        let o = out.get_or_insert_with(|| String::with_capacity(text.len()));
        o.push_str(&text[copied..i]);
        o.push_str("\\ufffd");
        i += 6;
        copied = i;
    }
    match out {
        None => Cow::Borrowed(text),
        Some(mut o) => {
            o.push_str(&text[copied..]);
            Cow::Owned(o)
        }
    }
}

/// `JSON.parse` semantics for surrogates: [`sanitize_lone_surrogates`], then `serde_json`.
pub fn parse_lenient<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    match serde_json::from_str(text) {
        Ok(v) => Ok(v),
        Err(e) => match sanitize_lone_surrogates(text) {
            Cow::Owned(fixed) => serde_json::from_str(&fixed),
            Cow::Borrowed(_) => Err(e),
        },
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

/// Move an unparseable file out of the way: `<path>.corrupt-<unix-ms>` (with a counter suffix
/// if that name is taken). Returns the new path. The file is renamed, never deleted.
pub fn quarantine(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let base = format!("{name}.corrupt-{}", now_ms());
    let mut target = path.with_file_name(&base);
    let mut n = 1;
    while target.exists() {
        target = path.with_file_name(format!("{base}-{n}"));
        n += 1;
    }
    fs::rename(path, &target)?;
    tracing::error!(
        path = %path.display(),
        moved_to = %target.display(),
        "File could not be parsed; moved it aside and continuing with defaults"
    );
    Ok(target)
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` through a unique temp file in the same directory (fsynced), then
/// rename it over `path`. Creates the parent directory. The temp file is removed on failure.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(
        ".tmp-{}-{}-{}",
        std::process::id(),
        now_ms(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// `JSON.stringify(value, null, indent)` bytes (no trailing newline).
pub fn to_json_indented<T: serde::Serialize + ?Sized>(
    value: &T,
    indent: &[u8],
) -> serde_json::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(indent);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser)?;
    Ok(buf)
}

/// Result of reading a JSON-object file with [`read_object`].
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectFile {
    /// The file does not exist.
    Missing,
    /// The file exists and holds this object (an empty/whitespace-only file is an empty object).
    Object(Map<String, Value>),
    /// The file did not parse as a JSON object; it was renamed to this path.
    Quarantined(PathBuf),
}

impl ObjectFile {
    /// The object, or an empty one for a missing / quarantined file.
    pub fn into_map(self) -> Map<String, Value> {
        match self {
            ObjectFile::Object(m) => m,
            _ => Map::new(),
        }
    }
}

/// Read a file that must hold a JSON object.
///
/// - missing → [`ObjectFile::Missing`]
/// - other I/O errors (permissions, a directory in the way, ...) → `Err`; callers must not write
///   over a file they could not read
/// - invalid JSON or a non-object root → the file is [`quarantine`]d → [`ObjectFile::Quarantined`]
///   (`Err` if even the rename fails, so nothing overwrites it)
pub fn read_object(path: &Path) -> io::Result<ObjectFile> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ObjectFile::Missing),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            // Not UTF-8. `JSON.parse` of a lossily decoded buffer would still fail; keep it.
            tracing::error!(path = %path.display(), error = %e, "File is not valid UTF-8");
            return quarantine(path).map(ObjectFile::Quarantined);
        }
        Err(e) => return Err(e),
    };
    if text.trim().is_empty() {
        return Ok(ObjectFile::Object(Map::new()));
    }
    match parse_lenient::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(ObjectFile::Object(m)),
        Ok(_) => {
            tracing::error!(path = %path.display(), "File does not hold a JSON object");
            quarantine(path).map(ObjectFile::Quarantined)
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "File is not valid JSON");
            quarantine(path).map(ObjectFile::Quarantined)
        }
    }
}

/// Read a JSON file of any shape (`Ok(None)` when missing). Unparseable files are quarantined
/// (`Ok(None)` too); other I/O errors are returned.
pub fn read_value(path: &Path) -> io::Result<Option<Value>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            tracing::error!(path = %path.display(), error = %e, "File is not valid UTF-8");
            quarantine(path)?;
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    match parse_lenient::<Value>(&text) {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "File is not valid JSON");
            quarantine(path)?;
            Ok(None)
        }
    }
}

/// An electron-store (`conf`) compatible JSON object file: tab-indented, written atomically, no
/// in-memory cache. Every [`get`](Self::get) reads the file and every [`set`](Self::set) /
/// [`delete`](Self::delete) re-reads it right before writing, so writes made by another process
/// (the Electron build sharing the same user data) since the last access are kept. Callers
/// serialize access within the process (e.g. behind a `Mutex`).
///
/// Remaining window: a write by another process between our re-read and our rename is lost
/// (last writer wins), exactly as between two electron-store instances.
#[derive(Debug, Clone)]
pub struct ConfFile {
    path: PathBuf,
}

impl ConfFile {
    /// A handle for `path`, without touching the disk.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        ConfFile { path: path.into() }
    }

    /// `new Store({ defaults })`: open `path`, merging in `defaults` for missing keys (defaults
    /// first, then the file's own keys, like `Object.assign({}, defaults, fileStore)`), and write
    /// the file only when a default key was missing.
    pub fn open(path: impl Into<PathBuf>, defaults: &Map<String, Value>) -> io::Result<Self> {
        let file = ConfFile::new(path);
        let current = file.try_read()?;
        if defaults.keys().any(|k| !current.contains_key(k)) {
            let mut merged = defaults.clone();
            for (k, v) in current {
                merged.insert(k, v);
            }
            file.write(&merged)?;
        }
        Ok(file)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The whole file, or `Err` when it exists but cannot be read. Missing or quarantined → `{}`.
    pub fn try_read(&self) -> io::Result<Map<String, Value>> {
        read_object(&self.path).map(ObjectFile::into_map)
    }

    /// The whole file; read errors are logged and give `{}` (reads never write).
    pub fn read(&self) -> Map<String, Value> {
        self.try_read().unwrap_or_else(|e| {
            tracing::error!(path = %self.path.display(), error = %e, "Failed to read store file");
            Map::new()
        })
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.read().remove(key)
    }

    /// Read, apply `f`, write. Nothing is written when the read fails.
    pub fn update<R>(&self, f: impl FnOnce(&mut Map<String, Value>) -> R) -> io::Result<R> {
        let mut data = self.try_read()?;
        let r = f(&mut data);
        self.write(&data)?;
        Ok(r)
    }

    /// `store.set(key, value)`: replace one key (keeping its position).
    pub fn set(&self, key: &str, value: Value) -> io::Result<()> {
        self.update(|d| {
            d.insert(key.to_string(), value);
        })
    }

    /// `store.delete(key)` (conf writes even when the key was absent).
    pub fn delete(&self, key: &str) -> io::Result<()> {
        self.update(|d| {
            d.shift_remove(key);
        })
    }

    fn write(&self, data: &Map<String, Value>) -> io::Result<()> {
        let bytes = to_json_indented(data, b"\t").map_err(io::Error::other)?;
        write_atomic(&self.path, &bytes)
    }
}

/// Validate an id that becomes part of a file name (`<dir>/<id>.json`): only
/// `[A-Za-z0-9_.-]`, non-empty, no leading dot and no `..`, so it cannot name a hidden file or
/// reach outside the directory.
pub fn is_safe_file_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn sanitizes_only_unpaired_surrogates() {
        assert!(matches!(
            sanitize_lone_surrogates(r#"{"a":"😀 ok"}"#),
            Cow::Borrowed(_)
        ));
        assert_eq!(
            sanitize_lone_surrogates(r#"{"a":"x\ud83dy"}"#),
            r#"{"a":"x\ufffdy"}"#
        );
        assert_eq!(
            sanitize_lone_surrogates(r#"["\uDE00", "\uD83DA"]"#),
            r#"["\ufffd", "\ufffdA"]"#
        );
        // An escaped backslash followed by text is not an escape.
        assert!(matches!(
            sanitize_lone_surrogates(r#"["\\ud83d"]"#),
            Cow::Borrowed(_)
        ));
        // Multibyte UTF-8 around a replacement survives.
        assert_eq!(
            sanitize_lone_surrogates("[\"é\\ud800日\"]"),
            "[\"é\\ufffd日\"]"
        );
        // Truncated escape at the end is left for serde to reject.
        assert_eq!(sanitize_lone_surrogates(r#""\ud8"#), r#""\ud8"#);
    }

    #[test]
    fn parse_lenient_accepts_what_json_parse_accepts() {
        let text = r#"{"title":"cut \ud83d","ok":"😀"}"#;
        assert!(serde_json::from_str::<Value>(text).is_err());
        let v: Value = parse_lenient(text).unwrap();
        assert_eq!(v["title"], json!("cut \u{fffd}"));
        assert_eq!(v["ok"], json!("😀"));
        assert!(parse_lenient::<Value>("{bad").is_err());
    }

    #[test]
    fn quarantine_renames_and_keeps_bytes() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("x.json");
        fs::write(&p, "{bad").unwrap();
        let q = quarantine(&p).unwrap();
        assert!(!p.exists());
        assert_eq!(fs::read_to_string(&q).unwrap(), "{bad");
        assert!(q
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("x.json.corrupt-"));
        // A second quarantine in the same millisecond gets a distinct name.
        fs::write(&p, "{bad2").unwrap();
        let q2 = quarantine(&p).unwrap();
        assert_ne!(q, q2);
    }

    #[test]
    fn read_object_outcomes() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("o.json");
        assert_eq!(read_object(&p).unwrap(), ObjectFile::Missing);
        fs::write(&p, "  ").unwrap();
        assert_eq!(read_object(&p).unwrap(), ObjectFile::Object(Map::new()));
        fs::write(&p, r#"{"a":"\udc00"}"#).unwrap();
        assert_eq!(read_object(&p).unwrap().into_map()["a"], json!("\u{fffd}"));
        fs::write(&p, "[1]").unwrap();
        assert!(matches!(
            read_object(&p).unwrap(),
            ObjectFile::Quarantined(_)
        ));
        assert!(!p.exists());
        // A directory in the way is an I/O error, not corruption.
        fs::create_dir(&p).unwrap();
        assert!(read_object(&p).is_err());
        assert!(p.is_dir());
    }

    #[test]
    fn write_atomic_leaves_no_temp_files() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("sub").join("w.json");
        write_atomic(&p, b"{}").unwrap();
        write_atomic(&p, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "{\"a\":1}");
        let names: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn conf_file_rereads_before_every_write() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("c.json");
        let defaults = json!({"metadata": {}}).as_object().unwrap().clone();
        let f = ConfFile::open(&p, &defaults).unwrap();
        // Another process writes a key.
        fs::write(&p, r#"{"metadata":{},"other":"kept"}"#).unwrap();
        f.set("mine", json!(1)).unwrap();
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(on_disk, json!({"metadata": {}, "other": "kept", "mine": 1}));
        assert_eq!(f.get("other"), Some(json!("kept")));
    }

    #[test]
    fn conf_file_quarantines_corrupt_file_instead_of_overwriting() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("c.json");
        fs::write(&p, "{not json").unwrap();
        let f = ConfFile::open(&p, &json!({"k": []}).as_object().unwrap().clone()).unwrap();
        assert_eq!(f.get("k"), Some(json!([])));
        let corrupt: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("c.json.corrupt-"))
            .collect();
        assert_eq!(corrupt.len(), 1);
        assert_eq!(
            fs::read_to_string(d.path().join(&corrupt[0])).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn conf_file_does_not_write_when_read_fails() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("c.json");
        fs::create_dir(&p).unwrap();
        let f = ConfFile::new(&p);
        assert!(f.set("a", json!(1)).is_err());
        assert!(p.is_dir());
    }

    #[test]
    fn safe_file_ids() {
        for ok in ["session_123", "scheduled-abc-1f2e", "a.b", "A-Z_0.9"] {
            assert!(is_safe_file_id(ok), "{ok}");
        }
        for bad in [
            "", ".hidden", "..", "a/../b", "a/b", "a\\b", "x..y", "a b", "é",
        ] {
            assert!(!is_safe_file_id(bad), "{bad}");
        }
    }
}
