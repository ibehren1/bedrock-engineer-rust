//! A minimal electron-store (`conf`) compatible JSON file, as used for
//! `<userData>/chat-sessions-meta.json`.
//!
//! Like `conf`, every read goes to disk and every write replaces the whole file (tab-indented,
//! no trailing newline, written atomically) after re-reading it. Defaults are merged in on open
//! the way `conf`'s constructor does (`Object.assign({}, defaults, fileStore)`, rewritten only
//! when a default key was missing). Built on [`common::json_file::ConfFile`]:
//!
//! - a missing file is empty;
//! - a file that does not parse is renamed to `<name>.corrupt-<unix-ms>` (never overwritten) and
//!   treated as empty, so the metadata is rebuilt from the session files;
//! - any other read error is returned, and a write that needs the current contents is skipped.

use crate::Result;
use common::json_file::ConfFile;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MetaFile {
    file: ConfFile,
}

impl MetaFile {
    /// Opens the file at `path`, merging `defaults` into it.
    pub fn open(path: impl Into<PathBuf>, defaults: &Map<String, Value>) -> Result<Self> {
        Ok(MetaFile {
            file: ConfFile::open(path, defaults)?,
        })
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// The whole file, or the read error.
    pub fn try_read(&self) -> Result<Map<String, Value>> {
        Ok(self.file.try_read()?)
    }

    /// The whole file; read errors are logged and give `{}`.
    pub fn read(&self) -> Map<String, Value> {
        self.file.read()
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.read().remove(key)
    }

    /// `store.set(key, value)`: read, replace one key (keeping its position), write.
    pub fn set(&self, key: &str, value: Value) -> Result<()> {
        Ok(self.file.set(key, value)?)
    }

    /// Read, change the value under `key` with `f`, write — one read-modify-write, so a value
    /// derived from the file (e.g. the metadata map) is never computed from a failed read.
    pub fn update_key(&self, key: &str, f: impl FnOnce(Option<Value>) -> Value) -> Result<()> {
        Ok(self.file.update(|d| {
            let v = f(d.get(key).cloned());
            d.insert(key.to_string(), v);
        })?)
    }

    /// `store.delete(key)`: read, remove one key, write (conf writes even when it was absent).
    pub fn delete(&self, key: &str) -> Result<()> {
        Ok(self.file.delete(key)?)
    }
}

/// `JSON.stringify(value, undefined, indent)`.
pub(crate) fn to_json<T: serde::Serialize + ?Sized>(value: &T, indent: &[u8]) -> Result<Vec<u8>> {
    Ok(common::json_file::to_json_indented(value, indent)?)
}

/// Write `bytes` to `path` via a temp file in the same directory and a rename.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    Ok(common::json_file::write_atomic(path, bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn defaults() -> Map<String, Value> {
        json!({ "recentSessions": [], "metadata": {} })
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn creates_file_with_defaults_tab_indented() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("chat-sessions-meta.json");
        MetaFile::open(&p, &defaults()).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert_eq!(text, "{\n\t\"recentSessions\": [],\n\t\"metadata\": {}\n}");
    }

    #[test]
    fn existing_complete_file_not_rewritten() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        let original = "{\"metadata\":{},\"recentSessions\":[\"a\"],\"activeSessionId\":\"a\"}";
        fs::write(&p, original).unwrap();
        MetaFile::open(&p, &defaults()).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
    }

    #[test]
    fn missing_default_merged_defaults_first() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        fs::write(&p, r#"{"activeSessionId":"x","metadata":{"a":1}}"#).unwrap();
        let m = MetaFile::open(&p, &defaults()).unwrap();
        let keys: Vec<String> = m.read().keys().cloned().collect();
        assert_eq!(keys, ["recentSessions", "metadata", "activeSessionId"]);
        assert_eq!(m.get("metadata"), Some(json!({"a": 1})));
    }

    #[test]
    fn invalid_json_moved_aside_and_treated_as_empty() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        fs::write(&p, "{not json").unwrap();
        let m = MetaFile::open(&p, &defaults()).unwrap();
        assert_eq!(m.get("recentSessions"), Some(json!([])));
        let kept: Vec<String> = fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("m.json.corrupt-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(
            fs::read_to_string(d.path().join(&kept[0])).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn lone_surrogates_parse() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        let original = r#"{"recentSessions":[],"metadata":{"a":{"title":"x\ud83d"}}}"#;
        fs::write(&p, original).unwrap();
        let m = MetaFile::open(&p, &defaults()).unwrap();
        assert_eq!(
            m.get("metadata"),
            Some(json!({"a": {"title": "x\u{fffd}"}}))
        );
        // Opening a complete file does not rewrite it.
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
    }

    #[test]
    fn read_error_is_not_turned_into_an_empty_write() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        fs::create_dir(&p).unwrap();
        assert!(MetaFile::open(&p, &defaults()).is_err());
        let m = MetaFile {
            file: ConfFile::new(&p),
        };
        assert!(m.set("activeSessionId", json!("s")).is_err());
        assert!(m.update_key("metadata", |_| json!({})).is_err());
        assert!(p.is_dir());
    }

    #[test]
    fn set_keeps_position_delete_removes() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("m.json");
        let m = MetaFile::open(&p, &defaults()).unwrap();
        m.set("activeSessionId", json!("s")).unwrap();
        m.set("recentSessions", json!(["s"])).unwrap();
        let keys: Vec<String> = m.read().keys().cloned().collect();
        assert_eq!(keys, ["recentSessions", "metadata", "activeSessionId"]);
        m.delete("activeSessionId").unwrap();
        assert_eq!(m.get("activeSessionId"), None);
        let names: Vec<_> = fs::read_dir(d.path()).unwrap().collect();
        assert_eq!(names.len(), 1, "no temp files left behind");
    }
}
