//! System tools (`src/preload/tools/handlers/system`): screenCapture and cameraCapture, plus the
//! main-process handlers they called (`src/main/handlers/screen-handlers.ts`,
//! `camera-handlers.ts` `camera:save-captured-image`).
//!
//! - Screen capture goes through a [`ScreenSource`] (the platform capture API). [`XcapSource`]
//!   is the production source (the `xcap` crate); [`ScreenService`] holds the handler logic
//!   (`screen:capture`, `screen:list-available-windows`, `screen:check-permissions`), so it can
//!   be tested with a fake source.
//! - Camera frames are grabbed in a webview with `getUserMedia`, as the TS tool did in the
//!   renderer. The tool asks a [`CameraSource`] for a frame (the app implements it with a
//!   webview round trip) and saves it with [`save_captured_image`].

mod camera;
mod native;
mod screen;

pub use camera::{
    save_captured_image, CameraCaptureMetadata, CameraCaptureRequest, CameraCaptureResult,
    CameraCaptureTool, CameraFrameRequest, CameraSource, CapturedFrame, NoCameraSource,
};
pub use native::XcapSource;
pub use screen::{
    PermissionCheckResult, ScreenCaptureMetadata, ScreenCaptureOptions, ScreenCaptureResult,
    ScreenCaptureTool, ScreenService, ScreenSource, SourceInfo, SourceKind, WindowDimensions,
    WindowInfo,
};

use crate::bedrock_tools::BedrockBackend;
use crate::Tool;
use std::sync::Arc;

/// `createSystemTools()`: screenCapture, cameraCapture.
pub fn create_system_tools(
    screen: Arc<ScreenService>,
    camera: Arc<dyn CameraSource>,
    bedrock: Arc<dyn BedrockBackend>,
) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ScreenCaptureTool::new(screen, bedrock.clone())),
        Arc::new(CameraCaptureTool::new(camera, bedrock)),
    ]
}

/// The TS tools' fallback when `recognizeImageTool.modelId` is unset.
const DEFAULT_RECOGNIZE_MODEL: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";

/// The recognition step both tools share: `bedrock:recognizeImage` on the captured file with
/// the `recognizeImageTool.modelId` setting. Adds `result.recognition` and extends `message`
/// like the TS tools; a recognition failure only appends a note.
pub(crate) async fn recognize_into(
    backend: &dyn BedrockBackend,
    ctx: &crate::ToolContext,
    tool: &str,
    file_path: &str,
    prompt: &str,
    result: &mut serde_json::Value,
) {
    use crate::util::js;
    use bedrock::image_recognition::RecognizeImageRequest;
    use serde_json::json;

    tracing::info!(tool, "Starting image recognition");
    let model_id = ctx
        .settings
        .recognize_image_model_id
        .clone()
        .unwrap_or_else(|| DEFAULT_RECOGNIZE_MODEL.to_string());
    let req = RecognizeImageRequest {
        image_path: file_path.to_string(),
        prompt: Some(prompt.to_string()),
        model_id: Some(model_id.clone()),
    };
    match backend.recognize_image(&ctx.settings.converse, &req).await {
        Ok(content) if !content.is_empty() => {
            let preview = js::slice(&content, 0, 100);
            let ellipsis = if js::len(&content) > 100 { "..." } else { "" };
            let note = format!(" Image recognition completed: {preview}{ellipsis}");
            result["result"]["recognition"] = json!({
                "content": content,
                "modelId": model_id,
                "prompt": prompt
            });
            if let Some(serde_json::Value::String(m)) = result.get_mut("message") {
                m.push_str(&note);
            }
            tracing::info!(tool, model_id = %model_id, "Image recognition completed successfully");
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(tool, error = %e, "Image recognition failed, but capture succeeded");
            if let Some(serde_json::Value::String(m)) = result.get_mut("message") {
                m.push_str(" (Note: Image recognition failed)");
            }
        }
    }
}

