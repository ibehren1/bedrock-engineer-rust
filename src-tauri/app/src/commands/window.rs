//! `window.api.window.openTaskHistory` (`src/main/handlers/window-handlers.ts`): the task
//! execution history window for a background task. Also opened when a task's OS notification
//! is clicked (see `crate::notify`).
//!
//! Like Electron, the window is created on demand, reused (navigated to the new task) while it
//! is open, and destroyed when closed.
//!
//! `window.workspaceFiles.popOut` → `window_open_file_viewer`: a project file in its own window
//! (the Files panel's preview and editor, without the rest of the app). One window per file,
//! labelled `file-viewer-<hash of the root-relative path>`; opening a file that already has a
//! window focuses it.

use serde_json::{json, Value};
use std::path::Path;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

use crate::state::StoreMutex;
use crate::window_startup::{fit_to_screen, ShowWhenLoaded};

/// Window label (also listed in `capabilities/default.json`).
pub const TASK_HISTORY_LABEL: &str = "task-history";

/// Default and minimum inner size of the task history window (Electron's 1400x900, 800x600).
const SIZE: (f64, f64) = (1400.0, 900.0);
const MIN_SIZE: (f64, f64) = (800.0, 600.0);

/// The hash route of the task history page.
pub fn task_history_route(task_id: &str) -> String {
    format!("/background-agent/task-history/{task_id}")
}

/// Open (or reuse) the task history window. `{ success, windowId, reused }` or
/// `{ success: false, error }`; `windowId` is the window label.
pub fn open_task_history(app: &AppHandle, task_id: &str) -> Value {
    tracing::info!(task_id, "Opening task history window");
    let route = task_history_route(task_id);

    if let Some(window) = app.get_webview_window(TASK_HISTORY_LABEL) {
        let reused = window.url().and_then(|mut url| {
            url.set_fragment(Some(&route));
            window.navigate(url)?;
            let _ = window.show();
            let _ = window.unminimize();
            window.set_focus()
        });
        match reused {
            Ok(()) => {
                return json!({ "success": true, "windowId": TASK_HISTORY_LABEL, "reused": true })
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to reuse task history window");
                let _ = window.destroy();
            }
        }
    }

    let built = build_app_window(
        app,
        TASK_HISTORY_LABEL,
        &route,
        "Task Execution History",
        SIZE,
        MIN_SIZE,
    );
    match built {
        Ok(_) => json!({ "success": true, "windowId": TASK_HISTORY_LABEL, "reused": false }),
        Err(e) => {
            tracing::error!(error = %e, "Failed to open task history window");
            json!({ "success": false, "error": e.to_string() })
        }
    }
}

/// Build a secondary window showing the app page at hash `route`: hidden until loaded, links
/// open in the default browser, navigation kept on the app's pages, fitted and centered.
fn build_app_window(
    app: &AppHandle,
    label: &str,
    route: &str,
    title: &str,
    size: (f64, f64),
    min_size: (f64, f64),
) -> tauri::Result<WebviewWindow> {
    let handle = app.clone();
    // Hidden until the page has loaded, like the main window (no blank flash).
    let reveal = ShowWhenLoaded::new();
    let builder = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html#{route}").into()),
    )
    .title(title)
    .inner_size(size.0, size.1)
    .min_inner_size(min_size.0, min_size.1)
    .resizable(true);
    let window = reveal
        .attach(crate::webview_proxy::configure(app, builder))
        // Links open in the default browser, as in the main window.
        .on_new_window(move |url, _features| {
            if let Err(e) = handle.opener().open_url(url.as_str(), None::<&str>) {
                tracing::error!(url = %url, error = %e, "Failed to open external URL");
            }
            NewWindowResponse::Deny
        })
        .on_navigation(crate::security::navigation_guard(app))
        .build()?;
    crate::webview_proxy::attach(&window);
    fit_to_screen(&window, size, min_size);
    reveal.arm_fallback(&window);
    Ok(window)
}

/// Label prefix of the pop-out file viewer windows (also in `capabilities/default.json`).
pub const FILE_VIEWER_LABEL_PREFIX: &str = "file-viewer-";

/// Default and minimum inner size of a file viewer window.
const FILE_VIEWER_SIZE: (f64, f64) = (900.0, 800.0);
const FILE_VIEWER_MIN_SIZE: (f64, f64) = (420.0, 320.0);

/// Whether `label` is a file viewer window.
pub fn is_file_viewer_label(label: &str) -> bool {
    label
        .strip_prefix(FILE_VIEWER_LABEL_PREFIX)
        .is_some_and(|rest| !rest.is_empty())
}

