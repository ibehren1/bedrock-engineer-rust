//! Access to the app's config store (`src/preload/store.ts`, the `store` crate) and the
//! electron-store–compatible side files the background agent keeps next to it.

use common::json_file::ConfFile;
use serde_json::{Map, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// `context.store.get` / `context.store.set` on the main config store.
pub trait ConfigStore: Send + Sync {
    /// `store.get(key)`; `None` when unset.
    fn get(&self, key: &str) -> Option<Value>;
    /// `store.set(key, value)`.
    fn set(&self, key: &str, value: Value) -> Result<(), String>;
}

impl ConfigStore for Mutex<store::Store> {
    fn get(&self, key: &str) -> Option<Value> {
        self.lock().unwrap().get(key)
    }
    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        self.lock()
            .unwrap()
            .set(key, value)
            .map_err(|e| e.to_string())
    }
}

impl<T: ConfigStore + ?Sized> ConfigStore for Arc<T> {
    fn get(&self, key: &str) -> Option<Value> {
        (**self).get(key)
    }
    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        (**self).set(key, value)
    }
}

/// A process-local [`ConfigStore`] (tests, or callers without a config file).
#[derive(Debug, Default)]
pub struct MemoryConfigStore {
    data: Mutex<Map<String, Value>>,
}

impl MemoryConfigStore {
    pub fn new(data: Value) -> Self {
        MemoryConfigStore {
            data: Mutex::new(data.as_object().cloned().unwrap_or_default()),
        }
    }

    /// The whole object (`store.all()`).
    pub fn all(&self) -> Value {
        Value::Object(self.data.lock().unwrap().clone())
    }
}

impl ConfigStore for MemoryConfigStore {
    fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        self.data.lock().unwrap().insert(key.to_string(), value);
        Ok(())
    }
}

/// A named electron-store file (`new Store({ name, defaults })`): `<dir>/<name>.json`, a JSON
/// object written with tab indentation, atomically. Keys other than the ones this crate uses are
/// kept as they are.
///
/// Like electron-store there is no in-memory copy: [`get`](Self::get) reads the file and
/// [`set`](Self::set) re-reads it right before writing, so concurrent writes by the Electron build
/// survive. A file that fails to parse is renamed to `<name>.json.corrupt-<unix-ms>` rather than
/// overwritten; any other read error makes `set` fail without writing
/// ([`common::json_file::ConfFile`]).
#[derive(Debug)]
pub(crate) struct JsonFile {
    file: ConfFile,
}

impl JsonFile {
    /// Open `<dir>/<name>.json`, filling in `key` with `default` when it is missing (electron-store
    /// writes its defaults on construction).
    pub(crate) fn open(dir: &Path, name: &str, key: &str, default: Value) -> Self {
        let path = dir.join(format!("{name}.json"));
        let mut defaults = Map::new();
        defaults.insert(key.to_string(), default);
        let file = ConfFile::open(&path, &defaults).unwrap_or_else(|e| {
            tracing::warn!(path = %path.display(), error = %e, "Failed to open store file");
            ConfFile::new(&path)
        });
        JsonFile { file }
    }

    /// The current value of `key` in the file.
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        self.file.get(key)
    }

    /// One read-modify-write of `key`; nothing is written when the file can't be read.
    pub(crate) fn update_key<R>(
        &mut self,
        key: &str,
        f: impl FnOnce(Option<Value>) -> (Value, R),
    ) -> std::io::Result<R> {
        self.file.update(|d| {
            let (v, r) = f(d.get(key).cloned());
            d.insert(key.to_string(), v);
            r
        })
    }
}
