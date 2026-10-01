//! Port of `CameraCaptureTool.ts` and the `camera:save-captured-image` handler
//! (`src/main/handlers/camera-handlers.ts`).
//!
//! The TS tool grabbed a frame with `getUserMedia` in the renderer and sent it to main to be
//! saved. Here the tool runs in Rust, so the frame comes from a [`CameraSource`] (the app asks
//! the main webview for it); saving is [`save_captured_image`], the handler.

use super::{capture_error, file_name_for_log, recognize_into};
use crate::base::Tool;
use crate::bedrock_tools::BedrockBackend;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::{self, truthy};
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const NAME: &str = "cameraCapture";
const DESCRIPTION: &str = "Capture images from PC camera using HTML5 getUserMedia API and save as an image file. Optionally analyze the captured image with AI to extract text content, identify objects, and provide detailed visual descriptions for analysis and documentation purposes.\n\nCapture images from camera using HTML5 getUserMedia API for AI analysis. Useful for real-time visual input, object recognition, and document scanning. Available cameras: {{allowedCameras}}";

const QUALITIES: [&str; 3] = ["low", "medium", "high"];
const FORMATS: [&str; 2] = ["jpg", "png"];

/// `CameraCaptureRequest` (`camera:save-captured-image` params).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraCaptureRequest {
    /// `data:image/jpeg;base64,...` (the prefix is optional).
    pub base64_data: String,
    pub device_id: String,
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

/// `CameraCaptureResult.metadata`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraCaptureMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub file_size: u64,
    pub timestamp: String,
    pub device_id: String,
    pub device_name: String,
}

/// `CameraCaptureResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraCaptureResult {
    pub success: bool,
    pub file_path: String,
    pub metadata: CameraCaptureMetadata,
}

/// What the tool asks the webview to capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFrameRequest {
    /// `undefined` / `'default'` → the default camera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// `low` | `medium` | `high` (640x480 / 1280x720 / 1920x1080 ideal).
    pub quality: String,
    /// `jpg` | `png`.
    pub format: String,
}

/// A frame grabbed by the webview: `captureImage()` plus the device it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedFrame {
    pub base64_data: String,
    pub width: u32,
    pub height: u32,
    /// `deviceInfo?.deviceId || 'default'`.
    pub device_id: String,
    /// `deviceInfo?.label || 'Unknown Camera'`.
    pub device_name: String,
}

/// Grabs one camera frame (`initializeCamera` + `captureImage` + `cleanup`).
#[async_trait]
pub trait CameraSource: Send + Sync {
    /// Errors are the TS error messages (`Failed to access camera: ...`, `Camera capture
    /// timeout`, ...).
    async fn capture_frame(
        &self,
        req: &CameraFrameRequest,
    ) -> std::result::Result<CapturedFrame, String>;
}

/// A [`CameraSource`] for contexts without a webview.
pub struct NoCameraSource;

#[async_trait]
impl CameraSource for NoCameraSource {
    async fn capture_frame(
        &self,
        _req: &CameraFrameRequest,
    ) -> std::result::Result<CapturedFrame, String> {
        Err("getUserMedia API is not available in this browser".to_string())
    }
}

/// `camera:save-captured-image`: decode the data URL and write it to `outputPath` or
/// `<temp_dir>/camera_capture_<ms>.<format>`.
pub fn save_captured_image(
    request: &CameraCaptureRequest,
    temp_dir: &Path,
) -> std::result::Result<CameraCaptureResult, String> {
    tracing::info!(
        device_id = %request.device_id,
        device_name = %request.device_name,
        width = request.width,
        height = request.height,
        format = %request.format,
        "Saving captured camera image"
    );
    let result = save_inner(request, temp_dir);
    if let Err(e) = &result {
        tracing::error!(error = %e, "Failed to save captured camera image");
    }
    result
}

fn save_inner(
    request: &CameraCaptureRequest,
    temp_dir: &Path,
) -> std::result::Result<CameraCaptureResult, String> {
    let bytes = decode_data_url(&request.base64_data);
    let output_path = request
        .output_path
        .clone()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            temp_dir.join(format!(
                "camera_capture_{}.{}",
                js::now_millis(),
                request.format
            ))
        });
    std::fs::write(&output_path, bytes).map_err(|e| e.to_string())?;
    let file_size = std::fs::metadata(&output_path)
        .map_err(|e| e.to_string())?
        .len();
    let file_path = output_path.to_string_lossy().into_owned();
    tracing::info!(
        path = %file_name_for_log(&file_path),
        file_size,
        device_name = %request.device_name,
        "Camera capture saved successfully"
    );
    Ok(CameraCaptureResult {
        success: true,
        file_path,
        metadata: CameraCaptureMetadata {
            width: request.width,
            height: request.height,
            format: request.format.clone(),
            file_size,
            timestamp: js::iso_now(),
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
        },
    })
}

