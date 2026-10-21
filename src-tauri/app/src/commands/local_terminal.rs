//! `window.api.localTerminal.*`: interactive shells on the user's own machine for the chat
//! page's Terminal tab, over [`local_terminal::LocalTerminalManager`].
//!
//! Only the main window's capability (`capabilities/main-window.json`) grants these, and no tool
//! calls them, so the model cannot reach the shells. Output goes out as pub/sub events on
//! `local-terminal:<terminalId>` to the main window only (see `backend::LocalTerminalEvents`);
//! byte payloads (live frames and backlogs) are `number[]` on the wire, like the sandbox
//! terminal's, and the bridge turns backlogs into `Uint8Array`s.

use crate::backend::Backend;
use crate::errors;
use crate::state::{project_path, StoreMutex};
use local_terminal::{Backlog, OpenTerminalResult, TerminalInfo};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

/// A new shell for chat `sessionId`, in the project folder (the settings' `projectPath`, read
/// here rather than taken from the renderer) or the home directory. Nothing is published until
/// `local_terminal_attach`.
#[tauri::command]
pub async fn local_terminal_open(
    app: AppHandle,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<OpenTerminalResult, String> {
    let cwd =
        local_terminal::shell::resolve_cwd(project_path(&app.state::<StoreMutex>()).as_deref());
    // Spawning forks and execs; keep it off the async workers.
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Backend>()
            .local_terminals
            .open(&session_id, cols, rows, &cwd)
    })
    .await
    .map_err(errors::plain)?
    .map_err(errors::plain)
}

/// Start (or keep) publishing; `{ backlog: number[], exited, exitCode }`. Safe to call again
/// whenever the renderer resubscribes.
#[tauri::command]
pub fn local_terminal_attach(
    backend: State<'_, Backend>,
    terminal_id: String,
) -> Result<Backlog, String> {
    backend
        .local_terminals
        .attach(&terminal_id)
        .map_err(errors::plain)
}

#[tauri::command]
pub fn local_terminal_input(
    backend: State<'_, Backend>,
    terminal_id: String,
    data: String,
) -> Value {
    backend.local_terminals.write(&terminal_id, &data);
    json!({ "success": true })
}

#[tauri::command]
pub fn local_terminal_resize(
    backend: State<'_, Backend>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<Value, String> {
    backend
        .local_terminals
        .resize(&terminal_id, cols, rows)
        .map_err(errors::plain)?;
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub fn local_terminal_close(backend: State<'_, Backend>, terminal_id: String) -> Value {
    backend.local_terminals.close(&terminal_id);
    json!({ "success": true })
}

/// The chat's shells in the order they were opened, running or exited.
#[tauri::command]
pub fn local_terminal_list(backend: State<'_, Backend>, session_id: String) -> Vec<TerminalInfo> {
    backend.local_terminals.list(&session_id)
}

#[tauri::command]
pub fn local_terminal_backlog(backend: State<'_, Backend>, terminal_id: String) -> Backlog {
    backend.local_terminals.backlog(&terminal_id)
}

/// Chat deletion: end that chat's shells. A no-op before the backend is up.
pub(crate) fn close_session_terminals(app: &AppHandle, session_ids: &[String]) {
    if let Some(backend) = app.try_state::<Backend>() {
        for id in session_ids {
            backend.local_terminals.close_session(id);
        }
    }
}

/// Deleting every chat: end every shell (in the background; `close_all` waits for them).
pub(crate) fn close_all_terminals(app: &AppHandle) {
    if let Some(backend) = app.try_state::<Backend>() {
        let terminals = backend.local_terminals.clone();
        tauri::async_runtime::spawn_blocking(move || terminals.close_all());
    }
}
