//! `window.api.camera` (`src/main/handlers/camera-handlers.ts`) and the webview side of the
//! cameraCapture tool.
//!
//! - Preview windows: frameless, always-on-top webviews loading `camera-preview.html`
//!   (`?deviceId=&deviceName=`), one per camera, like the Electron `BrowserWindow`s.
//! - Tool frames: [`WebviewCamera`] implements `tools::CameraSource` by emitting
//!   `camera:capture-request` `{ requestId, deviceId?, quality, format }` to the main window,
//!   whose bridge grabs a frame with `getUserMedia` (the TS tool's `CameraStreamManager`) and
//!   answers with `camera_capture_response { requestId, frame? , error? }`.
//!
//! Opacity: macOS sets the `NSWindow` alpha (Electron's `setOpacity`); elsewhere the window is
//! transparent and the page's opacity is set with CSS.

use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::oneshot;
use tools::system::{
    save_captured_image, CameraCaptureRequest, CameraCaptureResult, CameraFrameRequest,
    CameraSource, CapturedFrame,
};

/// Main → webview frame request event.
pub const CAPTURE_REQUEST_EVENT: &str = "camera:capture-request";
/// The TS capture waited 1 s for the camera to settle and timed out after 10 s; this bounds
/// the whole round trip (permission prompt included).
const FRAME_TIMEOUT: Duration = Duration::from_secs(60);

/// Window label prefix of the preview windows (see `capabilities/camera-preview.json`).
pub(crate) const PREVIEW_LABEL_PREFIX: &str = "camera-preview-";

/// `PREVIEW_SIZES`.
const PREVIEW_SIZES: [(&str, f64, f64); 3] = [
    ("small", 200.0, 150.0),
    ("medium", 320.0, 240.0),
    ("large", 480.0, 360.0),
];

fn preview_size(name: &str) -> Option<(f64, f64)> {
    PREVIEW_SIZES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, w, h)| (*w, *h))
}

/// `CameraPreviewOptions`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPreviewOptions {
    pub size: Option<String>,
    pub opacity: Option<f64>,
    pub position: Option<String>,
    #[allow(dead_code)]
    pub device_id: Option<String>,
    #[allow(dead_code)]
    pub device_name: Option<String>,
    pub camera_ids: Option<Vec<String>>,
    pub layout: Option<String>,
}

struct Preview {
    device_id: String,
    label: String,
    opacity: f64,
}

/// Pending tool frame requests.
#[derive(Default)]
pub struct PendingFrames {
    next: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<CapturedFrame, String>>>>,
}

impl PendingFrames {
    fn insert(&self) -> (String, oneshot::Receiver<Result<CapturedFrame, String>>) {
        let id = format!("camera_{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id.clone(), tx);
        }
        (id, rx)
    }

    fn remove(&self, id: &str) -> Option<oneshot::Sender<Result<CapturedFrame, String>>> {
        self.pending.lock().ok()?.remove(id)
    }

    /// Resolve a request; `false` when it is unknown (timed out or answered already).
    pub fn resolve(&self, id: &str, result: Result<CapturedFrame, String>) -> bool {
        match self.remove(id) {
            Some(tx) => tx.send(result).is_ok(),
            None => false,
        }
    }
}

/// Managed camera state: preview windows in creation order (Electron's `Map`) and the
/// pending tool frames.
#[derive(Default)]
pub struct CameraState {
    previews: Mutex<Vec<Preview>>,
    next_label: AtomicU64,
    pub frames: Arc<PendingFrames>,
}

