//! The global memory list: rules and facts the user asked the assistant to remember.
//!
//! Stored in `<userData>/memory.json` as `{ "items": MemoryItem[] }` (2-space JSON, newest first),
//! next to `config.json` and `chat-sessions/`. The list is cached in memory behind a mutex; each
//! change writes the whole file atomically (temp file + rename) before it is applied to the cache.
//!
//! A missing file is an empty list. A file that doesn't parse is moved aside as
//! `memory.json.corrupt-<unix-ms>` and the list starts empty. A file that exists but can't be
//! read is never written over: changes fail until the app restarts with a readable file.

use crate::util::js;
use common::json_file::{parse_lenient, quarantine, to_json_indented, write_atomic};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// File name under userData.
pub const MEMORY_FILE: &str = "memory.json";

/// What an item is: an instruction on how to behave, or a fact to keep in mind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    Rule,
    #[default]
    Memory,
}

impl MemoryKind {
    pub const ALL: [&'static str; 2] = ["rule", "memory"];

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "rule" => Ok(MemoryKind::Rule),
            "memory" => Ok(MemoryKind::Memory),
            other => Err(format!(
                "Invalid memory kind \"{other}\"; expected \"rule\" or \"memory\""
            )),
        }
    }
}

/// Who created an item: the model (memory tools) or the user (Memory page).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemorySource {
    Chat,
    #[default]
    Manual,
}

impl MemorySource {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "chat" => Ok(MemorySource::Chat),
            "manual" => Ok(MemorySource::Manual),
            other => Err(format!(
                "Invalid memory source \"{other}\"; expected \"chat\" or \"manual\""
            )),
        }
    }
}

/// One memory item, as the renderer sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String,
    pub content: String,
    pub kind: MemoryKind,
    pub enabled: bool,
    pub source: MemorySource,
    /// Unix ms.
    pub created_at: i64,
    /// Unix ms.
    pub updated_at: i64,
}

/// Fields to change in [`MemoryService::update`]; `None` keeps the current value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemoryPatch {
    pub content: Option<String>,
    pub kind: Option<MemoryKind>,
    pub enabled: Option<bool>,
}

/// Called with the full list after every successful write.
pub type MemoryListener = Arc<dyn Fn(&[MemoryItem]) + Send + Sync>;

#[derive(Deserialize)]
struct MemoryFile {
    #[serde(default)]
    items: Vec<MemoryItem>,
}

struct State {
    items: Vec<MemoryItem>,
    /// Set when the file exists but couldn't be read; writes are refused so it isn't clobbered.
    read_error: Option<String>,
}

/// The memory list shared by the memory tools and the app's `memory_*` commands.
pub struct MemoryService {
    path: PathBuf,
    state: Mutex<State>,
    listener: Mutex<Option<MemoryListener>>,
}

impl MemoryService {
    /// Loads `<user_data>/memory.json`.
    pub fn open(user_data: &Path) -> Self {
        let path = user_data.join(MEMORY_FILE);
        let state = load(&path);
        MemoryService {
            path,
            state: Mutex::new(state),
            listener: Mutex::new(None),
        }
    }

    /// Replaces the change listener. It runs while the list is locked (so notifications arrive
    /// in write order) and must not call back into the service.
    pub fn set_listener(&self, listener: impl Fn(&[MemoryItem]) + Send + Sync + 'static) {
        *self.listener.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(listener));
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// All items, newest first.
    pub fn list(&self) -> Vec<MemoryItem> {
        self.state().items.clone()
    }

    pub fn get(&self, id: &str) -> Option<MemoryItem> {
        self.state().items.iter().find(|i| i.id == id).cloned()
    }

    /// Adds an enabled item at the top of the list. `content` is trimmed and must not be empty.
    pub fn add(
        &self,
        content: &str,
        kind: MemoryKind,
        source: MemorySource,
    ) -> Result<MemoryItem, String> {
        let content = non_empty(content)?;
        self.mutate(|items| {
            let now = js::now_millis();
            let item = MemoryItem {
                id: uuid::Uuid::new_v4().to_string(),
                content,
                kind,
                enabled: true,
                source,
                created_at: now,
                updated_at: now,
            };
            items.insert(0, item.clone());
            Ok((item, true))
        })
    }

