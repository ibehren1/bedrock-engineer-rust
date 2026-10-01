//! Port of `ScreenCaptureTool.ts` and `src/main/handlers/screen-handlers.ts`.

use super::{capture_error, file_name_for_log, recognize_into};
use crate::base::Tool;
use crate::bedrock_tools::BedrockBackend;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js;
use async_trait::async_trait;
use base64::Engine;
use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

const NAME: &str = "screenCapture";
const DESCRIPTION: &str = "Capture the current screen and save as an image file. Optionally analyze the captured image with AI to extract text content, identify UI elements, and provide detailed visual descriptions for debugging and documentation purposes.\n\nCapture screen for AI analysis. Useful for debugging, UI analysis, and creating documentation. Available windows for screen capture:{{allowedWindows}}";

/// `desktopCapturer.getSources({ thumbnailSize })` for captures: the image is scaled to fit.
const CAPTURE_MAX: (u32, u32) = (1920, 1080);
/// The window list's preview thumbnail size.
const THUMBNAIL_MAX: (u32, u32) = (300, 200);

const PERMISSION_GRANTED: &str = "Screen recording permission granted";
const PERMISSION_REQUIRED: &str = "Screen recording permission required. Please enable in System Preferences > Security & Privacy > Privacy > Screen Recording.";

/// `ScreenCaptureOptions` (`screen:capture` params).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_target: Option<String>,
}

/// `ScreenCaptureResult.metadata`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub file_size: u64,
    pub timestamp: String,
}

/// `ScreenCaptureResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureResult {
    pub success: bool,
    pub file_path: String,
    pub metadata: ScreenCaptureMetadata,
}

/// `PermissionCheckResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCheckResult {
    pub has_permission: bool,
    pub platform: String,
    pub message: String,
}

/// `WindowInfo.dimensions`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowDimensions {
    pub width: u32,
    pub height: u32,
}

/// `WindowInfo` (`screen:list-available-windows`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// `data:image/png;base64,...`
    pub thumbnail: String,
    /// The thumbnail's size (as the Electron handler reported it).
    pub dimensions: WindowDimensions,
}

/// Screen or window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Screen,
    Window,
}

/// A capturable source (`DesktopCapturerSource` without the thumbnail).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    pub kind: SourceKind,
    /// `screen:<id>:0` / `window:<id>:0`, the Electron source id format.
    pub id: String,
    /// Window title (or the app name when the title is empty), or the display name.
    pub name: String,
    /// Owning application, for windows (matched by `windowTarget` too).
    pub app_name: String,
}

/// The platform capture API.
pub trait ScreenSource: Send + Sync {
    /// Whether the OS lets this app capture the screen (macOS Screen Recording).
    fn has_permission(&self) -> bool;
    /// Displays, primary first.
    fn screens(&self) -> std::result::Result<Vec<SourceInfo>, String>;
    /// Capturable windows.
    fn windows(&self) -> std::result::Result<Vec<SourceInfo>, String>;
    /// Full-resolution capture of one source.
    fn capture(&self, source: &SourceInfo) -> std::result::Result<RgbaImage, String>;
}

/// The `screen:*` handlers over a [`ScreenSource`]. Blocking: call from `spawn_blocking`.
#[derive(Clone)]
pub struct ScreenService {
    source: Arc<dyn ScreenSource>,
    /// `os.tmpdir()`.
    temp_dir: PathBuf,
}

impl ScreenService {
    pub fn new(source: Arc<dyn ScreenSource>, temp_dir: PathBuf) -> Self {
        Self { source, temp_dir }
    }

    /// The `xcap` source writing to the system temp dir.
    pub fn native() -> Self {
        Self::new(Arc::new(super::XcapSource), std::env::temp_dir())
    }

    /// `screen:check-permissions`.
    pub fn check_permissions(&self) -> PermissionCheckResult {
        let platform = js::platform();
        if platform == "darwin" {
            let has_permission = self.source.has_permission();
            tracing::info!(has_permission, "macOS screen permission check");
            return PermissionCheckResult {
                has_permission,
                platform: platform.to_string(),
                message: if has_permission {
                    PERMISSION_GRANTED
                } else {
                    PERMISSION_REQUIRED
                }
                .to_string(),
            };
        }
        PermissionCheckResult {
            has_permission: true,
            platform: platform.to_string(),
            message: "No special permissions required".to_string(),
        }
    }

