//! `window.store.*` → `store_*` (port of `src/preload/store.ts`).

use crate::state::{lock_store, StoreMutex};
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub fn store_get(state: State<'_, StoreMutex>, key: String) -> Result<Option<Value>, String> {
    Ok(lock_store(&state)?.get(&key))
}

#[tauri::command]
pub fn store_set(state: State<'_, StoreMutex>, key: String, value: Value) -> Result<(), String> {
    lock_store(&state)?
        .set(&key, value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_delete(state: State<'_, StoreMutex>, key: String) -> Result<(), String> {
    lock_store(&state)?.delete(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_all(state: State<'_, StoreMutex>) -> Result<Value, String> {
    Ok(lock_store(&state)?.all())
}
