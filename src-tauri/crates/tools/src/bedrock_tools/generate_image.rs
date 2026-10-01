//! Port of `GenerateImageTool.ts`.

use super::{is_string, present, truncate_for_logging, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use base64::Engine;
use bedrock::image::GenerateImageRequest;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Arc;

const NAME: &str = "generateImage";
const DESCRIPTION: &str = "Generate an image using Amazon Bedrock Foundation Models. By default uses stability.sd3-5-large-v1:0. Images are saved to the specified path. For Titan models, specific aspect ratios and sizes are supported.\n\nGenerate images using AI models. Always ask user permission before creating images.";

/// `DEFAULT_IMAGE_MODEL`.
pub const DEFAULT_IMAGE_MODEL: &str = "stability.sd3-5-large-v1:0";

pub struct GenerateImageTool {
    backend: Arc<dyn BedrockBackend>,
}

impl GenerateImageTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

/// `Buffer.from(data, 'base64')`: lenient about padding and the URL-safe alphabet.
fn decode_base64(data: &str) -> Vec<u8> {
    let cleaned: String = data
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(cleaned.as_bytes())
        .unwrap_or_default()
}

async fn save_image(output_path: &str, data: &[u8]) -> std::result::Result<(), String> {
    let path = Path::new(output_path);
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        let d = dir.to_string_lossy();
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| crate::util::node_io::NodeIoError::new(&e, "mkdir", &d, None).message)?;
    }
    tokio::fs::write(path, data).await.map_err(|e| {
        crate::util::node_io::NodeIoError::new(&e, "open", output_path, None).message
    })?;
    let verified = match tokio::fs::metadata(path).await {
        Ok(m) if m.len() == 0 => Err("Generated image file is empty".to_string()),
        Ok(_) => Ok(()),
        Err(e) => {
            Err(crate::util::node_io::NodeIoError::new(&e, "access", output_path, None).message)
        }
    };
    verified.map_err(|e| format!("Failed to verify saved image: {e}"))
}

