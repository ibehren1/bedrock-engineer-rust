//! Port of `GenerateVideoTool.ts` (non-blocking: starts the Nova Reel job and returns its ARN).

use super::{is_string, present, truncate_for_logging, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::{len, truthy};
use async_trait::async_trait;
use bedrock::video::{GenerateMovieRequest, VALID_DURATIONS};
use serde_json::{json, Map, Value};
use std::sync::Arc;

const NAME: &str = "generateVideo";
const DESCRIPTION: &str = "Generate video using Amazon Nova Reel. Creates realistic, studio-quality videos from text prompts or images. Supports Text-to-Video (6 seconds) and Multi-Shot generation (12-120 seconds). For image input: single image for TEXT_VIDEO mode, multiple images for MULTI_SHOT_MANUAL mode. Returns immediately with job ARN for status tracking.";

pub struct GenerateVideoTool {
    backend: Arc<dyn BedrockBackend>,
}

impl GenerateVideoTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

fn is_number(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Number(_)))
}

/// `validDurations.includes(v)`.
fn is_valid_duration(v: Option<&Value>) -> bool {
    v.and_then(Value::as_f64)
        .is_some_and(|d| VALID_DURATIONS.iter().any(|x| f64::from(*x) == d))
}

/// `imagePath.toLowerCase().split('.').pop()`.
fn extension(path: &str) -> String {
    path.to_lowercase()
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// `value.length` of `input.inputImages` as a template-literal string.
fn js_length(v: &Value) -> String {
    match v {
        Value::Array(a) => a.len().to_string(),
        Value::String(s) => len(s).to_string(),
        _ => "undefined".to_string(),
    }
}

/// `Math.round`.
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// A JS number as `${n}`.
fn js_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e21 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[async_trait]
impl Tool for GenerateVideoTool {
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
                    "prompt": {
                        "type": "string",
                        "description": "Text description of the video (1-4000 characters)",
                        "minLength": 1,
                        "maxLength": 512
                    },
                    "durationSeconds": {
                        "type": "number",
                        "description": "Duration: 6 seconds (TEXT_VIDEO) or 12-120 seconds in multiples of 6 (MULTI_SHOT_AUTOMATED)",
                        "enum": VALID_DURATIONS
                    },
                    "outputPath": {
                        "type": "string",
                        "description": "Optional. Local path to save video. Uses project path if not specified."
                    },
                    "seed": {
                        "type": "number",
                        "description": "Optional. Seed for deterministic generation (0-2147483646)",
                        "minimum": 0,
                        "maximum": 2147483646
                    },
                    "inputImages": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional. Array of input image file paths (must be 1280x720 resolution, PNG or JPEG format). For TEXT_VIDEO: single image only. For MULTI_SHOT: multiple images supported as shot starting frames."
                    },
                    "prompts": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional. Array of prompts for each shot when using multiple images. Must match the number of inputImages if provided."
                    }
                },
                "required": ["prompt", "durationSeconds"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();
        let prompt = input.get("prompt");
        if !truthy(prompt) {
            errors.push("Prompt is required".into());
        }
        if !is_string(prompt) {
            errors.push("Prompt must be a string".into());
        }
        if let Some(Value::String(p)) = prompt.filter(|p| truthy(Some(p))) {
            if p.trim().is_empty() {
                errors.push("Prompt cannot be empty".into());
            }
            if len(p) > 4000 {
                errors.push("Prompt must be 4000 characters or less".into());
            }
        }

        let duration = input.get("durationSeconds");
        if !is_number(duration) {
            errors.push("Duration seconds must be a number".into());
        }
        if truthy(duration) && !is_valid_duration(duration) {
            errors.push(
                "Duration must be 6 seconds (TEXT_VIDEO) or a multiple of 6 between 12-120 seconds (MULTI_SHOT_AUTOMATED)".into(),
            );
        }

        if present(input, "outputPath") && !is_string(input.get("outputPath")) {
            errors.push("Output path must be a string".into());
        }

        if present(input, "seed") {
            match input.get("seed").and_then(Value::as_f64) {
                Some(s) if is_number(input.get("seed")) => {
                    if !(0.0..=2_147_483_646.0).contains(&s) {
                        errors.push("Seed must be between 0 and 2147483646".into());
                    }
                }
                _ => errors.push("Seed must be a number".into()),
            }
        }

        let images = input.get("inputImages");
        if present(input, "inputImages") {
            match images {
                Some(Value::Array(items)) => {
                    if items.is_empty() {
                        errors.push("inputImages array cannot be empty when provided".into());
                    }
                    if duration.and_then(Value::as_f64) == Some(6.0) && items.len() > 1 {
                        errors.push(
                            "TEXT_VIDEO mode (6 seconds) supports only a single input image".into(),
                        );
                    }
                    for (i, item) in items.iter().enumerate() {
                        match item {
                            Value::String(p) if p.trim().is_empty() => {
                                errors.push(format!("inputImages[{i}] cannot be empty"));
                            }
                            Value::String(p) => {
                                if !matches!(extension(p).as_str(), "png" | "jpg" | "jpeg") {
                                    errors.push(format!(
                                        "inputImages[{i}] must be PNG or JPEG format ({p})"
                                    ));
                                }
                            }
                            _ => errors.push(format!("inputImages[{i}] must be a string")),
                        }
                    }
                }
                _ => errors.push("inputImages must be an array".into()),
            }
        }

        let prompts = input.get("prompts");
        if present(input, "prompts") {
            match prompts {
                Some(Value::Array(items)) => {
                    for (i, item) in items.iter().enumerate() {
                        match item {
                            Value::String(p) if p.trim().is_empty() => {
                                errors.push(format!("prompts[{i}] cannot be empty"));
                            }
                            Value::String(p) if len(p) > 4000 => {
                                errors
                                    .push(format!("prompts[{i}] must be 4000 characters or less"));
                            }
                            Value::String(_) => {}
                            _ => errors.push(format!("prompts[{i}] must be a string")),
                        }
                    }
                    if let Some(imgs) = images.filter(|v| truthy(Some(v))) {
                        let n = js_length(imgs);
                        if n != items.len().to_string() {
                            errors.push(format!(
                                "prompts array length ({}) must match inputImages array length ({n})",
                                items.len()
                            ));
                        }
                    }
                }
                _ => errors.push("prompts must be an array".into()),
            }
        }

        if truthy(prompts) && !truthy(images) {
            errors.push("prompts array can only be used together with inputImages".into());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let duration_value = input.get("durationSeconds").cloned().unwrap_or(Value::Null);
        let duration = duration_value.as_f64().unwrap_or(0.0);
        let output_path = input
            .get("outputPath")
            .and_then(Value::as_str)
            .map(str::to_string);
        let seed = input.get("seed").cloned();
        let strings = |key: &str| {
            input.get(key).and_then(Value::as_array).map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect::<Vec<_>>()
            })
        };
        let input_images = strings("inputImages");
        let prompts = strings("prompts");

        let Some(s3_uri) = ctx.settings.generate_video_s3_uri.clone() else {
            let mut extra = Map::new();
            extra.insert(
                "hint".into(),
                json!("Go to Tool Settings and configure generateVideo S3 URI (e.g., s3://your-bucket/videos/)"),
            );
            return Err(ToolError::execution(
                "S3 URI is not configured. Please configure S3 URI in tool settings before generating videos.",
                NAME,
                None,
                Some(extra),
            ));
        };
        if !s3_uri.starts_with("s3://") {
            let mut extra = Map::new();
            extra.insert("s3Uri".into(), json!(s3_uri));
            return Err(ToolError::execution(
                "Invalid S3 URI format. S3 URI must start with s3://",
                NAME,
                None,
                Some(extra),
            ));
        }

        let estimated_minutes = js_number(js_round(duration * 1.5));
        tracing::info!(
            duration_seconds = duration,
            estimated_time = %format!("{estimated_minutes} minutes"),
            s3_uri = %s3_uri,
            "Starting Nova Reel video generation"
        );

        let req = GenerateMovieRequest {
            prompt: prompt.clone(),
            duration_seconds: duration,
            output_path: output_path.clone(),
            seed: seed.as_ref().and_then(Value::as_f64),
            s3_uri: s3_uri.clone(),
            input_images: input_images.clone(),
            prompts,
        };
        let result = async {
            let response = self
                .backend
                .start_video_generation(&ctx.settings.converse, &req)
                .await?;
            if response.invocation_arn.is_empty() {
                return Err(
                    "Failed to start video generation: No invocation ARN returned".to_string(),
                );
            }
            Ok(response.invocation_arn)
        }
        .await;

        match result {
            Ok(arn) => {
                tracing::info!(invocation_arn = %arn, "Video generation started successfully");
                let mut r = Map::new();
                r.insert("invocationArn".into(), json!(arn));
                r.insert("status".into(), json!("InProgress"));
                r.insert("prompt".into(), json!(prompt));
                r.insert("durationSeconds".into(), duration_value);
                if let Some(seed) = seed {
                    r.insert("seed".into(), seed);
                }
                r.insert(
                    "estimatedCompletionTime".into(),
                    json!(format!("{estimated_minutes} minutes")),
                );
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": format!("Video generation started successfully. Use checkVideoStatus with ARN: {arn} to track progress."),
                    "result": Value::Object(r)
                })))
            }
            Err(e) => {
                tracing::error!(error = %e, "Error starting video generation");
                let mut extra = Map::new();
                extra.insert("prompt".into(), json!(truncate_for_logging(&prompt, 100)));
                extra.insert("durationSeconds".into(), duration_value);
                extra.insert("s3Uri".into(), json!(s3_uri));
                if let Some(p) = output_path {
                    extra.insert("outputPath".into(), json!(p));
                }
                extra.insert("hasInputImages".into(), json!(input_images.is_some()));
                extra.insert(
                    "imageCount".into(),
                    json!(input_images.as_ref().map_or(0, Vec::len)),
                );
                Err(ToolError::execution(
                    format!("Error starting video generation: {e}"),
                    NAME,
                    Some(json!({})),
                    Some(extra),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake::{response, FakeBedrock};
    use super::*;
    use crate::base::run_tool;
    use crate::context::ToolSettings;
    use bedrock::video::GeneratedMovie;

    fn ctx(s3: Option<&str>) -> ToolContext {
        let mut s = ToolSettings::from_store(&json!({"aws": {"region": "us-east-1"}}));
        s.generate_video_s3_uri = s3.map(str::to_string);
        ToolContext::new(s)
    }

    fn tool() -> GenerateVideoTool {
        GenerateVideoTool::new(Arc::new(FakeBedrock::default()))
    }

    #[test]
    fn spec_matches_ts() {
        let spec = tool().spec().unwrap().to_bedrock_tool();
        let schema = &spec["toolSpec"]["inputSchema"]["json"];
        assert_eq!(schema["required"], json!(["prompt", "durationSeconds"]));
        assert_eq!(schema["properties"]["prompt"]["maxLength"], 512);
        assert_eq!(schema["properties"]["durationSeconds"]["enum"][19], 120);
        assert_eq!(schema["properties"]["seed"]["maximum"], 2147483646);
    }

    #[test]
    fn validation_messages() {
        let t = tool();
        assert_eq!(
            t.validate_input(&json!({})),
            vec![
                "Prompt is required",
                "Prompt must be a string",
                "Duration seconds must be a number"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": " ", "durationSeconds": 7, "outputPath": 1, "seed": -1})),
            vec![
                "Prompt cannot be empty",
                "Duration must be 6 seconds (TEXT_VIDEO) or a multiple of 6 between 12-120 seconds (MULTI_SHOT_AUTOMATED)",
                "Output path must be a string",
                "Seed must be between 0 and 2147483646"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "x".repeat(4001), "durationSeconds": "6", "seed": null})),
            vec![
                "Prompt must be 4000 characters or less",
                "Duration seconds must be a number",
                "Duration must be 6 seconds (TEXT_VIDEO) or a multiple of 6 between 12-120 seconds (MULTI_SHOT_AUTOMATED)",
                "Seed must be a number"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "p", "durationSeconds": 6, "inputImages": ["a.png", " ", 3, "b.gif"]})),
            vec![
                "TEXT_VIDEO mode (6 seconds) supports only a single input image",
                "inputImages[1] cannot be empty",
                "inputImages[2] must be a string",
                "inputImages[3] must be PNG or JPEG format (b.gif)"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "p", "durationSeconds": 12, "inputImages": [], "prompts": ["a", "", 1, "x".repeat(4001)]})),
            vec![
                "inputImages array cannot be empty when provided",
                "prompts[1] cannot be empty",
                "prompts[2] must be a string",
                "prompts[3] must be 4000 characters or less",
                "prompts array length (4) must match inputImages array length (0)"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "p", "durationSeconds": 12, "inputImages": "a.png", "prompts": "x"})),
            vec!["inputImages must be an array", "prompts must be an array"]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "p", "durationSeconds": 12, "prompts": ["x"]})),
            vec!["prompts array can only be used together with inputImages"]
        );
        assert!(t
            .validate_input(&json!({"prompt": "p", "durationSeconds": 12, "seed": 5, "inputImages": ["a.JPG", "b.jpeg"], "prompts": ["x", "y"]}))
            .is_empty());
    }

    #[tokio::test]
    async fn starts_job_and_returns_arn() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.start_video.lock().unwrap() = Some(Ok(GeneratedMovie {
            invocation_arn: "arn:aws:bedrock:job/1".into(),
            ..Default::default()
        }));
        let t = GenerateVideoTool::new(fake.clone());
        let v = run_tool(
            &t,
            json!({"type": NAME, "prompt": "a cat", "durationSeconds": 12, "seed": 3, "inputImages": ["/a.png", "/b.png"]}),
            &ctx(Some("s3://bucket/videos/")),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v,
            json!({
                "success": true,
                "name": "generateVideo",
                "message": "Video generation started successfully. Use checkVideoStatus with ARN: arn:aws:bedrock:job/1 to track progress.",
                "result": {
                    "invocationArn": "arn:aws:bedrock:job/1",
                    "status": "InProgress",
                    "prompt": "a cat",
                    "durationSeconds": 12,
                    "seed": 3,
                    "estimatedCompletionTime": "18 minutes"
                }
            })
        );
        assert_eq!(
            fake.calls()[0],
            (
                "startVideoGeneration".to_string(),
                json!({
                    "prompt": "a cat", "durationSeconds": 12.0, "seed": 3.0,
                    "s3Uri": "s3://bucket/videos/", "inputImages": ["/a.png", "/b.png"]
                })
            )
        );
    }

    #[tokio::test]
    async fn s3_configuration_and_backend_errors() {
        let fake = Arc::new(FakeBedrock::default());
        let t = GenerateVideoTool::new(fake.clone());
        let input = json!({"type": NAME, "prompt": "p", "durationSeconds": 6});
        let err = run_tool(&t, input.clone(), &ctx(None)).await.unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "S3 URI is not configured. Please configure S3 URI in tool settings before generating videos.",
                "type": "EXECUTION",
                "toolName": "generateVideo",
                "hint": "Go to Tool Settings and configure generateVideo S3 URI (e.g., s3://your-bucket/videos/)"
            })
        );
        let err = run_tool(&t, input.clone(), &ctx(Some("bucket/x")))
            .await
            .unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Invalid S3 URI format. S3 URI must start with s3://",
                "type": "EXECUTION",
                "toolName": "generateVideo",
                "s3Uri": "bucket/x"
            })
        );
        assert!(fake.calls().is_empty());

        *fake.start_video.lock().unwrap() =
            Some(Err("Request was throttled. Please try again later.".into()));
        let err = run_tool(
            &t,
            json!({"type": NAME, "prompt": "p", "durationSeconds": 6, "outputPath": "/o.mp4"}),
            &ctx(Some("s3://b")),
        )
        .await
        .unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Error starting video generation: Request was throttled. Please try again later.",
                "type": "EXECUTION",
                "toolName": "generateVideo",
                "cause": {},
                "prompt": "p",
                "durationSeconds": 6,
                "s3Uri": "s3://b",
                "outputPath": "/o.mp4",
                "hasInputImages": false,
                "imageCount": 0
            })
        );

        *fake.start_video.lock().unwrap() = Some(Ok(GeneratedMovie::default()));
        let err = run_tool(&t, input, &ctx(Some("s3://b"))).await.unwrap_err();
        assert_eq!(
            response(&err)["error"],
            "Error starting video generation: Failed to start video generation: No invocation ARN returned"
        );
    }
}
