//! Keeping every window's `window.store` cache in step with `config.json`, and making sure a
//! window's queued store writes reach Rust before it goes away.
//!
//! - **`store-changed`**: after every successful `set` / `delete` on the config store — from a
//!   renderer (`store_set`) or from Rust (`open_directory`, the background scheduler, ...) — every
//!   window gets `{ key, value, deleted }` for the top-level key that changed, and the bridge
//!   updates its cache (see `tauriBridge.ts`).
//! - **Flush on close / quit**: the bridge's `store.set` is fire-and-forget, so a window closed
//!   right after a settings change could drop the queued write. Closing a bridge window (and
//!   quitting the app) first sends `store-flush-request { token }`; the bridge drains its write
//!   queue and answers with `store_flush_done { token }`, then the window is destroyed (or the app
//!   exits). A window that doesn't answer within [`FLUSH_TIMEOUT`] is closed anyway.
//! - **Single instance**: a second launch focuses the running app's main window instead of
//!   starting a second backend (two schedulers would run every task twice, and two stores would
//!   race on `config.json`).

use crate::state::StoreMutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, State, Window, WindowEvent};
use tokio::sync::oneshot;

/// Event carrying one config store change to every window.
pub const STORE_CHANGED_EVENT: &str = "store-changed";
/// Event asking a window's bridge to drain its pending store writes.
pub const FLUSH_REQUEST_EVENT: &str = "store-flush-request";
/// How long a window (or the app) waits for the bridge to confirm its writes are flushed.
pub const FLUSH_TIMEOUT: Duration = Duration::from_secs(3);
/// Windows that run the renderer bridge (and so have a store write queue).
const BRIDGE_WINDOWS: [&str; 2] = ["main", "task-history"];

/// The `store-changed` payload.
pub fn change_payload(key: &str, value: Option<&Value>) -> Value {
    json!({ "key": key, "value": value, "deleted": value.is_none() })
}

/// Emit `store-changed` for every write to the config store, whoever makes it.
pub fn install_store_listener<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    match app.state::<StoreMutex>().lock() {
        Ok(mut store) => store.set_listener(move |key, value| {
            crate::webview_proxy::on_store_change(&handle, key, value);
            if let Err(e) = handle.emit(STORE_CHANGED_EVENT, change_payload(key, value)) {
                tracing::warn!(key, error = %e, "Failed to emit store-changed");
            }
        }),
        Err(e) => tracing::error!(error = %e, "Store lock poisoned; store-changed disabled"),
    }
}

/// Pending flush acknowledgements, by token.
#[derive(Default)]
pub struct FlushState {
    next: AtomicU64,
    waiters: Mutex<HashMap<u64, oneshot::Sender<()>>>,
    /// Set once the pre-exit flush ran, so the exit it triggers is let through.
    exit_flushed: AtomicBool,
}

impl FlushState {
    fn register(&self) -> (u64, oneshot::Receiver<()>) {
        let token = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        if let Ok(mut w) = self.waiters.lock() {
            w.insert(token, tx);
        }
        (token, rx)
    }

    fn complete(&self, token: u64) {
        let tx = self.waiters.lock().ok().and_then(|mut w| w.remove(&token));
        if let Some(tx) = tx {
            let _ = tx.send(());
        }
    }

    fn forget(&self, token: u64) {
        if let Ok(mut w) = self.waiters.lock() {
            w.remove(&token);
        }
    }
}

/// Ask the bridge in window `label` to flush; resolves when it confirms or after the timeout.
async fn request_flush<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let state = app.state::<FlushState>();
    let (token, rx) = state.register();
    if app
        .emit_to(label, FLUSH_REQUEST_EVENT, json!({ "token": token }))
        .is_err()
    {
        state.forget(token);
        return;
    }
    if tokio::time::timeout(FLUSH_TIMEOUT, rx).await.is_err() {
        tracing::warn!(
            window = label,
            "Window did not confirm its store writes in time"
        );
    }
    state.forget(token);
}

/// `store_flush_done`: the bridge has drained its write queue for `token`.
#[tauri::command]
pub fn store_flush_done(state: State<'_, FlushState>, token: u64) {
    state.complete(token);
}

/// Window events: hold a bridge window's close until its store writes are flushed.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    if !BRIDGE_WINDOWS.contains(&window.label()) || window.try_state::<FlushState>().is_none() {
        return;
    }
    api.prevent_close();
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        request_flush(window.app_handle(), window.label()).await;
        if let Err(e) = window.destroy() {
            tracing::warn!(window = window.label(), error = %e, "Failed to close window");
        }
    });
}

/// App events: flush every open bridge window before the app exits (Cmd+Q, menu quit, ...).
pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    let RunEvent::ExitRequested { api, code, .. } = event else {
        return;
    };
    let Some(state) = app.try_state::<FlushState>() else {
        return;
    };
    if state.exit_flushed.load(Ordering::SeqCst) {
        return;
    }
    let open: Vec<String> = BRIDGE_WINDOWS
        .iter()
        .filter(|l| app.get_webview_window(l).is_some())
        .map(|l| l.to_string())
        .collect();
    if open.is_empty() {
        return;
    }
    api.prevent_exit();
    let app = app.clone();
    let code = code.unwrap_or(0);
    tauri::async_runtime::spawn(async move {
        for label in &open {
            request_flush(&app, label).await;
        }
        app.state::<FlushState>()
            .exit_flushed
            .store(true, Ordering::SeqCst);
        app.exit(code);
    });
}

/// Second launch: bring the running instance's main window to the front.
pub fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    tracing::info!("Second instance launched; focusing the existing window");
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_shape() {
        assert_eq!(
            change_payload("language", Some(&json!("ja"))),
            json!({ "key": "language", "value": "ja", "deleted": false })
        );
        assert_eq!(
            change_payload("language", None),
            json!({ "key": "language", "value": null, "deleted": true })
        );
    }

    #[tokio::test]
    async fn flush_tokens_complete_once() {
        let s = FlushState::default();
        let (a, rx_a) = s.register();
        let (b, rx_b) = s.register();
        assert_ne!(a, b);
        s.complete(a);
        s.complete(a); // unknown now: no-op
        assert!(rx_a.await.is_ok());
        s.forget(b);
        assert!(rx_b.await.is_err());
    }
}
