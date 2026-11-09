//! `window.api.chatAttachments.*` → `chat_attachments_*` (port of
//! `src/main/handlers/chat-attachments-handlers.ts`). `projectPath` / `userDataPath` are read
//! from the store on every call.
//!
//! Every command is `async` and does its filesystem work in `spawn_blocking`: these run on a
//! timer and on window focus for the whole chat list, and a sync command would run on the main
//! thread and freeze the window (see docs/port/BRIDGE.md).

use crate::backend::DocumentExtractor;
use crate::commands::file::file_path_string;
use crate::errors;
use crate::settings::attachment_paths;
use crate::state::{store_all, HistoryState, StoreMutex};
use attachments::handlers::{self, ChatFolder, OpenFolderResult, PickerAddResult, WithFilesResult};
use attachments::{
    AttachmentAddResult, AttachmentContextResult, AttachmentListing, AttachmentRemoveResult,
    NewAttachment, RemovedFolder, RemovedFolders, RenameResult, TidyInput, TidyResult,
};
use std::collections::HashSet;
use std::path::PathBuf;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn chat_attachments_list(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<AttachmentListing, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::list(&paths, &session_id)).await
}

#[tauri::command]
pub async fn chat_attachments_with_files(
    store: State<'_, StoreMutex>,
    session_ids: Option<Vec<String>>,
) -> Result<WithFilesResult, String> {
    let paths = attachment_paths(&store_all(&store));
    let ids = session_ids.unwrap_or_default();
    blocking(move || handlers::with_files(&paths, &ids)).await
}

/// Files arrive as `{ name, bytes: number[] }` (BRIDGE.md rule 6).
#[tauri::command]
pub async fn chat_attachments_add(
    store: State<'_, StoreMutex>,
    session_id: String,
    files: Vec<NewAttachment>,
) -> Result<AttachmentAddResult, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::add(&paths, &session_id, &files).map_err(errors::plain)).await?
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
pub async fn chat_attachments_remove(
    store: State<'_, StoreMutex>,
    session_id: String,
    name: String,
) -> Result<AttachmentRemoveResult, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::remove(&paths, &session_id, &name).map_err(errors::plain)).await?
}

#[tauri::command]
pub async fn chat_attachments_remove_all(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<RemovedFolder, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::remove_all(&paths, &session_id).map_err(errors::plain)).await?
}

#[tauri::command]
pub async fn chat_attachments_remove_every_folder(
    store: State<'_, StoreMutex>,
) -> Result<RemovedFolders, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::remove_every_folder(&paths).map_err(errors::plain)).await?
}

/// Rename the chat's folder to follow its new title, and point the chat's list of files
/// (`touchedFiles`, which the Files panel shows) at the new path.
#[tauri::command]
pub async fn chat_attachments_rename(
    store: State<'_, StoreMutex>,
    history: State<'_, HistoryState>,
    session_id: String,
) -> Result<RenameResult, String> {
    let paths = attachment_paths(&store_all(&store));
    let touched = history.lock()?.touched_files(&session_id);
    let (result, moved) = {
        let session_id = session_id.clone();
        blocking(move || {
            let result = handlers::rename(&paths, &session_id);
            let moved = attachments::follow_chat_dir_rename(&paths, &session_id, &touched);
            (result, moved)
        })
        .await?
    };
    if let Some(moved) = moved {
        history.lock()?.set_touched_files(&session_id, &moved);
    }
    Ok(result)
}

/// Where this chat's files belong (`<project>/chats/<chat-name>`), for the system prompt and
/// the Files panel. Creates nothing.
#[tauri::command]
pub async fn chat_attachments_folder(
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<ChatFolder, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || handlers::folder(&paths, &session_id).map_err(errors::plain)).await?
}

/// Move an existing chat's loose files into its folder (`chats/<chat>/`), run when the chat is
/// opened. Files another chat lists, files git tracks, and files older than the chat stay put;
/// see the `attachments` crate's `tidy` module.
#[tauri::command]
pub async fn chat_attachments_tidy(
    store: State<'_, StoreMutex>,
    history: State<'_, HistoryState>,
    session_id: String,
) -> Result<TidyResult, String> {
    let paths = attachment_paths(&store_all(&store));
    let (touched, others, since) = {
        let history = history.lock()?;
        let touched = history.touched_files(&session_id);
        let (created, updated) = history.session_times(&session_id).unwrap_or((0.0, 0.0));
        let others = history.touched_files_of_overlapping_sessions(&session_id, created, updated);
        (touched, others, created)
    };
    if touched.is_empty() {
        return Ok(TidyResult {
            directory: None,
            moved: vec![],
            touched,
            changed: false,
        });
    }

    let project = paths.project_path.clone();
    let result = {
        let session_id = session_id.clone();
        blocking(move || {
            let tracked = project
                .as_deref()
                .map(git_tracked_files)
                .unwrap_or_default();
            handlers::tidy(
                &paths,
                &session_id,
                &TidyInput {
                    touched: &touched,
                    others: &others,
                    tracked: &tracked,
                    since_ms: Some(since),
                },
            )
            .map_err(errors::plain)
        })
        .await??
    };
    if result.changed {
        history
            .lock()?
            .set_touched_files(&session_id, &result.touched);
    }
    Ok(result)
}

/// Project-relative paths git tracks (including staged ones), empty when the folder is not a
/// repository or git is not installed. A tracked file is the user's, never moved.
fn git_tracked_files(project: &std::path::Path) -> HashSet<String> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z", "--cached"])
        .current_dir(project)
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => HashSet::new(),
    }
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
pub async fn chat_attachments_open_folder(
    app: tauri::AppHandle,
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<OpenFolderResult, String> {
    let paths = attachment_paths(&store_all(&store));
    blocking(move || {
        handlers::open_folder(&paths, &session_id, |dir| {
            app.opener()
                .open_path(dir, None::<&str>)
                .map_err(|e| e.to_string())
        })
    })
    .await
}

/// Run `work` on the blocking pool, so filesystem work never runs on the main thread.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(errors::plain)
}
