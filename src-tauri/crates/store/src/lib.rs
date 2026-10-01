//! Persistent, electron-store–compatible configuration store.
//!
//! Port of `src/preload/store.ts`. The config file lives at
//! `<config dir>/<electron app name>/config.json` (on macOS:
//! `~/Library/Application Support/<name>/config.json`) so an existing Electron
//! install's settings are picked up unchanged.
//!
//! - `get` / `set` / `delete` accept dotted keys (`aws.region`).
//! - Like electron-store, the file is the source of truth: every `get` / `all`
//!   sees the file's current contents (re-parsed whenever its size or mtime
//!   changed), and every `set` / `delete` re-reads the file right before
//!   writing, so a concurrent write by the Electron build (or anything else) to
//!   another key is kept instead of being clobbered by a stale in-memory copy.
//! - Writes are atomic (unique tmp file + fsync + rename) and use tab-indented
//!   JSON, matching electron-store's on-disk format.
//! - A file that does not parse (after the `JSON.parse`-compatible lone
//!   surrogate fix-up) is never overwritten: it is renamed to
//!   `config.json.corrupt-<unix-ms>` and the store continues with its last
//!   known contents (or the defaults, on open).
//! - Defaults and migrations from the TS `init()` are applied on open; see
//!   [`init`] for the per-key rules.
//!
//! Remaining race: a write by another process that lands between our re-read
//! and our rename is lost (last writer wins), the same window two electron-store
//! instances have with each other. electron-store takes no file lock, so an
//! advisory lock here would not protect against the Electron build and is not
//! used; two Tauri instances are prevented by the single-instance plugin.

use common::json_file::{read_object, to_json_indented, write_atomic, ObjectFile};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub mod init;
pub use init::Env;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("key must not be empty")]
    EmptyKey,
    #[error("could not determine the user config directory")]
    NoConfigDir,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Returns `<config dir>/<app_name>/config.json`.
pub fn default_path(app_name: &str) -> Result<PathBuf> {
    let dir = dirs::config_dir().ok_or(Error::NoConfigDir)?;
    Ok(dir.join(app_name).join("config.json"))
}

/// Moves `<config_root>/<legacy>` to `<config_root>/<current>` when the app was renamed, so the
/// settings file, chat history and logs follow the new name. Does nothing when there is no legacy
/// dir or when `current` already holds data; an empty `current` dir is replaced. Returns whether
/// the dir was moved.
pub fn adopt_legacy_dir(config_root: &Path, legacy: &str, current: &str) -> std::io::Result<bool> {
    let from = config_root.join(legacy);
    let to = config_root.join(current);
    if !from.is_dir() {
        return Ok(false);
    }
    if to.exists() {
        if !to.is_dir() || fs::read_dir(&to)?.next().is_some() {
            return Ok(false);
        }
        fs::remove_dir(&to)?;
    }
    fs::rename(&from, &to)?;
    Ok(true)
}

/// [`adopt_legacy_dir`] under the OS config dir.
pub fn adopt_legacy_app_dir(legacy: &str, current: &str) -> Result<bool> {
    let dir = dirs::config_dir().ok_or(Error::NoConfigDir)?;
    Ok(adopt_legacy_dir(&dir, legacy, current)?)
}

/// Called after every successful `set` / `delete` with the top-level key that
/// changed and its new value (`None` when the key is gone).
pub type ChangeListener = Box<dyn Fn(&str, Option<&Value>) + Send>;

/// What the cached copy was read from, to skip re-parsing an unchanged file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: SystemTime,
    len: u64,
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let m = fs::metadata(path).ok()?;
    Some(Stamp {
        modified: m.modified().ok()?,
        len: m.len(),
    })
}

#[derive(Debug)]
struct Cache {
    data: Map<String, Value>,
    /// `None` forces the next read to parse the file.
    stamp: Option<Stamp>,
}