    /// `screen:list-available-windows`: never fails (errors → `[]`).
    pub fn list_available_windows(&self) -> Vec<WindowInfo> {
        let windows = match self.source.windows() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "Failed to list available windows");
                return Vec::new();
            }
        };
        let infos: Vec<WindowInfo> = windows
            .into_iter()
            .map(|w| {
                // A window that can't be captured still gets listed, with an empty thumbnail
                // (Electron's thumbnail was then an empty image).
                let thumb = self
                    .source
                    .capture(&w)
                    .map(|img| fit_within(img, THUMBNAIL_MAX))
                    .unwrap_or_else(|e| {
                        tracing::debug!(window = %w.name, error = %e, "Window thumbnail failed");
                        RgbaImage::new(0, 0)
                    });
                let dimensions = WindowDimensions {
                    width: thumb.width(),
                    height: thumb.height(),
                };
                let png = if thumb.width() == 0 {
                    Vec::new()
                } else {
                    encode(&thumb, "png", None).unwrap_or_default()
                };
                WindowInfo {
                    id: w.id,
                    name: w.name,
                    enabled: false,
                    thumbnail: format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(png)
                    ),
                    dimensions,
                }
            })
            .collect();
        tracing::info!(
            count = infos.len(),
            "Found available windows with thumbnails"
        );
        infos
    }

    /// `screen:capture`. Errors are the handler's thrown messages.
    pub fn capture(
        &self,
        options: &ScreenCaptureOptions,
    ) -> std::result::Result<ScreenCaptureResult, String> {
        let format = options
            .format
            .clone()
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| "png".to_string());
        tracing::info!(
            format = %format,
            quality = ?options.quality,
            has_window_target = options.window_target.as_deref().is_some_and(|t| !t.is_empty()),
            "Starting screen capture"
        );
        let result = self.capture_inner(options, &format);
        if let Err(e) = &result {
            tracing::error!(error = %e, "Failed to capture screen");
        }
        result
    }

    fn capture_inner(
        &self,
        options: &ScreenCaptureOptions,
        format: &str,
    ) -> std::result::Result<ScreenCaptureResult, String> {
        let source = match options.window_target.as_deref().filter(|t| !t.is_empty()) {
            Some(target) => {
                let windows = self.source.windows()?;
                if windows.is_empty() {
                    return Err("No window sources available".to_string());
                }
                let needle = target.to_lowercase();
                let found = windows.iter().find(|w| {
                    w.name.to_lowercase().contains(&needle)
                        || w.app_name.to_lowercase().contains(&needle)
                });
                match found {
                    Some(w) => {
                        tracing::info!(window_name = %w.name, "Capturing specific window");
                        w.clone()
                    }
                    None => {
                        let available = windows
                            .iter()
                            .map(|w| w.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(format!(
                            "Target window not found. Available windows: {available}"
                        ));
                    }
                }
            }
            None => self
                .source
                .screens()?
                .into_iter()
                .next()
                .ok_or_else(|| "No screen sources available".to_string())?,
        };

        let image = fit_within(self.source.capture(&source)?, CAPTURE_MAX);
        let output_path = options
            .output_path
            .clone()
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.temp_dir
                    .join(format!("screenshot_{}.{format}", js::now_millis()))
            });
        let quality = if format == "jpeg" {
            Some(jpeg_quality(options.quality))
        } else {
            None
        };
        let bytes = encode(
            &image,
            if format == "jpeg" { "jpeg" } else { "png" },
            quality,
        )?;
        std::fs::write(&output_path, bytes).map_err(|e| e.to_string())?;
        let file_size = std::fs::metadata(&output_path)
            .map_err(|e| e.to_string())?
            .len();
        let file_path = output_path.to_string_lossy().into_owned();
        tracing::info!(
            path = %file_name_for_log(&file_path),
            width = image.width(),
            height = image.height(),
            format,
            file_size,
            "Screenshot captured successfully"
        );
        Ok(ScreenCaptureResult {
            success: true,
            file_path,
            metadata: ScreenCaptureMetadata {
                width: image.width(),
                height: image.height(),
                format: format.to_string(),
                file_size,
                timestamp: js::iso_now(),
            },
        })
    }
}