#[async_trait]
impl Tool for GenerateImageTool {
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
                        "description": "Text description of the image you want to generate"
                    },
                    "outputPath": {
                        "type": "string",
                        "description": "Path where the generated image should be saved, including filename (e.g., \"/path/to/image.png\")"
                    },
                    "negativePrompt": {
                        "type": "string",
                        "description": "Optional. Things to exclude from the image"
                    },
                    "aspect_ratio": {
                        "type": "string",
                        "description": "Optional. Aspect ratio of the generated image. For Titan models, specific sizes will be chosen based on the aspect ratio.",
                        "enum": ["1:1", "16:9", "2:3", "3:2", "4:5", "5:4", "9:16", "9:21", "5:3", "3:5", "7:9", "9:7", "6:11", "11:6", "5:11", "11:5", "9:5"]
                    },
                    "seed": {
                        "type": "number",
                        "description": "Optional. Seed for deterministic generation. For Titan models, range is 0 to 2147483647."
                    },
                    "output_format": {
                        "type": "string",
                        "description": "Optional. Output format of the generated image",
                        "enum": ["png", "jpeg", "webp"],
                        "default": "png"
                    }
                },
                "required": ["prompt", "outputPath"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let prompt = input.get("prompt");
        if !truthy(prompt) {
            errors.push("Prompt is required".to_string());
        }
        if !is_string(prompt) {
            errors.push("Prompt must be a string".to_string());
        }
        if let Some(Value::String(p)) = prompt {
            if !p.is_empty() && p.trim().is_empty() {
                errors.push("Prompt cannot be empty".to_string());
            }
        }
        let output = input.get("outputPath");
        if !truthy(output) {
            errors.push("Output path is required".to_string());
        }
        if !is_string(output) {
            errors.push("Output path must be a string".to_string());
        }
        for (key, message) in [
            ("modelId", "Model ID must be a string"),
            ("negativePrompt", "Negative prompt must be a string"),
            ("aspect_ratio", "Aspect ratio must be a string"),
        ] {
            if present(input, key) && !is_string(input.get(key)) {
                errors.push(message.to_string());
            }
        }
        if present(input, "seed") && !matches!(input.get("seed"), Some(Value::Number(_))) {
            errors.push("Seed must be a number".to_string());
        }
        if present(input, "output_format")
            && !matches!(
                input.get("output_format").and_then(Value::as_str),
                Some("png" | "jpeg" | "webp")
            )
        {
            errors.push("Output format must be one of: png, jpeg, webp".to_string());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let s = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
        let prompt = s("prompt").unwrap_or_default();
        let output_path = s("outputPath").unwrap_or_default();
        let negative_prompt = s("negativePrompt");
        let aspect_ratio = s("aspect_ratio");
        let seed = input.get("seed").and_then(Value::as_f64);
        let output_format = s("output_format").unwrap_or_else(|| "png".to_string());

        // input > configured (generateImageTool.modelId) > default.
        let model_id = s("modelId")
            .filter(|m| !m.is_empty())
            .or_else(|| ctx.settings.generate_image_model_id.clone())
            .unwrap_or_else(|| DEFAULT_IMAGE_MODEL.to_string());

        tracing::info!(model_id = %model_id, output_path = %output_path, "Calling Bedrock image generation API");

        let result = async {
            let req = GenerateImageRequest {
                model_id: model_id.clone(),
                prompt: prompt.clone(),
                negative_prompt: negative_prompt.clone(),
                aspect_ratio: aspect_ratio.clone(),
                seed: seed.map(|n| n as i64),
                output_format: Some(output_format.clone()),
            };
            let response = self
                .backend
                .generate_image(&ctx.settings.converse, &req)
                .await?;
            let Some(image) = response.images.first() else {
                return Err("No image was generated".to_string());
            };
            save_image(&output_path, &decode_base64(image)).await?;
            Ok(response.seeds.and_then(|s| s.into_iter().next()))
        }
        .await;

        match result {
            Ok(seed) => {
                let mut r = Map::new();
                r.insert("imagePath".into(), json!(output_path));
                r.insert("prompt".into(), json!(prompt));
                if let Some(n) = negative_prompt {
                    r.insert("negativePrompt".into(), json!(n));
                }
                r.insert(
                    "aspect_ratio".into(),
                    json!(aspect_ratio.as_deref().unwrap_or("1:1")),
                );
                r.insert("modelUsed".into(), json!(model_id));
                if let Some(seed) = seed.filter(|s| !s.is_null()) {
                    r.insert("seed".into(), seed);
                }
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": format!("Image generated successfully and saved to {output_path}"),
                    "result": Value::Object(r)
                })))
            }
            Err(e) => {
                tracing::error!(error = %e, model_id = %model_id, "Error generating image");
                let mut extra = Map::new();
                extra.insert("prompt".into(), json!(truncate_for_logging(&prompt, 100)));
                extra.insert("modelId".into(), json!(model_id));
                extra.insert("outputPath".into(), json!(output_path));
                Err(ToolError::execution(
                    format!("Error generating image: {e}"),
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
    use bedrock::image::GeneratedImage;

    fn ctx(model: Option<&str>) -> ToolContext {
        let mut s = ToolSettings::from_store(&json!({"aws": {"region": "us-west-2"}}));
        s.generate_image_model_id = model.map(str::to_string);
        ToolContext::new(s)
    }

    #[test]
    fn validation_messages() {
        let t = GenerateImageTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({})),
            vec![
                "Prompt is required",
                "Prompt must be a string",
                "Output path is required",
                "Output path must be a string"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"prompt": "  ", "outputPath": "/x.png", "modelId": 1, "negativePrompt": null, "aspect_ratio": 2, "seed": "1", "output_format": "gif"})),
            vec![
                "Prompt cannot be empty",
                "Model ID must be a string",
                "Negative prompt must be a string",
                "Aspect ratio must be a string",
                "Seed must be a number",
                "Output format must be one of: png, jpeg, webp"
            ]
        );
        assert!(t
            .validate_input(&json!({"prompt": "cat", "outputPath": "/x.png", "seed": 3}))
            .is_empty());
    }

    #[tokio::test]
    async fn saves_image_and_reports_result() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.image.lock().unwrap() = Some(Ok(GeneratedImage {
            images: vec![base64::engine::general_purpose::STANDARD.encode(b"PNGDATA")],
            seeds: Some(vec![json!(42)]),
            finish_reasons: None,
        }));
        let t = GenerateImageTool::new(fake.clone());
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("nested/dir/cat.png");
        let out_s = out.to_string_lossy().into_owned();
        let v = run_tool(
            &t,
            json!({"type": NAME, "prompt": "a cat", "outputPath": out_s, "aspect_ratio": "16:9"}),
            &ctx(Some("amazon.nova-canvas-v1:0")),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(std::fs::read(&out).unwrap(), b"PNGDATA");
        assert_eq!(
            v,
            json!({
                "success": true,
                "name": "generateImage",
                "message": format!("Image generated successfully and saved to {out_s}"),
                "result": {
                    "imagePath": out_s,
                    "prompt": "a cat",
                    "aspect_ratio": "16:9",
                    "modelUsed": "amazon.nova-canvas-v1:0",
                    "seed": 42
                }
            })
        );
        let calls = fake.calls();
        assert_eq!(calls[0].1["region"], "us-west-2");
        assert_eq!(
            calls[0].1["req"],
            json!({"modelId": "amazon.nova-canvas-v1:0", "prompt": "a cat", "aspect_ratio": "16:9", "output_format": "png"})
        );
    }

    #[tokio::test]
    async fn model_priority_and_errors() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.image.lock().unwrap() = Some(Ok(GeneratedImage::default()));
        let t = GenerateImageTool::new(fake.clone());
        let err = run_tool(
            &t,
            json!({"type": NAME, "prompt": "a cat", "outputPath": "/tmp/x.png"}),
            &ctx(None),
        )
        .await
        .unwrap_err();
        assert_eq!(fake.calls()[0].1["req"]["modelId"], DEFAULT_IMAGE_MODEL);
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Error generating image: No image was generated",
                "type": "EXECUTION",
                "toolName": "generateImage",
                "cause": {},
                "prompt": "a cat",
                "modelId": DEFAULT_IMAGE_MODEL,
                "outputPath": "/tmp/x.png"
            })
        );

        *fake.image.lock().unwrap() = Some(Err("Invalid request parameters: bad".into()));
        let err = run_tool(
            &t,
            json!({"type": NAME, "prompt": "p", "outputPath": "/tmp/y.png", "modelId": "amazon.titan-image-generator-v2:0"}),
            &ctx(Some("configured")),
        )
        .await
        .unwrap_err();
        assert_eq!(
            fake.calls()[1].1["req"]["modelId"],
            "amazon.titan-image-generator-v2:0"
        );
        assert_eq!(
            response(&err)["error"],
            "Error generating image: Invalid request parameters: bad"
        );
    }

    #[test]
    fn lenient_base64() {
        assert_eq!(decode_base64("aGk="), b"hi");
        assert_eq!(decode_base64("aGk"), b"hi");
        assert_eq!(decode_base64("-_8"), vec![0xfb, 0xff]);
    }
}