/// `throw new Error(JSON.stringify({ success: false, name, error, message }))`; `BaseTool`
/// then wraps it like any other failure.
pub(crate) fn capture_error(tool: &str, error: &str, message: &str) -> crate::ToolError {
    crate::ToolError::plain(
        serde_json::json!({
            "success": false,
            "name": tool,
            "error": error,
            "message": message
        })
        .to_string(),
    )
}

/// File name without directories, for logs (`sanitizePath`).
pub(crate) fn file_name_for_log(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_log_path() {
        assert_eq!(file_name_for_log("/tmp/a/shot.png"), "shot.png");
        assert_eq!(file_name_for_log("C:\\x\\y.jpg"), "y.jpg");
        assert_eq!(file_name_for_log("plain"), "plain");
    }

    #[test]
    fn capture_error_is_the_ts_json() {
        let e = capture_error("screenCapture", "Screen capture failed", "boom");
        assert_eq!(
            e.message,
            r#"{"success":false,"name":"screenCapture","error":"Screen capture failed","message":"boom"}"#
        );
        assert_eq!(e.name, "Error");
    }
}

/// The user's go-ahead for `kind` for the calling agent (TOOLS-6), or the tool error to return.
/// Background and sub-agent runs never capture: nobody is watching what ends up on screen.
pub(crate) async fn confirm_capture(
    ctx: &crate::context::ToolContext,
    kind: crate::context::CaptureKind,
    tool: &str,
) -> crate::error::Result<()> {
    use crate::error::ToolError;
    if ctx.caller.unattended {
        return Err(ToolError::permission_denied(
            "Screen and camera capture aren't available in background or sub-agent runs.",
            tool,
            None,
        ));
    }
    let Some(approver) = &ctx.capture_approver else {
        return Ok(());
    };
    let agent_id = ctx.agent_id().unwrap_or_default();
    let request = crate::context::CaptureRequest {
        agent_name: ctx.find_agent(&agent_id).and_then(|a| a.name),
        agent_id,
        kind,
    };
    if approver.approve(&request).await {
        Ok(())
    } else {
        Err(ToolError::permission_denied(
            "The user declined to let this agent capture. Continue without it.",
            tool,
            None,
        ))
    }
}

#[cfg(test)]
mod capture_consent_tests {
    use super::*;
    use crate::context::{CaptureApprover, CaptureKind, CaptureRequest, ToolContext};
    use std::sync::{Arc, Mutex};

    struct Approver(bool, Mutex<Vec<CaptureRequest>>);
    impl CaptureApprover for Approver {
        fn approve<'a>(
            &'a self,
            request: &'a CaptureRequest,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
            self.1.lock().unwrap().push(request.clone());
            Box::pin(async move { self.0 })
        }
    }

    // TOOLS-6: captures need the agent's grant, and never run unattended.
    #[tokio::test]
    async fn captures_need_consent_and_never_run_unattended() {
        let mut ctx = ToolContext::default();
        ctx.caller.agent_id = Some("a".into());
        assert!(confirm_capture(&ctx, CaptureKind::Screen, "screenCapture")
            .await
            .is_ok());

        let yes = Arc::new(Approver(true, Mutex::new(Vec::new())));
        ctx.capture_approver = Some(yes.clone());
        assert!(confirm_capture(&ctx, CaptureKind::Camera, "cameraCapture")
            .await
            .is_ok());
        assert_eq!(yes.1.lock().unwrap()[0].kind, CaptureKind::Camera);
        assert_eq!(yes.1.lock().unwrap()[0].agent_id, "a");

        ctx.capture_approver = Some(Arc::new(Approver(false, Mutex::new(Vec::new()))));
        let err = confirm_capture(&ctx, CaptureKind::Screen, "screenCapture")
            .await
            .unwrap_err();
        assert!(err.message.contains("declined"), "{}", err.message);

        ctx.capture_approver = Some(yes.clone());
        ctx.caller.unattended = true;
        let err = confirm_capture(&ctx, CaptureKind::Screen, "screenCapture")
            .await
            .unwrap_err();
        assert!(err.message.contains("background"), "{}", err.message);
        assert_eq!(yes.1.lock().unwrap().len(), 1);
    }
}