impl CameraState {
    fn previews(&self) -> std::sync::MutexGuard<'_, Vec<Preview>> {
        self.previews.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `tools::CameraSource` backed by the main webview.
pub struct WebviewCamera {
    app: AppHandle,
    frames: Arc<PendingFrames>,
}

impl WebviewCamera {
    pub fn new(app: AppHandle, frames: Arc<PendingFrames>) -> Self {
        Self { app, frames }
    }
}

/// The event payload for one request.
fn capture_request_payload(request_id: &str, req: &CameraFrameRequest) -> Value {
    let mut v = serde_json::to_value(req).unwrap_or_else(|_| json!({}));
    v["requestId"] = json!(request_id);
    v
}

#[async_trait]
impl CameraSource for WebviewCamera {
    async fn capture_frame(&self, req: &CameraFrameRequest) -> Result<CapturedFrame, String> {
        if self.app.get_webview_window("main").is_none() {
            return Err("getUserMedia API is not available in this browser".to_string());
        }
        let (id, rx) = self.frames.insert();
        if let Err(e) = self.app.emit_to(
            "main",
            CAPTURE_REQUEST_EVENT,
            capture_request_payload(&id, req),
        ) {
            self.frames.remove(&id);
            return Err(e.to_string());
        }
        match tokio::time::timeout(FRAME_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Camera capture cancelled".to_string()),
            Err(_) => {
                self.frames.remove(&id);
                Err("Camera capture timeout".to_string())
            }
        }
    }
}

/// The main window's answer to `camera:capture-request`.
#[tauri::command]
pub fn camera_capture_response(
    camera: State<'_, CameraState>,
    request_id: String,
    frame: Option<CapturedFrame>,
    error: Option<String>,
) -> bool {
    let result = match (frame, error) {
        (Some(f), None) => Ok(f),
        (_, Some(e)) => Err(e),
        (None, None) => Err("Camera capture failed".to_string()),
    };
    camera.frames.resolve(&request_id, result)
}

/// `camera:save-captured-image` (to `os.tmpdir()` unless `outputPath`); rejects like the
/// handler threw.
#[tauri::command]
pub async fn camera_save_captured_image(
    request: CameraCaptureRequest,
) -> Result<CameraCaptureResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        save_captured_image(&request, &std::env::temp_dir())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------------------------
// Preview window layout (pure, `calculatePreviewPosition` / `calculateGridPositions`)
// ---------------------------------------------------------------------------------------------

const MARGIN: f64 = 20.0;

/// `calculatePreviewPosition(size, position, index)` within the primary display's work-area
/// size (logical pixels).
pub fn preview_position(
    size: (f64, f64),
    position: Option<&str>,
    index: usize,
    work_area: (f64, f64),
) -> (f64, f64) {
    let offset = index as f64 * 30.0;
    let (sw, sh) = work_area;
    let right = sw - size.0 - MARGIN - offset;
    let bottom = sh - size.1 - MARGIN - offset;
    let near = MARGIN + offset;
    match position.unwrap_or("bottom-right") {
        "bottom-left" => (near, bottom),
        "top-right" => (right, near),
        "top-left" => (near, near),
        _ => (right, bottom),
    }
}

/// `calculateGridPositions(size, count, startPosition)`.
pub fn grid_positions(
    size: (f64, f64),
    count: usize,
    start: Option<&str>,
    work_area: (f64, f64),
) -> Vec<(f64, f64)> {
    let start = start.unwrap_or("bottom-right");
    let gap = 10.0;
    let cols = (count as f64).sqrt().ceil().max(1.0) as usize;
    let rows = count.div_ceil(cols);
    let base_x = if start.contains("right") {
        work_area.0 - (cols as f64 * (size.0 + gap) + MARGIN - gap)
    } else {
        MARGIN
    };
    let base_y = if start.contains("bottom") {
        work_area.1 - (rows as f64 * (size.1 + gap) + MARGIN - gap)
    } else {
        MARGIN
    };
    (0..count)
        .map(|i| {
            let (col, row) = (i % cols, i / cols);
            (
                base_x + col as f64 * (size.0 + gap),
                base_y + row as f64 * (size.1 + gap),
            )
        })
        .collect()
}

/// `screen.getPrimaryDisplay().workAreaSize` in logical pixels.
fn work_area_size(app: &AppHandle) -> (f64, f64) {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let s = m.work_area().size.to_logical::<f64>(m.scale_factor());
            (s.width, s.height)
        })
        .unwrap_or((1440.0, 900.0))
}

fn preview_url(device_id: &str, device_name: &str) -> WebviewUrl {
    WebviewUrl::App(
        format!(
            "camera-preview.html?deviceId={}&deviceName={}",
            utf8_percent_encode(device_id, NON_ALPHANUMERIC),
            utf8_percent_encode(device_name, NON_ALPHANUMERIC)
        )
        .into(),
    )
}

/// `document.documentElement.style.opacity = <o>` (non-macOS opacity).
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn opacity_script(opacity: f64) -> String {
    format!("document.documentElement.style.opacity = '{opacity}'")
}