/// `options.quality || 80`, clamped to what `toJPEG` accepts.
fn jpeg_quality(q: Option<f64>) -> u8 {
    match q {
        Some(q) if q != 0.0 && !q.is_nan() => q.clamp(1.0, 100.0) as u8,
        _ => 80,
    }
}

/// Scale down (keeping the aspect ratio) to fit within `max`; smaller images are unchanged.
pub(crate) fn fit_within(img: RgbaImage, max: (u32, u32)) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= max.0 && h <= max.1 {
        return img;
    }
    DynamicImage::ImageRgba8(img)
        .resize(max.0, max.1, image::imageops::FilterType::Triangle)
        .into_rgba8()
}

/// PNG, or JPEG at `quality` (alpha dropped).
fn encode(
    img: &RgbaImage,
    format: &str,
    quality: Option<u8>,
) -> std::result::Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    if format == "jpeg" {
        let rgb = DynamicImage::ImageRgba8(img.clone()).into_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.unwrap_or(80))
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
    } else {
        img.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

/// The screenCapture tool.
pub struct ScreenCaptureTool {
    screen: Arc<ScreenService>,
    bedrock: Arc<dyn BedrockBackend>,
}

impl ScreenCaptureTool {
    pub fn new(screen: Arc<ScreenService>, bedrock: Arc<dyn BedrockBackend>) -> Self {
        Self { screen, bedrock }
    }

    async fn run(&self, input: &Value) -> std::result::Result<ScreenCaptureResult, String> {
        let screen = self.screen.clone();
        let window_target = input
            .get("windowTarget")
            .and_then(Value::as_str)
            .map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let permission = screen.check_permissions();
            if !permission.has_permission {
                return Err(format!(
                    "Screen capture permission required: {}",
                    permission.message
                ));
            }
            screen.capture(&ScreenCaptureOptions {
                format: Some("png".to_string()),
                window_target,
                ..Default::default()
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[async_trait]
impl Tool for ScreenCaptureTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "recognizePrompt": {
                        "type": "string",
                        "description": "Optional prompt for image recognition analysis. If provided, the captured image will be automatically analyzed with AI using the configured model."
                    },
                    "windowTarget": {
                        "type": "string",
                        "description": "Optional target window by name or application (partial match supported). Examples: \"Chrome\", \"Terminal\", \"Visual Studio Code\""
                    }
                }
            }),
        ))
    }

    fn validate_input(&self, _input: &Value) -> Vec<String> {
        Vec::new()
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let prompt = input
            .get("recognizePrompt")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .map(str::to_string);
        tracing::info!(
            format = "png",
            will_analyze = prompt.is_some(),
            "Starting screen capture"
        );

        let capture = match self.run(&input).await {
            Ok(c) if c.success => c,
            Ok(_) => {
                return Err(capture_error(
                    NAME,
                    "Screen capture failed",
                    "Screen capture failed",
                ))
            }
            Err(e) => {
                tracing::error!(error = %e, "Screen capture failed");
                return Err(capture_error(NAME, "Screen capture failed", &e));
            }
        };
        let m = &capture.metadata;
        let mut result = json!({
            "success": true,
            "name": NAME,
            "message": format!("Screen captured successfully: {}x{} ({})", m.width, m.height, m.format),
            "result": {
                "filePath": capture.file_path,
                "metadata": capture.metadata
            }
        });
        if let Some(prompt) = prompt {
            recognize_into(
                self.bedrock.as_ref(),
                ctx,
                NAME,
                &capture.file_path,
                &prompt,
                &mut result,
            )
            .await;
        }
        Ok(ToolOutput::Json(result))
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::sync::Mutex;

    /// A source and the size of its (solid-colour) image.
    pub type SizedSource = (SourceInfo, (u32, u32));

    /// Solid-colour sources; records captured ids.
    pub struct FakeScreen {
        pub permission: bool,
        pub screens: Vec<SizedSource>,
        pub windows: std::result::Result<Vec<SizedSource>, String>,
        pub captured: Mutex<Vec<String>>,
    }

    pub fn window(id: u32, name: &str, app: &str) -> SourceInfo {
        SourceInfo {
            kind: SourceKind::Window,
            id: format!("window:{id}:0"),
            name: name.to_string(),
            app_name: app.to_string(),
        }
    }

    impl Default for FakeScreen {
        fn default() -> Self {
            FakeScreen {
                permission: true,
                screens: vec![(
                    SourceInfo {
                        kind: SourceKind::Screen,
                        id: "screen:1:0".into(),
                        name: "Built-in Display".into(),
                        app_name: String::new(),
                    },
                    (3024, 1964),
                )],
                windows: Ok(vec![
                    (
                        window(7, "README.md — Visual Studio Code", "Code"),
                        (1200, 800),
                    ),
                    (
                        window(9, "Docs - Google Chrome", "Google Chrome"),
                        (640, 480),
                    ),
                ]),
                captured: Mutex::new(Vec::new()),
            }
        }
    }

    impl ScreenSource for FakeScreen {
        fn has_permission(&self) -> bool {
            self.permission
        }
        fn screens(&self) -> std::result::Result<Vec<SourceInfo>, String> {
            Ok(self.screens.iter().map(|(s, _)| s.clone()).collect())
        }
        fn windows(&self) -> std::result::Result<Vec<SourceInfo>, String> {
            self.windows
                .clone()
                .map(|w| w.into_iter().map(|(s, _)| s).collect())
        }
        fn capture(&self, source: &SourceInfo) -> std::result::Result<RgbaImage, String> {
            self.captured.lock().unwrap().push(source.id.clone());
            let all = self
                .screens
                .iter()
                .cloned()
                .chain(self.windows.clone().unwrap_or_default());
            let (_, (w, h)) = all
                .into_iter()
                .find(|(s, _)| s.id == source.id)
                .ok_or("unknown source")?;
            Ok(RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255])))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use crate::base::run_tool;
    use crate::bedrock_tools::fake::FakeBedrock;
    use crate::context::ToolSettings;

    fn service(fake: FakeScreen, dir: &std::path::Path) -> (Arc<FakeScreen>, ScreenService) {
        let fake = Arc::new(fake);
        (fake.clone(), ScreenService::new(fake, dir.to_path_buf()))
    }

    #[test]
    fn options_and_results_use_the_ipc_shapes() {
        let o: ScreenCaptureOptions =
            serde_json::from_value(json!({"format": "jpeg", "quality": 60, "outputPath": "/x.jpg", "windowTarget": "Chrome"}))
                .unwrap();
        assert_eq!(o.format.as_deref(), Some("jpeg"));
        assert_eq!(o.quality, Some(60.0));
        assert_eq!(o.output_path.as_deref(), Some("/x.jpg"));
        assert_eq!(o.window_target.as_deref(), Some("Chrome"));
        assert_eq!(
            serde_json::from_value::<ScreenCaptureOptions>(json!({})).unwrap(),
            ScreenCaptureOptions::default()
        );

        let r = ScreenCaptureResult {
            success: true,
            file_path: "/tmp/s.png".into(),
            metadata: ScreenCaptureMetadata {
                width: 1,
                height: 2,
                format: "png".into(),
                file_size: 3,
                timestamp: "t".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({"success": true, "filePath": "/tmp/s.png", "metadata": {"width": 1, "height": 2, "format": "png", "fileSize": 3, "timestamp": "t"}})
        );
        let p = PermissionCheckResult {
            has_permission: true,
            platform: "linux".into(),
            message: "m".into(),
        };
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({"hasPermission": true, "platform": "linux", "message": "m"})
        );
    }

    #[test]
    fn captures_the_primary_screen_scaled_to_fit_as_png() {
        let dir = tempfile::tempdir().unwrap();
        let (fake, svc) = service(FakeScreen::default(), dir.path());
        let r = svc.capture(&ScreenCaptureOptions::default()).unwrap();
        assert!(r.success);
        assert_eq!(*fake.captured.lock().unwrap(), vec!["screen:1:0"]);
        // 3024x1964 fit within 1920x1080 keeps the aspect ratio.
        assert_eq!((r.metadata.width, r.metadata.height), (1663, 1080));
        assert_eq!(r.metadata.format, "png");
        let name = std::path::Path::new(&r.file_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with("screenshot_") && name.ends_with(".png"),
            "{name}"
        );
        assert!(r.file_path.starts_with(&*dir.path().to_string_lossy()));
        let bytes = std::fs::read(&r.file_path).unwrap();
        assert_eq!(r.metadata.file_size, bytes.len() as u64);
        assert_eq!(&bytes[..4], b"\x89PNG");
        assert!(r.metadata.timestamp.ends_with('Z'));
    }

    #[test]
    fn jpeg_to_an_explicit_path() {
        let dir = tempfile::tempdir().unwrap();
        let (_, svc) = service(FakeScreen::default(), dir.path());
        let out = dir.path().join("shot.jpeg");
        let r = svc
            .capture(&ScreenCaptureOptions {
                format: Some("jpeg".into()),
                quality: Some(50.0),
                output_path: Some(out.to_string_lossy().into_owned()),
                window_target: Some("chrome".into()),
            })
            .unwrap();
        assert_eq!(r.file_path, out.to_string_lossy());
        assert_eq!((r.metadata.width, r.metadata.height), (640, 480));
        assert_eq!(r.metadata.format, "jpeg");
        assert_eq!(&std::fs::read(&out).unwrap()[..2], b"\xFF\xD8");
        assert_eq!(jpeg_quality(None), 80);
        assert_eq!(jpeg_quality(Some(0.0)), 80);
        assert_eq!(jpeg_quality(Some(500.0)), 100);
    }

    #[test]
    fn window_target_matches_title_or_app_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let (fake, svc) = service(FakeScreen::default(), dir.path());
        svc.capture(&ScreenCaptureOptions {
            window_target: Some("visual studio".into()),
            ..Default::default()
        })
        .unwrap();
        svc.capture(&ScreenCaptureOptions {
            window_target: Some("GOOGLE CHROME".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            *fake.captured.lock().unwrap(),
            vec!["window:7:0", "window:9:0"]
        );

        let err = svc
            .capture(&ScreenCaptureOptions {
                window_target: Some("Terminal".into()),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(
            err,
            "Target window not found. Available windows: README.md — Visual Studio Code, Docs - Google Chrome"
        );
    }

    #[test]
    fn no_sources_errors() {
        let dir = tempfile::tempdir().unwrap();
        let (_, svc) = service(
            FakeScreen {
                screens: vec![],
                windows: Ok(vec![]),
                ..Default::default()
            },
            dir.path(),
        );
        assert_eq!(
            svc.capture(&ScreenCaptureOptions::default()).unwrap_err(),
            "No screen sources available"
        );
        assert_eq!(
            svc.capture(&ScreenCaptureOptions {
                window_target: Some("x".into()),
                ..Default::default()
            })
            .unwrap_err(),
            "No window sources available"
        );
    }

    #[test]
    fn window_list_has_thumbnails_and_never_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (_, svc) = service(FakeScreen::default(), dir.path());
        let list = svc.list_available_windows();
        assert_eq!(list.len(), 2);
        let v = serde_json::to_value(&list[0]).unwrap();
        assert_eq!(v["id"], "window:7:0");
        assert_eq!(v["name"], "README.md — Visual Studio Code");
        assert_eq!(v["enabled"], false);
        assert_eq!(v["dimensions"], json!({"width": 300, "height": 200}));
        assert!(v["thumbnail"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR"));
        assert_eq!(
            list[1].dimensions,
            WindowDimensions {
                width: 267,
                height: 200
            }
        );

        let (_, broken) = service(
            FakeScreen {
                windows: Err("boom".into()),
                ..Default::default()
            },
            dir.path(),
        );
        assert!(broken.list_available_windows().is_empty());
    }

    #[test]
    fn permission_result_per_platform() {
        let dir = tempfile::tempdir().unwrap();
        let (_, denied) = service(
            FakeScreen {
                permission: false,
                ..Default::default()
            },
            dir.path(),
        );
        let r = denied.check_permissions();
        assert_eq!(r.platform, js::platform());
        if js::platform() == "darwin" {
            assert!(!r.has_permission);
            assert_eq!(r.message, PERMISSION_REQUIRED);
        } else {
            assert!(r.has_permission);
            assert_eq!(r.message, "No special permissions required");
        }
    }

    fn tool(fake: FakeScreen, dir: &std::path::Path) -> (ScreenCaptureTool, Arc<FakeBedrock>) {
        let bedrock = Arc::new(FakeBedrock::default());
        let (_, svc) = service(fake, dir);
        (
            ScreenCaptureTool::new(Arc::new(svc), bedrock.clone()),
            bedrock,
        )
    }

    #[tokio::test]
    async fn tool_result_shape() {
        let dir = tempfile::tempdir().unwrap();
        let (t, bedrock) = tool(FakeScreen::default(), dir.path());
        let out = run_tool(&t, json!({"type": NAME}), &ToolContext::default())
            .await
            .unwrap()
            .into_value();
        assert_eq!(out["success"], true);
        assert_eq!(out["name"], NAME);
        assert_eq!(
            out["message"],
            "Screen captured successfully: 1663x1080 (png)"
        );
        assert_eq!(out["result"]["metadata"]["format"], "png");
        assert!(out["result"]["filePath"]
            .as_str()
            .unwrap()
            .ends_with(".png"));
        assert!(out["result"].get("recognition").is_none());
        assert!(bedrock.calls().is_empty());
    }

    #[tokio::test]
    async fn tool_recognition_success_and_failure() {
        let dir = tempfile::tempdir().unwrap();
        let (t, bedrock) = tool(FakeScreen::default(), dir.path());
        let long = "x".repeat(120);
        let reply = long.clone();
        *bedrock.recognize.lock().unwrap() = Some(Arc::new(move |_| Ok(reply.clone())));
        let ctx = ToolContext {
            settings: ToolSettings {
                recognize_image_model_id: Some("model-x".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let out = run_tool(
            &t,
            json!({"type": NAME, "recognizePrompt": "what?", "windowTarget": "code"}),
            &ctx,
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            out["message"],
            format!(
                "Screen captured successfully: 1200x800 (png) Image recognition completed: {}...",
                "x".repeat(100)
            )
        );
        assert_eq!(
            out["result"]["recognition"],
            json!({"content": long, "modelId": "model-x", "prompt": "what?"})
        );
        let (_, call) = &bedrock.calls()[0];
        assert_eq!(call["prompt"], "what?");
        assert_eq!(call["modelId"], "model-x");
        assert_eq!(call["imagePath"], out["result"]["filePath"]);

        *bedrock.recognize.lock().unwrap() = Some(Arc::new(|_| Err("denied".into())));
        let out = run_tool(
            &t,
            json!({"type": NAME, "recognizePrompt": "p"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            out["message"],
            "Screen captured successfully: 1663x1080 (png) (Note: Image recognition failed)"
        );
        assert!(out["result"].get("recognition").is_none());
        assert_eq!(
            bedrock.calls()[1].1["modelId"],
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
    }

    #[tokio::test]
    async fn tool_errors_are_wrapped_json() {
        let dir = tempfile::tempdir().unwrap();
        let (t, _) = tool(FakeScreen::default(), dir.path());
        let err = run_tool(
            &t,
            json!({"type": NAME, "windowTarget": "nope"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let resp: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(resp["success"], false);
        assert_eq!(resp["type"], "EXECUTION");
        assert_eq!(resp["toolName"], NAME);
        let inner: Value = serde_json::from_str(resp["error"].as_str().unwrap()).unwrap();
        assert_eq!(inner["name"], NAME);
        assert_eq!(inner["error"], "Screen capture failed");
        assert!(inner["message"]
            .as_str()
            .unwrap()
            .starts_with("Target window not found."));

        if js::platform() == "darwin" {
            let (t, _) = tool(
                FakeScreen {
                    permission: false,
                    ..Default::default()
                },
                dir.path(),
            );
            let err = run_tool(&t, json!({"type": NAME}), &ToolContext::default())
                .await
                .unwrap_err();
            let resp: Value = serde_json::from_str(&err.message).unwrap();
            let inner: Value = serde_json::from_str(resp["error"].as_str().unwrap()).unwrap();
            assert_eq!(
                inner["message"],
                format!("Screen capture permission required: {PERMISSION_REQUIRED}")
            );
        }
    }

    #[test]
    fn spec() {
        let (t, _) = tool(FakeScreen::default(), std::path::Path::new("/tmp"));
        let s = t.spec().unwrap().to_bedrock_tool();
        assert_eq!(s["toolSpec"]["name"], NAME);
        assert!(s["toolSpec"]["description"]
            .as_str()
            .unwrap()
            .ends_with("Available windows for screen capture:{{allowedWindows}}"));
        assert_eq!(
            s["toolSpec"]["inputSchema"]["json"]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["recognizePrompt", "windowTarget"]
        );
        assert_eq!(t.category(), ToolCategory::System);
    }
}
