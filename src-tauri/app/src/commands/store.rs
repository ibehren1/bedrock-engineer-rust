//! `window.store.*` → `store_*` (port of `src/preload/store.ts`).

use crate::state::{lock_store, StoreMutex};
use serde_json::Value;
use tauri::State;

/// Reads for webviews: AWS secrets come back blanked (see [`crate::secrets`]).
#[tauri::command]
pub fn store_get(state: State<'_, StoreMutex>, key: String) -> Result<Option<Value>, String> {
    Ok(crate::secrets::redact(&key, lock_store(&state)?.get(&key)))
}

/// Writes from the main window. A blank secret (AWS, proxy, Tavily) keeps the saved one.
#[tauri::command]
pub fn store_set(state: State<'_, StoreMutex>, key: String, value: Value) -> Result<(), String> {
    let mut store = lock_store(&state)?;
    let value = crate::secrets::merge(&key, value, store.get(&key).as_ref());
    store.set(&key, value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_delete(state: State<'_, StoreMutex>, key: String) -> Result<(), String> {
    lock_store(&state)?.delete(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_all(state: State<'_, StoreMutex>) -> Result<Value, String> {
    Ok(crate::secrets::redact_all(lock_store(&state)?.all()))
}