    /// Applies `patch` to item `id` and bumps its `updatedAt`.
    pub fn update(&self, id: &str, patch: MemoryPatch) -> Result<MemoryItem, String> {
        let content = patch.content.as_deref().map(non_empty).transpose()?;
        self.mutate(|items| {
            let item = items
                .iter_mut()
                .find(|i| i.id == id)
                .ok_or_else(|| format!("Memory item with ID \"{id}\" not found"))?;
            if let Some(content) = content {
                item.content = content;
            }
            if let Some(kind) = patch.kind {
                item.kind = kind;
            }
            if let Some(enabled) = patch.enabled {
                item.enabled = enabled;
            }
            item.updated_at = js::now_millis();
            Ok((item.clone(), true))
        })
    }

    /// Removes item `id`; `None` (and no write) if there is no such item.
    pub fn delete(&self, id: &str) -> Result<Option<MemoryItem>, String> {
        self.mutate(|items| match items.iter().position(|i| i.id == id) {
            Some(index) => Ok((Some(items.remove(index)), true)),
            None => Ok((None, false)),
        })
    }

    /// Runs `f` on a copy of the list; when it reports a change, writes the copy, makes it the
    /// current list and notifies the listener.
    fn mutate<R>(
        &self,
        f: impl FnOnce(&mut Vec<MemoryItem>) -> Result<(R, bool), String>,
    ) -> Result<R, String> {
        let mut state = self.state();
        if let Some(e) = &state.read_error {
            return Err(format!(
                "Memory file {} could not be read ({e}); not overwriting it",
                self.path.display()
            ));
        }
        let mut items = state.items.clone();
        let (result, changed) = f(&mut items)?;
        if !changed {
            return Ok(result);
        }
        let bytes = to_json_indented(&json!({ "items": items }), b"  ")
            .map_err(|e| format!("Failed to save memory: {e}"))?;
        write_atomic(&self.path, &bytes).map_err(|e| {
            tracing::error!(path = %self.path.display(), error = %e, "Failed to write memory file");
            format!("Failed to save memory: {e}")
        })?;
        state.items = items;
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(listener) = listener {
            listener(&state.items);
        }
        Ok(result)
    }
}

fn non_empty(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    Ok(trimmed.to_string())
}

