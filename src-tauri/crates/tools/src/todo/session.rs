//! Port of `src/main/store/todoSession.ts`.
//!
//! Lists live in `<userData>/todos/session_<id>_todos.json` (2-space JSON); metadata in
//! the electron-store file `<userData>/todo-sessions-meta.json` (tab-indented JSON with
//! `recentTodos`, `metadata`, `activeTodoListId`).

use crate::util::js;
use common::json_file::{is_safe_file_id, parse_lenient, write_atomic, ConfFile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `TodoItem['status']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoItemStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl TodoItemStatus {
    pub const ALL: [&'static str; 4] = ["pending", "in_progress", "completed", "cancelled"];
}

/// `TodoItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub description: String,
    pub status: TodoItemStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// `TodoList`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    pub id: String,
    pub items: Vec<TodoItem>,
    pub created_at: String,
    pub updated_at: String,
    pub session_id: String,
    pub project_path: String,
}

/// `TodoItemUpdate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItemUpdate {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TodoItemStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `TodoMetadata`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoMetadata {
    pub id: String,
    pub session_id: String,
    pub project_path: String,
    pub item_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

/// `TodoUpdateResult`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoUpdateResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_list: Option<TodoList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_list: Option<TodoList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Minimal electron-store over [`ConfFile`]: defaults merged under the file's contents on open;
/// every read goes to disk and every write re-reads the file first (tab-indented, atomic). A file
/// that fails to parse is moved aside as `.corrupt-<unix-ms>`; one that can't be read is never
/// written over.
struct MetaStore {
    file: ConfFile,
}

impl MetaStore {
    fn open(path: PathBuf) -> MetaStore {
        let mut defaults = Map::new();
        defaults.insert("recentTodos".into(), json!([]));
        defaults.insert("metadata".into(), json!({}));
        let file = ConfFile::open(&path, &defaults).unwrap_or_else(|e| {
            tracing::error!(error = %e, "Failed to open todo metadata");
            ConfFile::new(&path)
        });
        MetaStore { file }
    }

    fn get(&self, key: &str) -> Option<Value> {
        self.file.get(key)
    }

    fn set(&mut self, key: &str, value: Value) {
        if let Err(e) = self.file.set(key, value) {
            tracing::error!(error = %e, "Failed to write todo metadata");
        }
    }

    /// One read-modify-write of `key`; skipped (logged) when the file can't be read.
    fn update(&mut self, key: &str, f: impl FnOnce(Option<Value>) -> Value) {
        let result = self.file.update(|d| {
            let v = f(d.get(key).cloned());
            d.insert(key.to_string(), v);
        });
        if let Err(e) = result {
            tracing::error!(error = %e, "Failed to write todo metadata");
        }
    }

    fn metadata(&self) -> Map<String, Value> {
        as_map(self.get("metadata"))
    }

    fn recent(&self) -> Vec<String> {
        as_ids(self.get("recentTodos"))
    }
}