/// The window label for the root-relative `path`: a short stable hash (FNV-1a, 48 bits as hex),
/// since labels only allow `a-zA-Z0-9-/:_`.
pub fn file_viewer_label(path: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{FILE_VIEWER_LABEL_PREFIX}{:012x}", hash & 0xffff_ffff_ffff)
}

/// `encodeURIComponent`.
fn encode_component(s: &str) -> String {
    const KEEP: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'!')
        .remove(b'~')
        .remove(b'*')
        .remove(b'\'')
        .remove(b'(')
        .remove(b')');
    percent_encoding::utf8_percent_encode(s, KEEP).to_string()
}

/// The hash route of the file viewer page for the root-relative `path`.
pub fn file_viewer_route(path: &str) -> String {
    format!("/file-viewer?path={}", encode_component(path))
}

/// `<file name> — <project folder name>`.
pub fn file_viewer_title(root: &Path, path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match root.file_name() {
        Some(folder) => format!("{name} — {}", folder.to_string_lossy()),
        None => name.to_string(),
    }
}

/// Open the file viewer window for `path` (root-relative or absolute, under the project folder),
/// or focus it if it is already open. `{ windowId, reused }`.
pub fn open_file_viewer(app: &AppHandle, store: &StoreMutex, path: &str) -> Result<Value, String> {
    let root = crate::commands::workspace_files::root(store)?;
    let root = workspace_files::canonical_root(&root)?;
    let abs = workspace_files::resolve_in_root(&root, path)?;
    if !abs.is_file() {
        return Err(workspace_files::Error::NotFound(path.to_string()).to_string());
    }
    let rel = workspace_files::relative_to(&root, &abs)
        .ok_or_else(|| workspace_files::Error::Outside.to_string())?;
    let label = file_viewer_label(&rel);

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.unminimize();
        match window.set_focus() {
            Ok(()) => return Ok(json!({ "windowId": label, "reused": true })),
            Err(e) => {
                tracing::error!(error = %e, "Failed to focus the file viewer window");
                let _ = window.destroy();
            }
        }
    }

    tracing::info!(window = %label, "Opening file viewer window");
    build_app_window(
        app,
        &label,
        &file_viewer_route(&rel),
        &file_viewer_title(&root, &rel),
        FILE_VIEWER_SIZE,
        FILE_VIEWER_MIN_SIZE,
    )
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to open file viewer window");
        e.to_string()
    })?;
    Ok(json!({ "windowId": label, "reused": false }))
}

/// `workspaceFiles.popOut`. Async so the window is built off the main thread.
#[tauri::command]
pub async fn window_open_file_viewer(
    app: AppHandle,
    store: State<'_, StoreMutex>,
    path: String,
) -> Result<Value, String> {
    open_file_viewer(&app, &store, &path)
}

/// `window:openTaskHistory`. Async so the window is built off the main thread (building a
/// window from a synchronous command can deadlock on Windows).
#[tauri::command]
pub async fn window_open_task_history(app: AppHandle, task_id: String) -> Result<Value, String> {
    Ok(open_task_history(&app, &task_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route() {
        assert_eq!(
            task_history_route("task_123"),
            "/background-agent/task-history/task_123"
        );
    }

    #[test]
    fn file_viewer_labels() {
        let a = file_viewer_label("docs/report.md");
        assert_eq!(a, file_viewer_label("docs/report.md"), "stable");
        assert_ne!(a, file_viewer_label("docs/report2.md"));
        assert_eq!(a.len(), FILE_VIEWER_LABEL_PREFIX.len() + 12);
        assert!(a.starts_with(FILE_VIEWER_LABEL_PREFIX));
        // Tauri labels: a-zA-Z0-9-/:_ only, even for paths with spaces and non-ASCII.
        let b = file_viewer_label("Mes documents/été 2026 (final).docx");
        assert!(b
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-/:_".contains(c)));
        assert!(is_file_viewer_label(&a));
        assert!(is_file_viewer_label(&b));
        assert!(!is_file_viewer_label("file-viewer-"));
        assert!(!is_file_viewer_label("main"));
        assert!(!is_file_viewer_label("task-history"));
    }

    #[test]
    fn file_viewer_route_and_title() {
        assert_eq!(
            file_viewer_route("docs/a b+c&d=é.md"),
            "/file-viewer?path=docs%2Fa%20b%2Bc%26d%3D%C3%A9.md"
        );
        assert_eq!(
            file_viewer_route("x(1)'!.txt"),
            "/file-viewer?path=x(1)'!.txt"
        );
        assert_eq!(
            file_viewer_title(Path::new("/Users/me/project"), "src/main.rs"),
            "main.rs — project"
        );
        assert_eq!(file_viewer_title(Path::new("/"), "a.txt"), "a.txt");
    }
}