fn set_opacity(window: &tauri::WebviewWindow, opacity: f64) {
    #[cfg(target_os = "macos")]
    {
        // NSWindow.alphaValue, on the main thread.
        if let Ok(ptr) = window.ns_window() {
            let ptr = ptr as usize;
            let _ = window.run_on_main_thread(move || {
                // SAFETY: `ptr` is the live NSWindow of this window; AppKit is touched on the
                // main thread only.
                unsafe {
                    let ns_window = &*(ptr as *const objc2::runtime::AnyObject);
                    let _: () = objc2::msg_send![ns_window, setAlphaValue: opacity];
                }
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.eval(opacity_script(opacity));
    }
}

fn create_preview_window(
    app: &AppHandle,
    label: &str,
    device_id: &str,
    device_name: &str,
    size: (f64, f64),
    position: (f64, f64),
    opacity: f64,
) -> tauri::Result<tauri::WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, label, preview_url(device_id, device_name))
        .title(device_name)
        .inner_size(size.0, size.1)
        .position(position.0, position.1)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .skip_taskbar(true)
        .focused(false);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.transparent(true).initialization_script(format!(
        "window.addEventListener('DOMContentLoaded', () => {{ {} }})",
        opacity_script(opacity)
    ));
    // Loads no remote content, but on Windows every webview must share the main window's
    // proxy setting (see `webview_proxy`).
    let window = crate::webview_proxy::configure(app, builder).build()?;
    crate::webview_proxy::attach(&window);
    // Windows / Linux attach the app menu bar to every window; the frameless preview has none
    // (as in Electron).
    #[cfg(not(target_os = "macos"))]
    let _ = window.remove_menu();
    #[cfg(target_os = "macos")]
    set_opacity(&window, opacity);

    // previewWindow.on('closed') → cameraPreviewWindows.delete(deviceId)
    let handle = app.clone();
    let (owned_label, owned_id, owned_name) = (
        label.to_string(),
        device_id.to_string(),
        device_name.to_string(),
    );
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            if let Some(state) = handle.try_state::<CameraState>() {
                state.previews().retain(|p| p.label != owned_label);
            }
            tracing::info!(device_id = %owned_id, device_name = %owned_name, "Camera preview window closed");
        }
    });
    Ok(window)
}

fn close_window(app: &AppHandle, label: &str) -> bool {
    match app.get_webview_window(label) {
        Some(w) => w.close().is_ok(),
        None => false,
    }
}

fn result(success: bool, message: impl Into<String>) -> Value {
    json!({ "success": success, "message": message.into() })
}

/// `camera:show-preview-window`.
#[tauri::command]
pub async fn camera_show_preview_window(
    app: AppHandle,
    camera: State<'_, CameraState>,
    options: Option<CameraPreviewOptions>,
) -> Result<Value, String> {
    let options = options.unwrap_or_default();
    tracing::info!(?options, "Showing camera preview windows");
    let size_name = options.size.clone().unwrap_or_else(|| "medium".into());
    let Some(size) = preview_size(&size_name) else {
        return Ok(result(false, format!("Invalid preview size: {size_name}")));
    };
    let opacity = options.opacity.filter(|o| *o != 0.0).unwrap_or(0.9);
    let requested = options
        .camera_ids
        .clone()
        .unwrap_or_else(|| vec!["default".into()]);
    let layout = options.layout.clone().unwrap_or_else(|| {
        if requested.len() > 1 {
            "cascade"
        } else {
            "single"
        }
        .to_string()
    });

    // Close the existing previews.
    let old: Vec<String> = camera.previews().drain(..).map(|p| p.label).collect();
    for label in old {
        close_window(&app, &label);
    }

    let mut ids: Vec<String> = requested
        .iter()
        .filter(|id| !id.is_empty())
        .cloned()
        .collect();
    if ids.is_empty() {
        ids.push("default".into());
    }
    let work_area = work_area_size(&app);
    let positions = if layout == "grid" && ids.len() > 1 {
        grid_positions(size, ids.len(), options.position.as_deref(), work_area)
    } else {
        (0..ids.len())
            .map(|i| preview_position(size, options.position.as_deref(), i, work_area))
            .collect()
    };

    let mut success_count = 0;
    let mut errors = Vec::new();
    for (i, device_id) in ids.iter().enumerate() {
        let device_name = format!("Camera {}", i + 1);
        let label = format!(
            "{PREVIEW_LABEL_PREFIX}{}",
            camera.next_label.fetch_add(1, Ordering::Relaxed) + 1
        );
        match create_preview_window(
            &app,
            &label,
            device_id,
            &device_name,
            size,
            positions[i],
            opacity,
        ) {
            Ok(_) => {
                camera.previews().push(Preview {
                    device_id: device_id.clone(),
                    label,
                    opacity,
                });
                success_count += 1;
                tracing::info!(device_id = %device_id, device_name = %device_name, "Camera preview window created");
            }
            Err(e) => {
                tracing::error!(device_id = %device_id, error = %e, "Failed to create preview window for camera");
                errors.push(format!("Camera {device_id}: {e}"));
            }
        }
    }

    if success_count == 0 {
        return Ok(result(
            false,
            format!(
                "Failed to create any preview windows. Errors: {}",
                errors.join("; ")
            ),
        ));
    }
    Ok(result(
        true,
        if success_count == ids.len() {
            format!("{success_count} preview window(s) created successfully")
        } else {
            format!(
                "{success_count} of {} preview window(s) created ({} failed)",
                ids.len(),
                errors.len()
            )
        },
    ))
}

