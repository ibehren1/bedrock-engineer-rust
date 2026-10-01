//! `window.file.*` and the file/dialog `window.api.*` methods: ports of
//! `src/main/handlers/file-handlers.ts` and the self-contained parts of `src/preload/file.ts`.
//! Filesystem logic lives in [`crate::files`].
//!
//! Chat exports: `save_chat_to_docx` only writes the bytes (html-to-docx runs in the renderer,
//! `src/renderer/src/lib/docx/htmlToDocx.ts`); `save_chat_to_pdf` prints the HTML with the
//! system webview ([`crate::pdf`]).

use crate::files::{self, AgentList, ExportImage, ExportResult};
use crate::state::{lock_store, project_path, project_path_or_cwd, StoreMutex};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

const CATEGORY: &str = "file:ipc";

pub(crate) fn file_path_string(p: FilePath) -> Option<String> {
    p.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}

/// `open-file`: pick one file; `undefined` when canceled.
#[tauri::command]
pub async fn open_file(window: WebviewWindow) -> Result<Option<String>, String> {
    Ok(window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("openFile...")
        .blocking_pick_file()
        .and_then(file_path_string))
}

/// `open-directory`: pick a folder; a new choice also becomes the project path, as in Electron
/// (the bridge mirrors that into its store cache).
#[tauri::command]
pub async fn open_directory(
    window: WebviewWindow,
    store: State<'_, StoreMutex>,
) -> Result<Option<String>, String> {
    let path = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Select Directory")
        .set_can_create_directories(true)
        .blocking_pick_folder()
        .and_then(file_path_string);
    if let Some(p) = &path {
        let mut store = lock_store(&store)?;
        if store
            .get("projectPath")
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            != Some(p.as_str())
        {
            store
                .set("projectPath", Value::String(p.clone()))
                .map_err(|e| e.to_string())?;
            tracing::info!(category = CATEGORY, new_path = %p, "Project path changed");
        }
    }
    Ok(path)
}

/// `get-local-image`: a data URL for a local image file; rejects when it can't be read.
#[tauri::command]
pub async fn get_local_image(path: String) -> Result<String, String> {
    files::local_image_data_url(&path).map_err(|e| {
        tracing::error!(category = CATEGORY, path = %path, error = %e, "Failed to read image");
        e.to_string()
    })
}

#[tauri::command]
pub async fn read_project_ignore(project_path: String) -> Result<Value, String> {
    files::read_project_ignore(Path::new(&project_path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_project_ignore(project_path: String, content: String) -> Result<Value, String> {
    Ok(files::write_project_ignore(
        Path::new(&project_path),
        &content,
    ))
}

/// `save-chat-to-markdown` (`window.file.exportChatMarkdown`).
#[tauri::command]
pub async fn save_chat_to_markdown(
    store: State<'_, StoreMutex>,
    title: String,
    markdown: String,
    images: Vec<ExportImage>,
) -> Result<ExportResult, String> {
    let project = project_path_or_cwd(&store);
    Ok(files::save_chat_to_markdown(
        &project, &title, &markdown, &images,
    ))
}

/// `save-chat-to-docx` (`window.file.exportChatDocx`): the renderer converted the HTML with
/// html-to-docx; write the document to `<projectPath>/<title>/<title>.docx`.
#[tauri::command]
pub async fn save_chat_to_docx(
    store: State<'_, StoreMutex>,
    title: String,
    docx_base64: String,
) -> Result<ExportResult, String> {
    let project = project_path_or_cwd(&store);
    let bytes = files::decode_base64_lenient(&docx_base64);
    Ok(files::save_chat_document(
        &project,
        &title,
        "docx",
        Ok(bytes),
    ))
}

/// `save-chat-to-pdf` (`window.file.exportChatPdf`): print the self-contained HTML document to
/// `<projectPath>/<title>/<title>.pdf`.
#[tauri::command]
pub async fn save_chat_to_pdf(
    app: AppHandle,
    store: State<'_, StoreMutex>,
    title: String,
    html: String,
) -> Result<ExportResult, String> {
    let project = project_path_or_cwd(&store);
    let (export_dir, _) = files::chat_export_paths(&project, &title, "pdf");
    // Like Electron, the folder exists before the document is rendered.
    if let Err(e) = std::fs::create_dir_all(&export_dir) {
        return Ok(files::save_chat_document(
            &project,
            &title,
            "pdf",
            Err(e.to_string()),
        ));
    }
    let pdf = crate::pdf::render_html_to_pdf(&app, html).await;
    Ok(files::save_chat_document(&project, &title, "pdf", pdf))
}

/// `window.file.readSharedAgents`.
#[tauri::command]
pub async fn file_read_shared_agents(store: State<'_, StoreMutex>) -> Result<AgentList, String> {
    Ok(files::read_shared_agents(project_path(&store).as_deref()))
}

/// `window.file.readDirectoryAgents`: the source tree's `directory-agents` in development, the
/// bundled resource folder otherwise.
#[tauri::command]
pub async fn file_read_directory_agents(app: AppHandle) -> Result<AgentList, String> {
    Ok(files::read_directory_agents(&directory_agents_dir(&app)))
}

fn directory_agents_dir(app: &AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/renderer/src/assets/directory-agents")
    } else {
        app.path()
            .resource_dir()
            .map(|d| d.join("directory-agents"))
            .unwrap_or_default()
    }
}
