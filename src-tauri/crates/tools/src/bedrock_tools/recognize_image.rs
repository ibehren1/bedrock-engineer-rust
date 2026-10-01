//! Port of `RecognizeImageTool.ts`.

use super::{is_string, present, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use bedrock::image_recognition::RecognizeImageRequest;
use serde_json::{json, Value};
use std::sync::Arc;

const NAME: &str = "recognizeImage";
const DESCRIPTION: &str = "Analyze and describe multiple images (up to 5) using Amazon Bedrock's Claude vision capabilities. The tool processes images in parallel and returns detailed descriptions.\n\nAnalyze and describe image content. Supports multiple images simultaneously.";

/// The TS tool's fallback when `recognizeImageTool.modelId` is unset.
pub const DEFAULT_RECOGNIZE_MODEL: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";

pub struct RecognizeImageTool {
    backend: Arc<dyn BedrockBackend>,
}

impl RecognizeImageTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl Tool for RecognizeImageTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Bedrock
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "imagePaths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Paths to the image files to analyze (maximum 5). Supports common formats: .jpg, .jpeg, .png, .gif, .webp"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Custom prompt to guide the image analysis (e.g., \"Describe this image in detail\", \"What text appears in this image?\", etc.). Default: \"Describe this image in detail.\""
                    }
                },
                "required": ["imagePaths"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let paths = input.get("imagePaths");
        if !truthy(paths) {
            errors.push("Image paths are required".to_string());
        }
        let arr = paths.and_then(Value::as_array);
        if arr.is_none() {
            errors.push("Image paths must be an array".to_string());
        }
        if let Some(arr) = arr {
            if arr.is_empty() {
                errors.push("At least one image path is required".to_string());
            }
            if arr.len() > 5 {
                errors.push("Maximum 5 images are allowed".to_string());
            }
            for (i, p) in arr.iter().enumerate() {
                if !p.is_string() {
                    errors.push(format!("Image path at index {i} must be a string"));
                }
                if let Value::String(s) = p {
                    if !s.is_empty() && s.trim().is_empty() {
                        errors.push(format!("Image path at index {i} cannot be empty"));
                    }
                }
            }
        }
        if present(input, "prompt") && !is_string(input.get("prompt")) {
            errors.push("Prompt must be a string".to_string());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let paths: Vec<String> = input
            .get("imagePaths")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .take(5)
                    .map(|p| p.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::to_string);
        let model_id = ctx
            .settings
            .recognize_image_model_id
            .clone()
            .unwrap_or_else(|| DEFAULT_RECOGNIZE_MODEL.to_string());

        let results = futures_util::future::join_all(paths.iter().map(|path| {
            let req = RecognizeImageRequest {
                image_path: path.clone(),
                prompt: prompt.clone(),
                model_id: Some(model_id.clone()),
            };
            async move {
                let outcome = async {
                    if tokio::fs::metadata(&req.image_path).await.is_err() {
                        return Err(format!("Image file not found: {}", req.image_path));
                    }
                    self.backend
                        .recognize_image(&ctx.settings.converse, &req)
                        .await
                }
                .await;
                match outcome {
                    Ok(description) => json!({
                        "path": req.image_path,
                        "description": if description.is_empty() { "No description available".to_string() } else { description },
                        "success": true
                    }),
                    Err(e) => {
                        tracing::error!(path = %req.image_path, error = %e, "Failed to recognize image");
                        json!({
                            "path": req.image_path,
                            "description": format!("Error: {}", if e.is_empty() { "Failed to analyze this image" } else { &e }),
                            "success": false
                        })
                    }
                }
            }
        }))
        .await;

        let success_count = results.iter().filter(|r| r["success"] == true).count();
        Ok(ToolOutput::Json(json!({
            "name": NAME,
            "success": success_count > 0,
            "message": format!("Analyzed {success_count} of {} images successfully", paths.len()),
            "result": { "images": results, "modelUsed": model_id }
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake::FakeBedrock;
    use super::*;
    use crate::base::run_tool;
    use crate::context::ToolSettings;

    #[test]
    fn validation_messages() {
        let t = RecognizeImageTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({})),
            vec!["Image paths are required", "Image paths must be an array"]
        );
        assert_eq!(
            t.validate_input(&json!({"imagePaths": [], "prompt": 1})),
            vec![
                "At least one image path is required",
                "Prompt must be a string"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"imagePaths": ["a", 2, " ", "d", "e", "f"]})),
            vec![
                "Maximum 5 images are allowed",
                "Image path at index 1 must be a string",
                "Image path at index 2 cannot be empty"
            ]
        );
    }

    #[tokio::test]
    async fn recognizes_in_parallel_with_per_image_errors() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.recognize.lock().unwrap() = Some(Arc::new(|p: &str| {
            if p.ends_with("bad.png") {
                Err("Throttled".to_string())
            } else if p.ends_with("empty.png") {
                Ok(String::new())
            } else {
                Ok(format!("desc of {}", p.rsplit('/').next().unwrap()))
            }
        }));
        let dir = tempfile::tempdir().unwrap();
        let mk = |n: &str| {
            let p = dir.path().join(n);
            std::fs::write(&p, b"x").unwrap();
            p.to_string_lossy().into_owned()
        };
        let (a, bad, empty) = (mk("a.png"), mk("bad.png"), mk("empty.png"));
        let missing = dir
            .path()
            .join("missing.png")
            .to_string_lossy()
            .into_owned();
        let s = ToolSettings {
            recognize_image_model_id: Some("amazon.nova-pro-v1:0".into()),
            ..Default::default()
        };
        let v = run_tool(
            &RecognizeImageTool::new(fake.clone()),
            json!({"type": NAME, "imagePaths": [a, bad, empty, missing], "prompt": "What?"}),
            &ToolContext::new(s),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v,
            json!({
                "name": "recognizeImage",
                "success": true,
                "message": "Analyzed 2 of 4 images successfully",
                "result": {
                    "images": [
                        {"path": a, "description": "desc of a.png", "success": true},
                        {"path": bad, "description": "Error: Throttled", "success": false},
                        {"path": empty, "description": "No description available", "success": true},
                        {"path": missing, "description": format!("Error: Image file not found: {missing}"), "success": false}
                    ],
                    "modelUsed": "amazon.nova-pro-v1:0"
                }
            })
        );
        let calls = fake.calls();
        assert_eq!(calls.len(), 3);
        // Calls run concurrently, so their order is not fixed.
        assert!(calls.iter().any(|(_, c)| c
            == &json!({"imagePath": a, "prompt": "What?", "modelId": "amazon.nova-pro-v1:0"})));
    }

    #[tokio::test]
    async fn all_failures_is_unsuccessful_with_default_model() {
        let fake = Arc::new(FakeBedrock::default());
        let v = run_tool(
            &RecognizeImageTool::new(fake),
            json!({"type": NAME, "imagePaths": ["/nope/x.png"]}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["success"], false);
        assert_eq!(v["message"], "Analyzed 0 of 1 images successfully");
        assert_eq!(v["result"]["modelUsed"], DEFAULT_RECOGNIZE_MODEL);
    }
}