pub struct Store {
    path: PathBuf,
    cache: RefCell<Cache>,
    listener: Option<ChangeListener>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Opens (or creates) the store at `path`, applying defaults and migrations
    /// for the current process environment.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::open_with_env(path, &Env::current())
    }

    /// Like [`Store::open`], with an explicit environment.
    ///
    /// An unparseable file is moved aside (see the module docs) and the store
    /// starts from the defaults. Other read errors are returned.
    pub fn open_with_env(path: impl Into<PathBuf>, env: &Env) -> Result<Self> {
        let path = path.into();
        let before_stamp = stamp_of(&path);
        let mut data = match read_object(&path)? {
            ObjectFile::Object(m) => m,
            ObjectFile::Missing | ObjectFile::Quarantined(_) => Map::new(),
        };
        let before = data.clone();
        init::init(&mut data, env);
        let store = Store {
            cache: RefCell::new(Cache {
                data,
                stamp: before_stamp.filter(|s| Some(*s) == stamp_of(&path)),
            }),
            path,
            listener: None,
        };
        if store.cache.borrow().data != before || !store.path.exists() {
            store.persist()?;
        }
        Ok(store)
    }

    /// A store for `path` holding only the defaults, without reading or writing
    /// the file. For when [`Store::open`] failed: the app can still start, and
    /// because every write re-reads the file first, a later successful write
    /// merges into whatever the file holds rather than replacing it.
    pub fn detached(path: impl Into<PathBuf>, env: &Env) -> Self {
        let mut data = Map::new();
        init::init(&mut data, env);
        Store {
            path: path.into(),
            cache: RefCell::new(Cache { data, stamp: None }),
            listener: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Registers the [`ChangeListener`] (replacing any previous one).
    pub fn set_listener(&mut self, listener: impl Fn(&str, Option<&Value>) + Send + 'static) {
        self.listener = Some(Box::new(listener));
    }

    /// Brings the cached copy up to date with the file. With `force`, the file
    /// is always parsed; otherwise only when its size or mtime changed.
    fn refresh(&self, force: bool) -> Result<()> {
        let stamp = stamp_of(&self.path);
        if !force && stamp.is_some() && stamp == self.cache.borrow().stamp {
            return Ok(());
        }
        match read_object(&self.path)? {
            ObjectFile::Object(m) => {
                let after = stamp_of(&self.path);
                let mut cache = self.cache.borrow_mut();
                cache.data = m;
                // Changed while being read: parse again next time.
                cache.stamp = if after == stamp { after } else { None };
            }
            // Deleted behind our back: electron-store would now see `{}`.
            ObjectFile::Missing => {
                let mut cache = self.cache.borrow_mut();
                cache.data = Map::new();
                cache.stamp = None;
            }
            // Moved aside: recreate the file from the last known good contents.
            ObjectFile::Quarantined(_) => self.persist()?,
        }
        Ok(())
    }

    fn refresh_for_read(&self) {
        if let Err(e) = self.refresh(false) {
            tracing::error!(path = %self.path.display(), error = %e, "Failed to re-read config store; using the last known values");
        }
    }

    /// Gets a value by dotted key.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.refresh_for_read();
        let cache = self.cache.borrow();
        let mut parts = key.split('.');
        let mut cur = cache.data.get(parts.next()?)?;
        for p in parts {
            cur = cur.as_object()?.get(p)?;
        }
        Some(cur.clone())
    }

    /// Sets a value by dotted key, creating intermediate objects, then persists.
    /// The file is re-read first; nothing is written if that fails.
    pub fn set(&mut self, key: &str, value: Value) -> Result<()> {
        let parts = split_key(key)?;
        self.refresh(true)?;
        let (last, init) = parts.split_last().expect("non-empty");
        {
            let mut cache = self.cache.borrow_mut();
            let mut cur = &mut cache.data;
            for p in init {
                let entry = cur
                    .entry((*p).to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if !entry.is_object() {
                    *entry = Value::Object(Map::new());
                }
                cur = entry.as_object_mut().expect("object");
            }
            cur.insert((*last).to_string(), value);
        }
        self.persist()?;
        self.notify(parts[0]);
        Ok(())
    }

    /// Deletes a value by dotted key (no-op if absent), then persists.
    /// The file is re-read first; nothing is written if that fails.
    pub fn delete(&mut self, key: &str) -> Result<()> {
        let parts = split_key(key)?;
        self.refresh(true)?;
        let (last, init) = parts.split_last().expect("non-empty");
        let removed = {
            let mut cache = self.cache.borrow_mut();
            let mut cur = Some(&mut cache.data);
            for p in init {
                cur = cur
                    .and_then(|m| m.get_mut(*p))
                    .and_then(Value::as_object_mut);
            }
            cur.is_some_and(|m| m.shift_remove(*last).is_some())
        };
        if removed {
            self.persist()?;
            self.notify(parts[0]);
        }
        Ok(())
    }

    /// Returns the whole store as a JSON object.
    pub fn all(&self) -> Value {
        self.refresh_for_read();
        Value::Object(self.cache.borrow().data.clone())
    }

    fn notify(&self, top_key: &str) {
        if let Some(listener) = &self.listener {
            let cache = self.cache.borrow();
            listener(top_key, cache.data.get(top_key));
        }
    }

    fn persist(&self) -> Result<()> {
        let buf = to_json_indented(&self.cache.borrow().data, b"\t")?;
        write_atomic(&self.path, &buf)?;
        self.cache.borrow_mut().stamp = stamp_of(&self.path);
        Ok(())
    }
}

fn split_key(key: &str) -> Result<Vec<&str>> {
    if key.is_empty() || key.split('.').any(str::is_empty) {
        return Err(Error::EmptyKey);
    }
    Ok(key.split('.').collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn adopt_legacy_dir_moves_old_dir() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("Old")).unwrap();
        fs::write(root.path().join("Old/config.json"), "{}").unwrap();
        assert!(adopt_legacy_dir(root.path(), "Old", "New").unwrap());
        assert!(!root.path().join("Old").exists());
        assert!(root.path().join("New/config.json").is_file());
    }

    #[test]
    fn adopt_legacy_dir_replaces_empty_current_dir() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("Old")).unwrap();
        fs::write(root.path().join("Old/config.json"), "{}").unwrap();
        fs::create_dir(root.path().join("New")).unwrap();
        assert!(adopt_legacy_dir(root.path(), "Old", "New").unwrap());
        assert!(root.path().join("New/config.json").is_file());
    }

    #[test]
    fn adopt_legacy_dir_keeps_existing_current_data() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("Old")).unwrap();
        fs::write(root.path().join("Old/config.json"), "old").unwrap();
        fs::create_dir(root.path().join("New")).unwrap();
        fs::write(root.path().join("New/config.json"), "new").unwrap();
        assert!(!adopt_legacy_dir(root.path(), "Old", "New").unwrap());
        assert_eq!(
            fs::read_to_string(root.path().join("New/config.json")).unwrap(),
            "new"
        );
        assert!(root.path().join("Old/config.json").is_file());
    }

    #[test]
    fn adopt_legacy_dir_without_legacy_is_noop() {
        let root = TempDir::new().unwrap();
        assert!(!adopt_legacy_dir(root.path(), "Old", "New").unwrap());
        assert!(!root.path().join("New").exists());
    }

    fn tmp() -> (TempDir, PathBuf) {
        let d = TempDir::new().unwrap();
        let p = d.path().join("sub").join("config.json");
        (d, p)
    }

    #[test]
    fn creates_file_with_defaults() {
        let (_d, p) = tmp();
        let s = Store::open(&p).unwrap();
        assert!(p.exists());
        let mut expected = Map::new();
        init::init(&mut expected, &Env::current());
        assert_eq!(s.all(), Value::Object(expected));
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(on_disk, s.all());
    }

    #[test]
    fn set_get_dotted_and_persist() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("aws.region", json!("us-west-2")).unwrap();
        assert_eq!(s.get("aws.region"), Some(json!("us-west-2")));
        let s2 = Store::open(&p).unwrap();
        assert_eq!(s2.get("aws.region"), Some(json!("us-west-2")));
    }

    #[test]
    fn set_overwrites_non_object_parent() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("x", json!(1)).unwrap();
        s.set("x.y", json!(2)).unwrap();
        assert_eq!(s.get("x"), Some(json!({"y": 2})));
    }

    #[test]
    fn delete_dotted_and_missing() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("a.b", json!(1)).unwrap();
        s.set("a.c", json!(2)).unwrap();
        s.delete("a.b").unwrap();
        s.delete("nope.nothing").unwrap();
        assert_eq!(s.get("a"), Some(json!({"c": 2})));
        assert_eq!(Store::open(&p).unwrap().get("a.b"), None);
    }

    #[test]
    fn empty_key_rejected() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        assert!(matches!(s.set("", json!(1)), Err(Error::EmptyKey)));
        assert!(matches!(s.set("a..b", json!(1)), Err(Error::EmptyKey)));
        assert_eq!(s.get(""), None);
    }

    #[test]
    fn all_returns_object() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("k", json!("v")).unwrap();
        assert_eq!(s.all()["k"], json!("v"));
    }

    #[test]
    fn tab_indented_and_no_tmp_left() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("nested.key", json!(true)).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("\n\t\""), "expected tab indentation: {text}");
        assert!(!p.with_extension("json.tmp").exists());
    }

    #[test]
    fn existing_values_not_overwritten() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(
            &p,
            r#"{"aws":{"region":"eu-west-1"},"selectedVoiceId":"tiffany"}"#,
        )
        .unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get("aws"), Some(json!({"region": "eu-west-1"})));
        assert_eq!(s.get("selectedVoiceId"), Some(json!("tiffany")));
    }

    #[test]
    fn migrations_persisted_on_open() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"appTheme":"midnight"}"#).unwrap();
        Store::open(&p).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("charcoal"));
    }

    #[test]
    fn unchanged_file_not_rewritten() {
        let (_d, p) = tmp();
        Store::open(&p).unwrap();
        let before = fs::metadata(&p).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        Store::open(&p).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn empty_file_treated_as_empty() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "").unwrap();
        assert!(Store::open(&p).is_ok());
    }

    fn corrupt_files(p: &Path) -> Vec<PathBuf> {
        fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|f| {
                f.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("config.json.corrupt-")
            })
            .collect()
    }

    #[test]
    fn non_object_root_is_moved_aside_and_defaults_used() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "[1,2]").unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get("language"), Some(json!("en")));
        let corrupt = corrupt_files(&p);
        assert_eq!(corrupt.len(), 1);
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), "[1,2]");
    }

    #[test]
    fn invalid_json_is_never_overwritten() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"customAgents": [ {"id": "a"#).unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get("customAgents"), Some(json!([])));
        let corrupt = corrupt_files(&p);
        assert_eq!(corrupt.len(), 1);
        assert_eq!(
            fs::read_to_string(&corrupt[0]).unwrap(),
            r#"{"customAgents": [ {"id": "a"#
        );
    }

    #[test]
    fn lone_surrogates_load_like_json_parse() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(
            &p,
            r#"{"customAgents":[{"id":"a","name":"cut \ud83d"}],"language":"ja"}"#,
        )
        .unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get("language"), Some(json!("ja")));
        assert_eq!(
            s.get("customAgents"),
            Some(json!([{"id": "a", "name": "cut \u{fffd}"}]))
        );
        assert!(corrupt_files(&p).is_empty());
    }

    #[test]
    fn concurrent_external_write_is_not_clobbered() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        // Another process (the Electron build) writes a key after we opened.
        let mut on_disk: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        on_disk["customAgents"] = json!([{"id": "from-electron"}]);
        fs::write(&p, serde_json::to_string(&on_disk).unwrap()).unwrap();
        s.set("language", json!("ja")).unwrap();
        let reopened = Store::open(&p).unwrap();
        assert_eq!(
            reopened.get("customAgents"),
            Some(json!([{"id": "from-electron"}]))
        );
        assert_eq!(reopened.get("language"), Some(json!("ja")));
    }

    #[test]
    fn get_sees_external_writes() {
        let (_d, p) = tmp();
        let s = Store::open(&p).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut on_disk: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        on_disk["language"] = json!("ja");
        fs::write(&p, serde_json::to_string(&on_disk).unwrap()).unwrap();
        assert_eq!(s.get("language"), Some(json!("ja")));
        assert_eq!(s.all()["language"], json!("ja"));
    }

    #[test]
    fn corruption_after_open_keeps_last_known_values() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        s.set("customAgents", json!([{"id": "mine"}])).unwrap();
        fs::write(&p, "{garbage").unwrap();
        s.set("language", json!("ja")).unwrap();
        assert_eq!(s.get("customAgents"), Some(json!([{"id": "mine"}])));
        assert_eq!(s.get("language"), Some(json!("ja")));
        let corrupt = corrupt_files(&p);
        assert_eq!(corrupt.len(), 1);
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), "{garbage");
    }

    #[test]
    fn unreadable_file_is_not_written() {
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        fs::remove_file(&p).unwrap();
        fs::create_dir(&p).unwrap(); // read fails with an I/O error
        assert!(s.set("language", json!("ja")).is_err());
        assert!(p.is_dir());
        // Reads fall back to the last known values.
        assert_eq!(s.get("language"), Some(json!("en")));
    }

    #[test]
    fn detached_store_merges_into_file_on_write() {
        let (_d, p) = tmp();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"customAgents":[{"id":"x"}]}"#).unwrap();
        let mut s = Store::detached(&p, &Env::current());
        assert_eq!(s.get("customAgents"), Some(json!([{"id": "x"}])));
        s.set("language", json!("ja")).unwrap();
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(
            on_disk,
            json!({"customAgents": [{"id": "x"}], "language": "ja"})
        );
    }

    #[test]
    fn listener_gets_top_level_changes() {
        use std::sync::{Arc, Mutex};
        let (_d, p) = tmp();
        let mut s = Store::open(&p).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        s.set_listener(move |k, v| sink.lock().unwrap().push((k.to_string(), v.cloned())));
        s.set("aws.region", json!("eu-west-1")).unwrap();
        s.delete("aws.region").unwrap();
        s.delete("missing").unwrap();
        s.delete("language").unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].0, "aws");
        assert_eq!(seen[0].1.as_ref().unwrap()["region"], json!("eu-west-1"));
        assert!(seen[1].1.as_ref().unwrap().get("region").is_none());
        assert_eq!(seen[2], ("language".to_string(), None));
    }

    #[test]
    fn default_path_shape() {
        let p = default_path("bedrock-engineer").unwrap();
        assert!(p.ends_with(Path::new("bedrock-engineer").join("config.json")));
    }
}
