//! `window.api.screen` and the `screen:*` IPC channels (`src/main/handlers/screen-handlers.ts`),
//! over the same `tools::ScreenService` the screenCapture tool uses.

use crate::backend::Backend;
use tauri::State;
use tools::system::{PermissionCheckResult, ScreenCaptureOptions, ScreenCaptureResult, WindowInfo};

/// `screen:list-available-windows`: `[]` on failure.
#[tauri::command]
pub async fn screen_list_available_windows(
    backend: State<'_, Backend>,
) -> Result<Vec<WindowInfo>, String> {
    let screen = backend.screen.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || screen.list_available_windows())
            .await
            .unwrap_or_default(),
    )
}

/// `screen:capture`; rejects with the handler's error message.
#[tauri::command]
pub async fn screen_capture(
    backend: State<'_, Backend>,
    options: Option<ScreenCaptureOptions>,
) -> Result<ScreenCaptureResult, String> {
    let screen = backend.screen.clone();
    let options = options.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || screen.capture(&options))
        .await
        .map_err(|e| e.to_string())?
}

/// `screen:check-permissions`.
#[tauri::command]
pub async fn screen_check_permissions(
    backend: State<'_, Backend>,
) -> Result<PermissionCheckResult, String> {
    let screen = backend.screen.clone();
    tauri::async_runtime::spawn_blocking(move || screen.check_permissions())
        .await
        .map_err(|e| e.to_string())
}