/// `Buffer.from(data.replace(/^data:image\/[a-z]+;base64,/, ''), 'base64')`: Node skips
/// characters outside the base64 alphabet and tolerates missing padding.
fn decode_data_url(data: &str) -> Vec<u8> {
    let body = match data.strip_prefix("data:image/") {
        Some(rest) => {
            let kind_len = rest.bytes().take_while(u8::is_ascii_lowercase).count();
            match rest[kind_len..].strip_prefix(";base64,") {
                Some(b) if kind_len > 0 => b,
                _ => data,
            }
        }
        None => data,
    };
    let mut clean: String = body
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/')
        .collect();
    // A dangling single character carries no full byte.
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_allow_trailing_bits(true)
            .with_decode_padding_mode(DecodePaddingMode::RequireNone),
    );
    LENIENT.decode(clean.as_bytes()).unwrap_or_default()
}

/// The cameraCapture tool.
pub struct CameraCaptureTool {
    camera: Arc<dyn CameraSource>,
    bedrock: Arc<dyn BedrockBackend>,
    temp_dir: PathBuf,
}

impl CameraCaptureTool {
    pub fn new(camera: Arc<dyn CameraSource>, bedrock: Arc<dyn BedrockBackend>) -> Self {
        Self::with_temp_dir(camera, bedrock, std::env::temp_dir())
    }

    pub fn with_temp_dir(
        camera: Arc<dyn CameraSource>,
        bedrock: Arc<dyn BedrockBackend>,
        temp_dir: PathBuf,
    ) -> Self {
        Self {
            camera,
            bedrock,
            temp_dir,
        }
    }

    async fn capture(
        &self,
        req: &CameraFrameRequest,
    ) -> std::result::Result<CameraCaptureResult, String> {
        let frame = self.camera.capture_frame(req).await?;
        tracing::info!(
            width = frame.width,
            height = frame.height,
            format = %req.format,
            data_size = frame.base64_data.len(),
            "Image captured from camera"
        );
        let request = CameraCaptureRequest {
            base64_data: frame.base64_data,
            device_id: frame.device_id,
            device_name: frame.device_name,
            width: frame.width,
            height: frame.height,
            format: req.format.clone(),
            output_path: None,
        };
        let temp_dir = self.temp_dir.clone();
        tokio::task::spawn_blocking(move || save_captured_image(&request, &temp_dir))
            .await
            .map_err(|e| e.to_string())?
    }
}

/// `input[key] || default` for a string option.
fn string_or<'a>(input: &'a Value, key: &str, default: &'a str) -> &'a str {
    match input.get(key) {
        Some(Value::String(s)) if !s.is_empty() => s,
        _ => default,
    }
}