/// `camera:hide-preview-window`.
#[tauri::command]
pub fn camera_hide_preview_window(app: AppHandle, camera: State<'_, CameraState>) -> Value {
    let labels: Vec<String> = camera.previews().drain(..).map(|p| p.label).collect();
    let closed = labels.iter().filter(|l| close_window(&app, l)).count();
    tracing::info!(
        closed_count = closed,
        "Camera preview windows hidden successfully"
    );
    result(
        true,
        if closed > 0 {
            format!("{closed} preview window(s) closed")
        } else {
            "No preview windows active".to_string()
        },
    )
}

/// `camera:close-preview-window`.
#[tauri::command]
pub fn camera_close_preview_window(
    app: AppHandle,
    camera: State<'_, CameraState>,
    device_id: String,
) -> Value {
    let label = {
        let mut previews = camera.previews();
        let idx = previews
            .iter()
            .position(|p| p.device_id == device_id && app.get_webview_window(&p.label).is_some());
        idx.map(|i| previews.remove(i).label)
    };
    match label {
        Some(label) if close_window(&app, &label) => result(
            true,
            format!("Preview window for device {device_id} closed successfully"),
        ),
        _ => result(
            false,
            format!("Preview window for device {device_id} not found or already closed"),
        ),
    }
}

/// `camera:update-preview-settings`.
#[tauri::command]
pub fn camera_update_preview_settings(
    app: AppHandle,
    camera: State<'_, CameraState>,
    options: Option<CameraPreviewOptions>,
) -> Value {
    let options = options.unwrap_or_default();
    let mut previews = camera.previews();
    if previews.is_empty() {
        return result(false, "No active preview windows");
    }
    let work_area = work_area_size(&app);
    let mut updated = 0;
    for p in previews.iter_mut() {
        let Some(window) = app.get_webview_window(&p.label) else {
            continue;
        };
        if let Some(size) = options.size.as_deref().and_then(preview_size) {
            let _ = window.set_size(LogicalSize::new(size.0, size.1));
        }
        if let Some(position) = options.position.as_deref() {
            let current = window
                .inner_size()
                .ok()
                .zip(window.scale_factor().ok())
                .map(|(s, f)| {
                    let l = s.to_logical::<f64>(f);
                    (l.width, l.height)
                })
                .unwrap_or((320.0, 240.0));
            let (x, y) = preview_position(current, Some(position), updated, work_area);
            let _ = window.set_position(LogicalPosition::new(x, y));
        }
        if let Some(opacity) = options.opacity {
            set_opacity(&window, opacity);
            p.opacity = opacity;
        }
        updated += 1;
    }
    result(true, format!("{updated} preview window(s) updated"))
}

