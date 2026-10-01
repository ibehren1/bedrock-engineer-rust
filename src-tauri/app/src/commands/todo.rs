//! `window.api.todo.*` (port of `src/main/handlers/todo-handlers.ts`), sharing the todo tools'
//! `TodoSessionManager`. Like the TS handlers these never reject: failures come back as
//! `{ success: false, error }`, `null` or `[]`.

use crate::backend::Backend;
use crate::state::{store_all, StoreMutex};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;
use tools::todo::{TodoItemUpdate, TodoSessionManager};
use tools::ToolSettings;

fn manager(backend: &Backend, store: &StoreMutex) -> Result<Arc<TodoSessionManager>, String> {
    let snapshot = store_all(store);
    let user_data = snapshot
        .get("userDataPath")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    backend.todo.manager(user_data.as_deref())
}

fn failure(e: impl std::fmt::Display) -> Value {
    json!({ "success": false, "error": e.to_string() })
}

/// `todo-init` → `{ success, result, message }`.
#[tauri::command]
pub async fn todo_init(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    session_id: String,
    items: Vec<String>,
) -> Result<Value, String> {
    let settings = ToolSettings::from_store(&store_all(&store));
    Ok(backend
        .todo
        .init_todo_list(&settings, &session_id, &items)
        .await)
}

/// `todo-update` → `TodoUpdateResult`.
#[tauri::command]
pub async fn todo_update(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    session_id: String,
    updates: Vec<TodoItemUpdate>,
) -> Result<Value, String> {
    let settings = ToolSettings::from_store(&store_all(&store));
    Ok(backend
        .todo
        .update_todo_list(&settings, &session_id, &updates)
        .await)
}

/// `get-todo-list` → the list or `null` (also without a session id).
#[tauri::command]
pub fn get_todo_list(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    session_id: Option<String>,
) -> Value {
    let Some(session_id) = session_id.filter(|s| !s.is_empty()) else {
        return Value::Null;
    };
    match manager(&backend, &store) {
        Ok(m) => serde_json::to_value(m.get_todo_list(&session_id)).unwrap_or(Value::Null),
        Err(e) => {
            tracing::error!(error = %e, "Error retrieving TODO list");
            Value::Null
        }
    }
}

/// `delete-todo-list` → `{ success }`.
#[tauri::command]
pub fn delete_todo_list(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Value {
    match manager(&backend, &store) {
        Ok(m) => {
            m.delete_todo_list(&session_id);
            json!({ "success": true })
        }
        Err(e) => failure(e),
    }
}

/// `get-recent-todos` → `TodoMetadata[]`.
#[tauri::command]
pub fn get_recent_todos(backend: State<'_, Backend>, store: State<'_, StoreMutex>) -> Value {
    manager(&backend, &store)
        .ok()
        .and_then(|m| serde_json::to_value(m.get_recent_todos()).ok())
        .unwrap_or_else(|| json!([]))
}

/// `get-all-todo-metadata` → `TodoMetadata[]`.
#[tauri::command]
pub fn get_all_todo_metadata(backend: State<'_, Backend>, store: State<'_, StoreMutex>) -> Value {
    manager(&backend, &store)
        .ok()
        .and_then(|m| serde_json::to_value(m.get_all_todo_metadata()).ok())
        .unwrap_or_else(|| json!([]))
}

/// `set-active-todo-list` → `{ success }`.
#[tauri::command]
pub fn set_active_todo_list(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    session_id: Option<String>,
) -> Value {
    match manager(&backend, &store).and_then(|m| m.set_active_todo_list(session_id.as_deref())) {
        Ok(()) => json!({ "success": true }),
        Err(e) => failure(e),
    }
}

/// `get-active-todo-list-id` → id or `null`.
#[tauri::command]
pub fn get_active_todo_list_id(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
) -> Option<String> {
    manager(&backend, &store)
        .ok()
        .and_then(|m| m.get_active_todo_list_id())
}
