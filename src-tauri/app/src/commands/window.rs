//! `window.api.window.openTaskHistory` (`src/main/handlers/window-handlers.ts`): the task
//! execution history window for a background task. Also opened when a task's OS notification
//! is clicked (see `crate::notify`).
//!
//! Like Electron, the window is created on demand, reused (navigated to the new task) while it
//! is open, and destroyed when closed.

use serde_json::{json, Value};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

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

    let handle = app.clone();
    // Hidden until the page has loaded, like the main window (no blank flash).
    let reveal = ShowWhenLoaded::new();
    let builder = WebviewWindowBuilder::new(
        app,
        TASK_HISTORY_LABEL,
        WebviewUrl::App(format!("index.html#{route}").into()),
    )
    .title("Task Execution History")
    .inner_size(SIZE.0, SIZE.1)
    .min_inner_size(MIN_SIZE.0, MIN_SIZE.1);
    let built = reveal
        .attach(crate::webview_proxy::configure(app, builder))
        // Links open in the default browser, as in the main window.
        .on_new_window(move |url, _features| {
            if let Err(e) = handle.opener().open_url(url.as_str(), None::<&str>) {
                tracing::error!(url = %url, error = %e, "Failed to open external URL");
            }
            NewWindowResponse::Deny
        })
        .on_navigation(crate::security::navigation_guard(app))
        .build();
    match built {
        Ok(window) => {
            crate::webview_proxy::attach(&window);
            fit_to_screen(&window, SIZE, MIN_SIZE);
            reveal.arm_fallback(&window);
            json!({ "success": true, "windowId": TASK_HISTORY_LABEL, "reused": false })
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to open task history window");
            json!({ "success": false, "error": e.to_string() })
        }
    }
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
}