#[async_trait]
impl Tool for CameraCaptureTool {
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
                    "deviceId": {
                        "type": "string",
                        "description": "Optional camera device ID to use for capture. If not provided, the default camera will be used."
                    },
                    "recognizePrompt": {
                        "type": "string",
                        "description": "Optional prompt for image recognition analysis. If provided, the captured image will be automatically analyzed with AI using the configured model."
                    },
                    "quality": {
                        "type": "string",
                        "enum": ["low", "medium", "high"],
                        "description": "Image quality setting. Low: 640x480, Medium: 1280x720, High: 1920x1080. Default is medium."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["jpg", "png"],
                        "description": "Output image format. Default is jpg."
                    }
                }
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let one_of = |key: &str, allowed: &[&str]| {
            let v = input.get(key);
            truthy(v)
                && !v
                    .and_then(Value::as_str)
                    .is_some_and(|s| allowed.contains(&s))
        };
        if one_of("quality", &QUALITIES) {
            errors.push("Quality must be one of: low, medium, high".to_string());
        }
        if one_of("format", &FORMATS) {
            errors.push("Format must be one of: jpg, png".to_string());
        }
        let non_string = |key: &str| {
            let v = input.get(key);
            truthy(v) && !v.is_some_and(Value::is_string)
        };
        if non_string("deviceId") {
            errors.push("Device ID must be a string".to_string());
        }
        if non_string("recognizePrompt") {
            errors.push("Recognition prompt must be a string".to_string());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let prompt = input
            .get("recognizePrompt")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .map(str::to_string);
        let req = CameraFrameRequest {
            device_id: input
                .get("deviceId")
                .and_then(Value::as_str)
                .filter(|d| !d.is_empty())
                .map(str::to_string),
            quality: string_or(&input, "quality", "medium").to_string(),
            format: string_or(&input, "format", "jpg").to_string(),
        };
        tracing::info!(
            device_id = req.device_id.as_deref().unwrap_or("default"),
            quality = %req.quality,
            format = %req.format,
            will_analyze = prompt.is_some(),
            "Starting camera capture with getUserMedia API"
        );

        let saved = match self.capture(&req).await {
            Ok(r) if r.success => r,
            Ok(_) => {
                return Err(capture_error(
                    NAME,
                    "Camera capture failed",
                    "Failed to save captured image",
                ))
            }
            Err(e) => {
                tracing::error!(error = %e, "Camera capture failed");
                return Err(capture_error(NAME, "Camera capture failed", &e));
            }
        };
        let m = &saved.metadata;
        let mut result = json!({
            "success": true,
            "name": NAME,
            "message": format!(
                "Camera capture successful: {}x{} ({}) from {}",
                m.width, m.height, m.format, m.device_name
            ),
            "result": {
                "filePath": saved.file_path,
                "metadata": saved.metadata
            }
        });
        if let Some(prompt) = prompt {
            recognize_into(
                self.bedrock.as_ref(),
                ctx,
                NAME,
                &saved.file_path,
                &prompt,
                &mut result,
            )
            .await;
        }
        Ok(ToolOutput::Json(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use crate::bedrock_tools::fake::FakeBedrock;
    use std::sync::Mutex;

    /// 1x1 PNG.
    const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    #[derive(Default)]
    struct FakeCamera {
        requests: Mutex<Vec<CameraFrameRequest>>,
        fail: Option<String>,
    }

    #[async_trait]
    impl CameraSource for FakeCamera {
        async fn capture_frame(
            &self,
            req: &CameraFrameRequest,
        ) -> std::result::Result<CapturedFrame, String> {
            self.requests.lock().unwrap().push(req.clone());
            if let Some(e) = &self.fail {
                return Err(e.clone());
            }
            Ok(CapturedFrame {
                base64_data: format!("data:image/png;base64,{PNG_B64}"),
                width: 1280,
                height: 720,
                device_id: "cam-1".into(),
                device_name: "FaceTime HD Camera".into(),
            })
        }
    }

    fn tool(
        camera: FakeCamera,
        dir: &Path,
    ) -> (CameraCaptureTool, Arc<FakeCamera>, Arc<FakeBedrock>) {
        let camera = Arc::new(camera);
        let bedrock = Arc::new(FakeBedrock::default());
        (
            CameraCaptureTool::with_temp_dir(camera.clone(), bedrock.clone(), dir.to_path_buf()),
            camera,
            bedrock,
        )
    }

    #[test]
    fn request_and_result_use_the_ipc_shapes() {
        let r: CameraCaptureRequest = serde_json::from_value(json!({
            "base64Data": "data:image/jpeg;base64,AAAA",
            "deviceId": "d",
            "deviceName": "n",
            "width": 640,
            "height": 480,
            "format": "jpg"
        }))
        .unwrap();
        assert_eq!(r.output_path, None);
        assert_eq!(r.width, 640);
        let res = CameraCaptureResult {
            success: true,
            file_path: "/f".into(),
            metadata: CameraCaptureMetadata {
                width: 1,
                height: 2,
                format: "jpg".into(),
                file_size: 3,
                timestamp: "t".into(),
                device_id: "d".into(),
                device_name: "n".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&res).unwrap(),
            json!({"success": true, "filePath": "/f", "metadata": {"width": 1, "height": 2, "format": "jpg", "fileSize": 3, "timestamp": "t", "deviceId": "d", "deviceName": "n"}})
        );
        assert_eq!(
            serde_json::to_value(CameraFrameRequest {
                device_id: None,
                quality: "medium".into(),
                format: "jpg".into()
            })
            .unwrap(),
            json!({"quality": "medium", "format": "jpg"})
        );
    }

    #[test]
    fn data_url_decoding() {
        assert_eq!(decode_data_url("data:image/jpeg;base64,aGk="), b"hi");
        assert_eq!(decode_data_url("aGk="), b"hi");
        assert_eq!(decode_data_url("aGk"), b"hi");
        assert_eq!(decode_data_url("aG\nk=!"), b"hi");
        // Only the image/<lowercase> prefix is stripped.
        assert_ne!(decode_data_url("data:text/plain;base64,aGk="), b"hi");
        // Trailing bits are ignored, as Node does.
        assert_eq!(decode_data_url("aGl="), b"hi");
        assert!(decode_data_url("").is_empty());
    }

    #[test]
    fn saves_to_temp_or_output_path() {
        let dir = tempfile::tempdir().unwrap();
        let req = CameraCaptureRequest {
            base64_data: format!("data:image/png;base64,{PNG_B64}"),
            device_id: "d".into(),
            device_name: "Cam".into(),
            width: 1,
            height: 1,
            format: "png".into(),
            output_path: None,
        };
        let r = save_captured_image(&req, dir.path()).unwrap();
        let name = Path::new(&r.file_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with("camera_capture_") && name.ends_with(".png"),
            "{name}"
        );
        assert_eq!(&std::fs::read(&r.file_path).unwrap()[..4], b"\x89PNG");
        assert_eq!(
            r.metadata.file_size,
            std::fs::metadata(&r.file_path).unwrap().len()
        );
        assert_eq!(r.metadata.device_name, "Cam");

        let out = dir.path().join("x.png");
        let r = save_captured_image(
            &CameraCaptureRequest {
                output_path: Some(out.to_string_lossy().into_owned()),
                ..req.clone()
            },
            dir.path(),
        )
        .unwrap();
        assert_eq!(r.file_path, out.to_string_lossy());

        let err = save_captured_image(
            &CameraCaptureRequest {
                output_path: Some(
                    dir.path()
                        .join("missing/x.png")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..req
            },
            dir.path(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn validation_messages() {
        let (t, _, _) = tool(FakeCamera::default(), Path::new("/tmp"));
        assert!(t.validate_input(&json!({})).is_empty());
        assert!(t
            .validate_input(&json!({"quality": "high", "format": "png", "deviceId": "x", "recognizePrompt": "p"}))
            .is_empty());
        assert_eq!(
            t.validate_input(&json!({"quality": "ultra", "format": "gif", "deviceId": 3, "recognizePrompt": true})),
            vec![
                "Quality must be one of: low, medium, high",
                "Format must be one of: jpg, png",
                "Device ID must be a string",
                "Recognition prompt must be a string"
            ]
        );
        // Falsy values are "not provided".
        assert!(t
            .validate_input(
                &json!({"quality": "", "format": null, "deviceId": 0, "recognizePrompt": false})
            )
            .is_empty());
    }

    #[tokio::test]
    async fn captures_with_defaults_and_saves() {
        let dir = tempfile::tempdir().unwrap();
        let (t, camera, bedrock) = tool(FakeCamera::default(), dir.path());
        let out = run_tool(&t, json!({"type": NAME}), &ToolContext::default())
            .await
            .unwrap()
            .into_value();
        assert_eq!(
            *camera.requests.lock().unwrap(),
            vec![CameraFrameRequest {
                device_id: None,
                quality: "medium".into(),
                format: "jpg".into()
            }]
        );
        assert_eq!(out["success"], true);
        assert_eq!(out["name"], NAME);
        assert_eq!(
            out["message"],
            "Camera capture successful: 1280x720 (jpg) from FaceTime HD Camera"
        );
        let meta = &out["result"]["metadata"];
        assert_eq!(meta["deviceId"], "cam-1");
        assert_eq!(meta["format"], "jpg");
        assert!(out["result"]["filePath"]
            .as_str()
            .unwrap()
            .ends_with(".jpg"));
        assert!(bedrock.calls().is_empty());
    }

    #[tokio::test]
    async fn passes_options_and_recognizes() {
        let dir = tempfile::tempdir().unwrap();
        let (t, camera, bedrock) = tool(FakeCamera::default(), dir.path());
        *bedrock.recognize.lock().unwrap() = Some(Arc::new(|_| Ok("a cat".into())));
        let out = run_tool(
            &t,
            json!({"type": NAME, "deviceId": "cam-2", "quality": "high", "format": "png", "recognizePrompt": "what?"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            camera.requests.lock().unwrap()[0].device_id.as_deref(),
            Some("cam-2")
        );
        assert_eq!(camera.requests.lock().unwrap()[0].quality, "high");
        assert_eq!(
            out["message"],
            "Camera capture successful: 1280x720 (png) from FaceTime HD Camera Image recognition completed: a cat"
        );
        assert_eq!(out["result"]["recognition"]["content"], "a cat");
        assert_eq!(out["result"]["recognition"]["prompt"], "what?");
    }

    #[tokio::test]
    async fn capture_failure_is_wrapped_json() {
        let dir = tempfile::tempdir().unwrap();
        let (t, _, _) = tool(
            FakeCamera {
                fail: Some("Failed to access camera: NotAllowedError".into()),
                ..Default::default()
            },
            dir.path(),
        );
        let err = run_tool(&t, json!({"type": NAME}), &ToolContext::default())
            .await
            .unwrap_err();
        let resp: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(resp["toolName"], NAME);
        let inner: Value = serde_json::from_str(resp["error"].as_str().unwrap()).unwrap();
        assert_eq!(
            inner,
            json!({"success": false, "name": NAME, "error": "Camera capture failed", "message": "Failed to access camera: NotAllowedError"})
        );

        let err = run_tool(
            &t,
            json!({"type": NAME, "quality": "x"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let resp: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(resp["type"], "VALIDATION");
    }
}