/// `camera:get-preview-status`.
#[tauri::command]
pub fn camera_get_preview_status(app: AppHandle, camera: State<'_, CameraState>) -> Value {
    let windows: Vec<Value> = camera
        .previews()
        .iter()
        .filter_map(|p| {
            let window = app.get_webview_window(&p.label)?;
            let logical = window
                .inner_size()
                .ok()
                .zip(window.scale_factor().ok())
                .map(|(s, f)| s.to_logical::<f64>(f));
            let size = logical
                .and_then(|l| {
                    PREVIEW_SIZES
                        .iter()
                        .find(|(_, w, h)| (l.width - w).abs() < 0.5 && (l.height - h).abs() < 0.5)
                })
                .map(|(n, _, _)| *n)
                .unwrap_or("medium");
            Some(json!({ "deviceId": p.device_id, "size": size, "opacity": p.opacity }))
        })
        .collect();
    json!({ "isActive": !windows.is_empty(), "count": windows.len(), "windows": windows })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: (f64, f64) = (1440.0, 875.0);

    #[test]
    fn cascade_positions_match_ts() {
        let s = (320.0, 240.0);
        assert_eq!(preview_position(s, None, 0, WORK), (1100.0, 615.0));
        assert_eq!(
            preview_position(s, Some("bottom-right"), 1, WORK),
            (1070.0, 585.0)
        );
        assert_eq!(
            preview_position(s, Some("bottom-left"), 0, WORK),
            (20.0, 615.0)
        );
        assert_eq!(
            preview_position(s, Some("top-right"), 2, WORK),
            (1040.0, 80.0)
        );
        assert_eq!(preview_position(s, Some("top-left"), 1, WORK), (50.0, 50.0));
        assert_eq!(
            preview_position(s, Some("middle"), 0, WORK),
            (1100.0, 615.0)
        );
    }

    #[test]
    fn grid_positions_match_ts() {
        let s = (200.0, 150.0);
        // 3 cameras: 2 columns, 2 rows.
        let g = grid_positions(s, 3, Some("bottom-right"), WORK);
        let base_x = 1440.0 - (2.0 * 210.0 + 20.0 - 10.0);
        let base_y = 875.0 - (2.0 * 160.0 + 20.0 - 10.0);
        assert_eq!(
            g,
            vec![
                (base_x, base_y),
                (base_x + 210.0, base_y),
                (base_x, base_y + 160.0)
            ]
        );
        let g = grid_positions(s, 4, Some("top-left"), WORK);
        assert_eq!(g[3], (20.0 + 210.0, 20.0 + 160.0));
    }

    #[test]
    fn preview_sizes_and_options() {
        assert_eq!(preview_size("small"), Some((200.0, 150.0)));
        assert_eq!(preview_size("large"), Some((480.0, 360.0)));
        assert_eq!(preview_size("huge"), None);
        let o: CameraPreviewOptions = serde_json::from_value(json!({
            "size": "small", "opacity": 0.5, "position": "top-left",
            "cameraIds": ["a", "b"], "layout": "grid"
        }))
        .unwrap();
        assert_eq!(o.camera_ids.unwrap(), vec!["a", "b"]);
        assert_eq!(o.layout.as_deref(), Some("grid"));
    }

    #[test]
    fn preview_url_encodes_params() {
        let WebviewUrl::App(p) = preview_url("abc/+=", "Camera 1") else {
            panic!()
        };
        assert_eq!(
            p.to_string_lossy(),
            "camera-preview.html?deviceId=abc%2F%2B%3D&deviceName=Camera%201"
        );
    }

    #[test]
    fn frame_requests_resolve_once() {
        let frames = PendingFrames::default();
        let (id, mut rx) = frames.insert();
        assert_eq!(id, "camera_1");
        let frame = CapturedFrame {
            base64_data: "x".into(),
            width: 1,
            height: 1,
            device_id: "default".into(),
            device_name: "Cam".into(),
        };
        assert!(frames.resolve(&id, Ok(frame.clone())));
        assert!(!frames.resolve(&id, Err("late".into())));
        assert_eq!(rx.try_recv().unwrap(), Ok(frame));
        let (id2, _rx) = frames.insert();
        assert_eq!(id2, "camera_2");
        assert!(!frames.resolve("unknown", Err("x".into())));
    }

    #[test]
    fn capture_request_payload_shape() {
        let v = capture_request_payload(
            "camera_1",
            &CameraFrameRequest {
                device_id: Some("d".into()),
                quality: "high".into(),
                format: "png".into(),
            },
        );
        assert_eq!(
            v,
            json!({"deviceId": "d", "quality": "high", "format": "png", "requestId": "camera_1"})
        );
    }

    #[test]
    fn opacity_css() {
        assert_eq!(
            opacity_script(0.5),
            "document.documentElement.style.opacity = '0.5'"
        );
    }
}
