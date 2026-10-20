//! `window.memory.*` → `memory_*`: the user's side of the global memory list, sharing the memory
//! tools' [`tools::memory::MemoryService`]. Every successful change is pushed to all windows as
//! the `memory-changed` event (payload: the full list), whether it came from here or a tool.

use crate::backend::Backend;
use tauri::State;
use tools::memory::{MemoryItem, MemoryKind, MemoryPatch, MemorySource};

/// Tauri event emitted after every write, with the whole list (newest first) as payload.
pub const MEMORY_CHANGED_EVENT: &str = "memory-changed";

/// All items, newest first.
#[tauri::command]
pub fn memory_list(backend: State<'_, Backend>) -> Vec<MemoryItem> {
    backend.memory.list()
}

/// Adds an enabled item. `kind` defaults to `"memory"`, `source` to `"manual"`.
#[tauri::command]
pub fn memory_add(
    backend: State<'_, Backend>,
    content: String,
    kind: Option<String>,
    source: Option<String>,
) -> Result<MemoryItem, String> {
    let kind = kind
        .as_deref()
        .map(MemoryKind::parse)
        .transpose()?
        .unwrap_or_default();
    let source = source
        .as_deref()
        .map(MemorySource::parse)
        .transpose()?
        .unwrap_or_default();
    backend.memory.add(&content, kind, source)
}

/// Changes the given fields and bumps `updatedAt`; rejects an unknown id.
#[tauri::command]
pub fn memory_update(
    backend: State<'_, Backend>,
    id: String,
    content: Option<String>,
    kind: Option<String>,
    enabled: Option<bool>,
) -> Result<MemoryItem, String> {
    let patch = MemoryPatch {
        content,
        kind: kind.as_deref().map(MemoryKind::parse).transpose()?,
        enabled,
    };
    backend.memory.update(&id, patch)
}

/// `true` if the item existed and was removed.
#[tauri::command]
pub fn memory_delete(backend: State<'_, Backend>, id: String) -> Result<bool, String> {
    backend.memory.delete(&id).map(|removed| removed.is_some())
}