fn as_map(v: Option<Value>) -> Map<String, Value> {
    match v {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn as_ids(v: Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// `TodoSessionManager`.
pub struct TodoSessionManager {
    todos_dir: PathBuf,
    meta: Mutex<MetaStore>,
}

impl TodoSessionManager {
    /// `new TodoSessionManager()` with `store.get('userDataPath')`.
    pub fn new(user_data_path: Option<&str>) -> Result<Self, String> {
        let Some(user_data) = user_data_path.filter(|p| !p.is_empty()) else {
            return Err("userDataPath is not set in store".to_string());
        };
        let todos_dir = Path::new(user_data).join("todos");
        std::fs::create_dir_all(&todos_dir).map_err(|e| e.to_string())?;
        let meta = MetaStore::open(Path::new(user_data).join("todo-sessions-meta.json"));
        let manager = TodoSessionManager {
            todos_dir,
            meta: Mutex::new(meta),
        };
        manager.initialize_metadata();
        Ok(manager)
    }

    fn meta(&self) -> std::sync::MutexGuard<'_, MetaStore> {
        self.meta.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Rebuild metadata from the todo files when it is empty.
    fn initialize_metadata(&self) {
        if !self.meta().metadata().is_empty() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(&self.todos_dir) else {
            return;
        };
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with("_todos.json"))
            .collect();
        names.sort();
        for name in names {
            let file_id = name.replacen("_todos.json", "", 1);
            if let Some(list) = self.read_todo_file(&file_id) {
                self.update_metadata(&file_id, &list);
            }
        }
    }

    /// `<todos>/session_<id>_todos.json`, or `None` (logged) for an id that isn't a plain file
    /// name ([`is_safe_file_id`]), so no id can address a file outside the todos directory.
    fn todo_file_path(&self, session_id: &str) -> Option<PathBuf> {
        if !is_safe_file_id(session_id) {
            tracing::error!("Rejected invalid todo session id {session_id:?}");
            return None;
        }
        let name = if session_id.starts_with("session_") {
            format!("{session_id}_todos.json")
        } else {
            format!("session_{session_id}_todos.json")
        };
        Some(self.todos_dir.join(name))
    }

    fn todo_file_exists(&self, session_id: &str) -> bool {
        self.todo_file_path(session_id).is_some_and(|p| p.exists())
    }

    fn read_todo_file(&self, session_id: &str) -> Option<TodoList> {
        let path = self.todo_file_path(session_id)?;
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse_lenient(&text) {
                Ok(list) => Some(list),
                Err(e) => {
                    tracing::error!(error = %e, "Error reading todo file {session_id}");
                    None
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::error!(error = %e, "Error reading todo file {session_id}");
                }
                None
            }
        }
    }

    /// Atomic (temp file + rename), so a crash mid-write can't leave a truncated list.
    async fn write_todo_file(&self, session_id: &str, list: &TodoList) {
        let Some(path) = self.todo_file_path(session_id) else {
            return;
        };
        let text = match serde_json::to_string_pretty(list) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(error = %e, "Error serializing todo file {session_id}");
                return;
            }
        };
        let result = tokio::task::spawn_blocking(move || write_atomic(&path, text.as_bytes()))
            .await
            .unwrap_or_else(|e| Err(std::io::Error::other(e)));
        if let Err(e) = result {
            tracing::error!(error = %e, "Error writing todo file {session_id}");
        }
    }

    fn update_metadata(&self, session_id: &str, list: &TodoList) {
        let meta = TodoMetadata {
            id: list.id.clone(),
            session_id: list.session_id.clone(),
            project_path: list.project_path.clone(),
            item_count: list.items.len(),
            created_at: list.created_at.clone(),
            updated_at: list.updated_at.clone(),
        };
        let entry = serde_json::to_value(meta).unwrap_or(Value::Null);
        self.meta().update("metadata", |current| {
            let mut m = as_map(current);
            m.insert(session_id.into(), entry);
            Value::Object(m)
        });
    }

    fn update_recent_todos(&self, session_id: &str) {
        self.meta().update("recentTodos", |current| {
            let mut recent: Vec<String> = vec![session_id.to_string()];
            recent.extend(as_ids(current).into_iter().filter(|id| id != session_id));
            recent.truncate(10);
            json!(recent)
        });
    }

    /// `createTodoList`.
    pub async fn create_todo_list(
        &self,
        session_id: &str,
        project_path: &str,
        items: &[String],
    ) -> TodoList {
        let now = js::iso_now();
        let list = TodoList {
            id: format!("todolist-{}", js::now_millis()),
            items: items
                .iter()
                .enumerate()
                .map(|(i, d)| TodoItem {
                    id: format!("task-{}-{i}", js::now_millis()),
                    description: d.clone(),
                    status: TodoItemStatus::Pending,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                })
                .collect(),
            created_at: now.clone(),
            updated_at: now,
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
        };
        if self.todo_file_path(session_id).is_none() {
            // Not persisted (and not recorded in the metadata) under an unsafe id.
            return list;
        }
        self.write_todo_file(session_id, &list).await;
        self.update_metadata(session_id, &list);
        self.update_recent_todos(session_id);
        list
    }

    /// `updateTodoList`. As in TS, updates before a missing id are applied to the list
    /// returned as `currentList` (but not persisted).
    pub async fn update_todo_list(
        &self,
        session_id: &str,
        updates: &[TodoItemUpdate],
    ) -> TodoUpdateResult {
        let Some(mut list) = self.read_todo_file(session_id) else {
            return TodoUpdateResult {
                success: false,
                updated_list: None,
                current_list: None,
                error: Some(
                    "No todo list found. Please initialize a todo list first using todoInit."
                        .to_string(),
                ),
            };
        };
        let now = js::iso_now();
        for update in updates {
            let Some(item) = list.items.iter_mut().find(|i| i.id == update.id) else {
                return TodoUpdateResult {
                    success: false,
                    updated_list: None,
                    error: Some(format!("Task with ID \"{}\" not found", update.id)),
                    current_list: Some(list),
                };
            };
            if let Some(s) = update.status {
                item.status = s;
            }
            if let Some(d) = update.description.as_ref().filter(|d| !d.is_empty()) {
                item.description = d.clone();
            }
            item.updated_at = now.clone();
        }
        list.updated_at = now;
        self.write_todo_file(session_id, &list).await;
        self.update_metadata(session_id, &list);
        self.update_recent_todos(session_id);
        TodoUpdateResult {
            success: true,
            updated_list: Some(list),
            current_list: None,
            error: None,
        }
    }

    /// `getTodoList`.
    pub fn get_todo_list(&self, session_id: &str) -> Option<TodoList> {
        self.read_todo_file(session_id)
    }

    /// `deleteTodoList`.
    pub fn delete_todo_list(&self, session_id: &str) {
        let Some(path) = self.todo_file_path(session_id) else {
            return;
        };
        match std::fs::remove_file(&path) {
            Ok(()) => self.meta().update("metadata", |current| {
                let mut m = as_map(current);
                m.remove(session_id);
                Value::Object(m)
            }),
            Err(e) => tracing::error!(error = %e, "Error deleting todo file {session_id}"),
        }
        self.meta().update("recentTodos", |current| {
            let recent: Vec<String> = as_ids(current)
                .into_iter()
                .filter(|id| id != session_id)
                .collect();
            json!(recent)
        });
    }

    fn parse_meta(v: &Value) -> Option<TodoMetadata> {
        serde_json::from_value(v.clone()).ok()
    }

    /// `getRecentTodos`.
    pub fn get_recent_todos(&self) -> Vec<TodoMetadata> {
        let (recent, meta) = {
            let s = self.meta();
            (s.recent(), s.metadata())
        };
        recent
            .iter()
            .filter_map(|id| meta.get(id).and_then(Self::parse_meta))
            .filter(|m| self.todo_file_exists(&m.session_id))
            .filter(|m| m.item_count > 0)
            .collect()
    }

    /// `getAllTodoMetadata`, newest `updatedAt` first.
    pub fn get_all_todo_metadata(&self) -> Vec<TodoMetadata> {
        let meta = self.meta().metadata();
        let mut all: Vec<TodoMetadata> = meta
            .values()
            .filter_map(Self::parse_meta)
            .filter(|m| self.todo_file_exists(&m.session_id))
            .filter(|m| m.item_count > 0)
            .collect();
        let ts = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.timestamp_millis())
                .unwrap_or(0)
        };
        all.sort_by_key(|m| std::cmp::Reverse(ts(&m.updated_at)));
        all
    }

    /// `setActiveTodoList`. electron-store rejects `undefined`, so clearing fails exactly
    /// as it does in TS.
    pub fn set_active_todo_list(&self, session_id: Option<&str>) -> Result<(), String> {
        let Some(id) = session_id else {
            return Err("Use `delete()` to clear values".to_string());
        };
        self.meta().set("activeTodoListId", json!(id));
        Ok(())
    }

    /// `getActiveTodoListId`.
    pub fn get_active_todo_list_id(&self) -> Option<String> {
        self.meta()
            .get("activeTodoListId")
            .and_then(|v| v.as_str().map(str::to_string))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(dir: &tempfile::TempDir) -> TodoSessionManager {
        TodoSessionManager::new(Some(&dir.path().to_string_lossy())).unwrap()
    }

    #[test]
    fn requires_user_data_path() {
        assert_eq!(
            TodoSessionManager::new(None).err().unwrap(),
            "userDataPath is not set in store"
        );
        assert!(TodoSessionManager::new(Some("")).is_err());
    }

    #[tokio::test]
    async fn create_update_and_query() {
        let dir = tempfile::tempdir().unwrap();
        let m = manager(&dir);
        let list = m
            .create_todo_list("session_1", "/proj", &["a".into(), "b".into()])
            .await;
        assert!(list.id.starts_with("todolist-"));
        assert_eq!(list.items.len(), 2);
        assert!(list.items[1].id.ends_with("-1"));
        assert_eq!(list.items[0].status, TodoItemStatus::Pending);
        assert!(dir.path().join("todos/session_1_todos.json").exists());
        // Session ids without the prefix map to the same naming scheme.
        m.create_todo_list("abc", "/proj", &["x".into()]).await;
        assert!(dir.path().join("todos/session_abc_todos.json").exists());

        let id = list.items[0].id.clone();
        let r = m
            .update_todo_list(
                "session_1",
                &[TodoItemUpdate {
                    id: id.clone(),
                    status: Some(TodoItemStatus::Completed),
                    description: Some("A".into()),
                }],
            )
            .await;
        assert!(r.success);
        let updated = r.updated_list.unwrap();
        assert_eq!(updated.items[0].status, TodoItemStatus::Completed);
        assert_eq!(updated.items[0].description, "A");
        assert_eq!(m.get_todo_list("session_1").unwrap(), updated);

        let r = m
            .update_todo_list(
                "session_1",
                &[TodoItemUpdate {
                    id: "nope".into(),
                    status: None,
                    description: None,
                }],
            )
            .await;
        assert!(!r.success);
        assert_eq!(r.error.as_deref(), Some("Task with ID \"nope\" not found"));
        assert!(r.current_list.is_some());

        let r = m.update_todo_list("missing", &[]).await;
        assert_eq!(
            r.error.as_deref(),
            Some("No todo list found. Please initialize a todo list first using todoInit.")
        );

        let recent = m.get_recent_todos();
        assert_eq!(recent[0].session_id, "session_1");
        assert_eq!(recent.len(), 2);
        assert_eq!(m.get_all_todo_metadata().len(), 2);

        // Metadata is persisted tab-indented, like electron-store.
        let meta = std::fs::read_to_string(dir.path().join("todo-sessions-meta.json")).unwrap();
        assert!(meta.contains("\n\t\"recentTodos\""));

        m.delete_todo_list("session_1");
        assert!(m.get_todo_list("session_1").is_none());
        assert_eq!(m.get_recent_todos().len(), 1);

        assert!(m.set_active_todo_list(Some("abc")).is_ok());
        assert_eq!(m.get_active_todo_list_id().as_deref(), Some("abc"));
        assert_eq!(
            m.set_active_todo_list(None).unwrap_err(),
            "Use `delete()` to clear values"
        );
    }

    #[tokio::test]
    async fn unsafe_ids_touch_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let m = manager(&dir);
        let outside = dir.path().join("session_x_todos.json");
        std::fs::write(&outside, "keep").unwrap();
        m.create_todo_list("../session_x", "/p", &["a".into()])
            .await;
        m.delete_todo_list("../x");
        assert!(m.get_todo_list("../x").is_none());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
        assert!(!dir.path().join("todos").read_dir().unwrap().any(|_| true));
    }

    #[tokio::test]
    async fn writes_are_atomic_and_keep_external_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let m = manager(&dir);
        m.create_todo_list("session_1", "/p", &["a".into()]).await;
        // Another process adds a key and a metadata entry.
        let meta_path = dir.path().join("todo-sessions-meta.json");
        let mut v: Value =
            serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
        v["metadata"]["session_other"] = json!({"id": "x"});
        v["activeTodoListId"] = json!("session_other");
        std::fs::write(&meta_path, v.to_string()).unwrap();
        m.create_todo_list("session_2", "/p", &["b".into()]).await;
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(v["metadata"]["session_other"]["id"], "x");
        assert!(v["metadata"]["session_2"].is_object());
        assert_eq!(
            m.get_active_todo_list_id().as_deref(),
            Some("session_other")
        );
        let names: Vec<String> = std::fs::read_dir(dir.path().join("todos"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "no temp files left: {names:?}");
    }

    #[test]
    fn lone_surrogates_and_corrupt_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let todos = dir.path().join("todos");
        std::fs::create_dir_all(&todos).unwrap();
        std::fs::write(
            todos.join("session_s_todos.json"),
            r#"{"id":"l","items":[{"id":"t","description":"cut \ud83d","status":"pending","createdAt":"a","updatedAt":"b"}],"createdAt":"a","updatedAt":"b","sessionId":"session_s","projectPath":"/p"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("todo-sessions-meta.json"), "{broken").unwrap();
        let m = manager(&dir);
        let list = m.get_todo_list("session_s").unwrap();
        assert_eq!(list.items[0].description, "cut \u{fffd}");
        // The broken metadata file was moved aside, not overwritten.
        let kept: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("todo-sessions-meta.json.corrupt-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(m.get_all_todo_metadata()[0].session_id, "session_s");
    }

    #[tokio::test]
    async fn metadata_is_rebuilt_from_files() {
        let dir = tempfile::tempdir().unwrap();
        {
            let m = manager(&dir);
            m.create_todo_list("session_9", "/p", &["t".into()]).await;
        }
        std::fs::remove_file(dir.path().join("todo-sessions-meta.json")).unwrap();
        let m = manager(&dir);
        assert_eq!(m.get_all_todo_metadata()[0].session_id, "session_9");
    }
}
