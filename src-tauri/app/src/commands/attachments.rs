//! `window.api.chatAttachments.*` → `chat_attachments_*` (port of
//! `src/main/handlers/chat-attachments-handlers.ts`). `projectPath` / `userDataPath` are read
//! from the store on every call.

use crate::backend::DocumentExtractor;
use crate::commands::file::file_path_string;
use crate::errors;
use crate::settings::attachment_paths;
use crate::state::{store_all, StoreMutex};
use attachments::handlers::{self, OpenFolderResult, PickerAddResult, WithFilesResult};
use attachments::{
    AttachmentAddResult, AttachmentContextResult, AttachmentListing, AttachmentRemoveResult,
    NewAttachment, RemovedFolder, RemovedFolders, RenameResult,
};
use std::path::PathBuf;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn chat_attachments_list(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> AttachmentListing {
    handlers::list(&attachment_paths(&store_all(&store)), &session_id)
}

#[tauri::command]
pub fn chat_attachments_with_files(
    store: State<'_, StoreMutex>,
    session_ids: Option<Vec<String>>,
) -> WithFilesResult {
    handlers::with_files(
        &attachment_paths(&store_all(&store)),
        &session_ids.unwrap_or_default(),
    )
}

/// Files arrive as `{ name, bytes: number[] }` (BRIDGE.md rule 6).
#[tauri::command]
pub fn chat_attachments_add(
    store: State<'_, StoreMutex>,
    session_id: String,
    files: Vec<NewAttachment>,
) -> Result<AttachmentAddResult, String> {
    handlers::add(&attachment_paths(&store_all(&store)), &session_id, &files).map_err(errors::plain)
}

/// Native multi-select picker; the chosen files are copied into the chat's folder.
#[tauri::command]
pub async fn chat_attachments_add_from_picker(
    window: WebviewWindow,
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<PickerAddResult, String> {
    let picked: Option<Vec<PathBuf>> = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Add files to this chat")
        .blocking_pick_files()
        .map(|paths| {
            paths
                .into_iter()
                .filter_map(file_path_string)
                .map(PathBuf::from)
                .collect()
        });
    handlers::add_from_picker(
        &attachment_paths(&store_all(&store)),
        &session_id,
        picked.as_deref(),
    )
    .map_err(errors::plain)
}

#[tauri::command]
pub fn chat_attachments_remove(
    store: State<'_, StoreMutex>,
    session_id: String,
    name: String,
) -> Result<AttachmentRemoveResult, String> {
    handlers::remove(&attachment_paths(&store_all(&store)), &session_id, &name)
        .map_err(errors::plain)
}

#[tauri::command]
pub fn chat_attachments_remove_all(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<RemovedFolder, String> {
    handlers::remove_all(&attachment_paths(&store_all(&store)), &session_id).map_err(errors::plain)
}

#[tauri::command]
pub fn chat_attachments_remove_every_folder(
    store: State<'_, StoreMutex>,
) -> Result<RemovedFolders, String> {
    handlers::remove_every_folder(&attachment_paths(&store_all(&store))).map_err(errors::plain)
}

#[tauri::command]
pub fn chat_attachments_rename(store: State<'_, StoreMutex>, session_id: String) -> RenameResult {
    handlers::rename(&attachment_paths(&store_all(&store)), &session_id)
}

/// PDF / DOCX text comes from the `tools` crate's extractors.
#[tauri::command]
pub async fn chat_attachments_build_context(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<AttachmentContextResult, String> {
    let paths = attachment_paths(&store_all(&store));
    // Extraction reads whole files; keep it off the async workers.
    tauri::async_runtime::spawn_blocking(move || {
        handlers::build_context(&paths, &session_id, &DocumentExtractor)
    })
    .await
    .map_err(errors::plain)
}

/// Reveal the chat's folder (created first) in the OS file manager.
#[tauri::command]
pub fn chat_attachments_open_folder(
    app: tauri::AppHandle,
    store: State<'_, StoreMutex>,
    session_id: String,
) -> OpenFolderResult {
    handlers::open_folder(&attachment_paths(&store_all(&store)), &session_id, |dir| {
        app.opener()
            .open_path(dir, None::<&str>)
            .map_err(|e| e.to_string())
    })
}