fn load(path: &Path) -> State {
    let empty = |read_error| State {
        items: Vec::new(),
        read_error,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return empty(None),
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "Failed to read memory file");
            return empty(Some(e.to_string()));
        }
    };
    match parse_lenient::<MemoryFile>(&text) {
        Ok(file) => {
            let mut items = file.items;
            // Newest first, whatever order the file had.
            items.sort_by_key(|i| std::cmp::Reverse(i.created_at));
            State {
                items,
                read_error: None,
            }
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "Memory file is corrupt; starting with an empty list");
            match quarantine(path) {
                Ok(_) => empty(None),
                // Couldn't move it aside: leave it alone rather than overwrite it.
                Err(e) => empty(Some(e.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn add(s: &MemoryService, content: &str) -> MemoryItem {
        s.add(content, MemoryKind::Memory, MemorySource::Manual)
            .unwrap()
    }

    #[test]
    fn crud() {
        let dir = tempfile::tempdir().unwrap();
        let s = MemoryService::open(dir.path());
        assert!(s.list().is_empty());

        let a = s
            .add("  Use tabs  ", MemoryKind::Rule, MemorySource::Chat)
            .unwrap();
        assert_eq!(a.content, "Use tabs");
        assert_eq!(a.kind, MemoryKind::Rule);
        assert_eq!(a.source, MemorySource::Chat);
        assert!(a.enabled);
        assert_eq!(a.created_at, a.updated_at);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = add(&s, "Name is Ada");
        // Newest first.
        let ids: Vec<_> = s.list().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, [b.id.clone(), a.id.clone()]);

        std::thread::sleep(std::time::Duration::from_millis(2));
        let u = s
            .update(
                &a.id,
                MemoryPatch {
                    content: Some("Use spaces".into()),
                    enabled: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(u.content, "Use spaces");
        assert!(!u.enabled);
        assert_eq!(u.kind, MemoryKind::Rule);
        assert!(u.updated_at > u.created_at);
        assert_eq!(s.get(&a.id), Some(u));

        assert_eq!(s.delete(&a.id).unwrap().map(|i| i.id), Some(a.id.clone()));
        assert_eq!(s.delete(&a.id).unwrap(), None);
        assert_eq!(s.list(), vec![b]);
    }

    #[test]
    fn rejects_empty_content_and_unknown_ids() {
        let dir = tempfile::tempdir().unwrap();
        let s = MemoryService::open(dir.path());
        assert_eq!(
            s.add(" \n ", MemoryKind::Memory, MemorySource::Manual),
            Err("Memory content cannot be empty".into())
        );
        let a = add(&s, "x");
        assert_eq!(
            s.update(
                &a.id,
                MemoryPatch {
                    content: Some(" ".into()),
                    ..Default::default()
                }
            ),
            Err("Memory content cannot be empty".into())
        );
        assert_eq!(
            s.update("nope", MemoryPatch::default()),
            Err("Memory item with ID \"nope\" not found".into())
        );
        assert_eq!(s.list(), vec![a]);
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let s = MemoryService::open(dir.path());
        let a = add(&s, "first");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = s
            .add("second", MemoryKind::Rule, MemorySource::Chat)
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join(MEMORY_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["items"][0]["content"], "second");
        assert_eq!(v["items"][0]["kind"], "rule");
        assert_eq!(v["items"][0]["source"], "chat");
        assert!(v["items"][0]["createdAt"].is_i64());
        assert!(v["items"][0]["updatedAt"].is_i64());

        let reopened = MemoryService::open(dir.path());
        assert_eq!(reopened.list(), vec![b, a]);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MEMORY_FILE), "{ not json").unwrap();
        let s = MemoryService::open(dir.path());
        assert!(s.list().is_empty());
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("memory.json.corrupt-")),
            "{names:?}"
        );
        // And writing works again.
        add(&s, "x");
        assert_eq!(MemoryService::open(dir.path()).list().len(), 1);
    }

    #[test]
    fn listener_gets_the_full_list_after_each_write() {
        let dir = tempfile::tempdir().unwrap();
        let s = MemoryService::open(dir.path());
        let calls = Arc::new(Mutex::new(Vec::<usize>::new()));
        let seen = calls.clone();
        s.set_listener(move |items| seen.lock().unwrap().push(items.len()));
        let a = add(&s, "a");
        add(&s, "b");
        s.update(
            &a.id,
            MemoryPatch {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        s.delete(&a.id).unwrap();
        // No-ops and failures don't notify.
        s.delete(&a.id).unwrap();
        let _ = s.add("", MemoryKind::Memory, MemorySource::Manual);
        assert_eq!(*calls.lock().unwrap(), [1, 2, 2, 1]);
    }

    #[test]
    fn failed_write_leaves_the_list_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let s = MemoryService::open(dir.path());
        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        s.set_listener(move |_| {
            f.fetch_add(1, Ordering::SeqCst);
        });
        // A directory in the file's place: the rename over it fails.
        std::fs::create_dir(dir.path().join(MEMORY_FILE)).unwrap();
        assert!(s
            .add("x", MemoryKind::Memory, MemorySource::Manual)
            .unwrap_err()
            .starts_with("Failed to save memory"));
        assert!(s.list().is_empty());
        assert_eq!(fired.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unreadable_file_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(MEMORY_FILE)).unwrap();
        let s = MemoryService::open(dir.path());
        assert!(s.list().is_empty());
        assert!(s
            .add("x", MemoryKind::Memory, MemorySource::Manual)
            .unwrap_err()
            .contains("could not be read"));
        assert!(dir.path().join(MEMORY_FILE).is_dir());
    }

    #[test]
    fn parses_kinds_and_sources() {
        assert_eq!(MemoryKind::parse("rule"), Ok(MemoryKind::Rule));
        assert!(MemoryKind::parse("Rule").is_err());
        assert_eq!(MemorySource::parse("chat"), Ok(MemorySource::Chat));
        assert!(MemorySource::parse("x").is_err());
    }
}
