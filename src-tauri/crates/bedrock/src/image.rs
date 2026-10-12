//! Image generation — port of `src/main/api/bedrock/services/imageService.ts`
//! (`bedrock:generateImage` IPC handler, used by the `generateImage` tool).
//!
//! Stability (Core / Ultra / SD3.5) through `InvokeModel`, with the same request bodies and
//! response checks as the TS. Amazon Nova Canvas and Titan Image Generator were dropped (EOL /
//! no longer offered on Bedrock), along with their request/response code paths.

use crate::error::{Error, Result};
use crate::sdk::SdkConfigSource;
use crate::settings::AwsSettings;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// `getSupportedModels()`.
pub const SUPPORTED_IMAGE_MODELS: [&str; 3] = [
    "stability.sd3-5-large-v1:0",
    "stability.stable-image-core-v1:0",
    "stability.stable-image-ultra-v1:0",
];

/// `GenerateImageRequest` (`src/main/api/bedrock/types/image.ts`). Note the snake_case
/// `aspect_ratio` / `output_format` keys, as in the TS type.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateImageRequest {
    pub model_id: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// `'1:1' | '16:9' | '2:3' | '3:2' | '4:5' | '5:4' | '9:16' | '9:21'`.
    #[serde(
        rename = "aspect_ratio",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// `png` (default) | `jpeg` | `webp`.
    #[serde(
        rename = "output_format",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub output_format: Option<String>,
}

/// `GeneratedImage`: base64 images, plus Stability's `seeds` / `finish_reasons`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub images: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeds: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reasons: Option<Vec<Value>>,
}

/// `ModelType` (all Stability models).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageModelType {
    Core,
    Ultra,
    Sd3,
}

/// `isModelSupported(modelId)`.
pub fn is_image_model_supported(model_id: &str) -> bool {
    SUPPORTED_IMAGE_MODELS.contains(&model_id)
}

/// `getModelType(modelId)`: first match of `core`, `ultra`, `sd3`.
pub fn image_model_type(model_id: &str) -> Result<ImageModelType> {
    let t = if model_id.contains("core") {
        ImageModelType::Core
    } else if model_id.contains("ultra") {
        ImageModelType::Ultra
    } else if model_id.contains("sd3") {
        ImageModelType::Sd3
    } else {
        return Err(Error::plain(format!(
            "Unknown model type for modelId: {model_id}"
        )));
    };
    Ok(t)
}

/// `buildRequestBody(modelType, request)`. `JSON.stringify` drops the undefined members.
pub fn build_image_request_body(model_type: ImageModelType, req: &GenerateImageRequest) -> Value {
    let output_format = req.output_format.clone().unwrap_or_else(|| "png".into());
    let mut body = Map::new();
    body.insert("prompt".into(), json!(req.prompt));
    if let Some(n) = &req.negative_prompt {
        body.insert("negative_prompt".into(), json!(n));
    }
    if let Some(a) = &req.aspect_ratio {
        body.insert("aspect_ratio".into(), json!(a));
    }
    if let Some(s) = req.seed {
        body.insert("seed".into(), json!(s));
    }
    if model_type == ImageModelType::Sd3 {
        body.insert("mode".into(), json!("text-to-image"));
    }
    body.insert("output_format".into(), json!(output_format));
    Value::Object(body)
}

/// Response-body handling of `generateImage`.
pub fn parse_image_response(body: &[u8]) -> Result<GeneratedImage> {
    let result: Value = serde_json::from_slice(body).map_err(|e| Error::Custom {
        name: "SyntaxError".into(),
        message: e.to_string(),
        fields: Map::new(),
    })?;
    let images = result
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::plain("Invalid response format from Bedrock"))?;
    let list = |key: &str| result.get(key).and_then(Value::as_array).cloned();
    Ok(GeneratedImage {
        images: images
            .iter()
            .map(|v| v.as_str().map(str::to_string).unwrap_or_default())
            .collect(),
        seeds: list("seeds"),
        finish_reasons: list("finish_reasons"),
    })
}

/// `generateImage(request)`.
pub async fn generate_image(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    req: &GenerateImageRequest,
) -> Result<GeneratedImage> {
    if !is_image_model_supported(&req.model_id) {
        return Err(Error::plain(format!(
            "Model {} is not supported. Supported models: {}",
            req.model_id,
            SUPPORTED_IMAGE_MODELS.join(", ")
        )));
    }
    if aws.region.is_empty()
        || (!aws.use_profile.unwrap_or(false)
            && (aws.access_key_id.is_empty() || aws.secret_access_key.is_empty()))
    {
        tracing::warn!("AWS credentials not configured properly");
    }
    let model_type = image_model_type(&req.model_id)?;
    let body = build_image_request_body(model_type, req);
    let result = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrockruntime::Client::new(&conf);
        let out = client
            .invoke_model()
            .model_id(&req.model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(aws_smithy_types::Blob::new(
                serde_json::to_vec(&body).unwrap_or_default(),
            ))
            .send()
            .await
            .map_err(Error::from)?;
        parse_image_response(out.body.as_ref())
    }
    .await;
    result.map_err(|e| {
        tracing::error!(error = %e, "Error generating image");
        match e.service_name() {
            Some("UnrecognizedClientException") => Error::plain(
                "AWS authentication failed. Please check your credentials and permissions.",
            ),
            Some("ValidationException") => {
                Error::plain(format!("Invalid request parameters: {}", e.message()))
            }
            _ => e,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(model_id: &str) -> GenerateImageRequest {
        GenerateImageRequest {
            model_id: model_id.into(),
            prompt: "a cat".into(),
            ..Default::default()
        }
    }

    #[test]
    fn model_types_follow_substring_order() {
        use ImageModelType::*;
        assert_eq!(
            image_model_type("stability.stable-image-core-v1:0").unwrap(),
            Core
        );
        assert_eq!(
            image_model_type("stability.stable-image-ultra-v1:0").unwrap(),
            Ultra
        );
        assert_eq!(image_model_type("stability.sd3-5-large-v1:0").unwrap(), Sd3);
        assert_eq!(
            image_model_type("x").unwrap_err().to_string(),
            "Unknown model type for modelId: x"
        );
        for id in SUPPORTED_IMAGE_MODELS {
            assert!(is_image_model_supported(id));
        }
        for removed in [
            "amazon.nova-canvas-v1:0",
            "amazon.titan-image-generator-v1",
            "amazon.titan-image-generator-v2:0",
            "stability.sd3-large-v1:0",
            "stability.stable-image-core-v1:1",
            "stability.stable-image-ultra-v1:1",
            "amazon.nova-reel-v1:0",
        ] {
            assert!(!is_image_model_supported(removed), "{removed}");
        }
    }

    #[test]
    fn stability_bodies_drop_undefined_members() {
        let body = build_image_request_body(
            ImageModelType::Core,
            &req("stability.stable-image-core-v1:0"),
        );
        assert_eq!(body, json!({ "prompt": "a cat", "output_format": "png" }));

        let mut r = req("stability.sd3-5-large-v1:0");
        r.negative_prompt = Some("blurry".into());
        r.aspect_ratio = Some("16:9".into());
        r.seed = Some(0);
        r.output_format = Some("jpeg".into());
        let body = build_image_request_body(ImageModelType::Sd3, &r);
        assert_eq!(
            body,
            json!({
                "prompt": "a cat", "negative_prompt": "blurry", "aspect_ratio": "16:9",
                "seed": 0, "mode": "text-to-image", "output_format": "jpeg"
            })
        );
    }

    #[test]
    fn response_parsing() {
        let ok = parse_image_response(br#"{"images":["AAA"],"seeds":[1],"finish_reasons":[null]}"#)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&ok).unwrap(),
            json!({ "images": ["AAA"], "seeds": [1], "finish_reasons": [null] })
        );
        let minimal = parse_image_response(br#"{"images":["B"]}"#).unwrap();
        assert_eq!(
            serde_json::to_value(&minimal).unwrap(),
            json!({ "images": ["B"] })
        );
        assert_eq!(
            parse_image_response(br#"{}"#).unwrap_err().to_string(),
            "Invalid response format from Bedrock"
        );
        assert_eq!(
            parse_image_response(b"nope").unwrap_err().name(),
            "SyntaxError"
        );
    }

    #[test]
    fn request_json_uses_ts_keys() {
        let r: GenerateImageRequest = serde_json::from_value(json!({
            "modelId": "stability.sd3-5-large-v1:0", "prompt": "p", "negativePrompt": "n",
            "aspect_ratio": "1:1", "seed": 3, "output_format": "png"
        }))
        .unwrap();
        assert_eq!(r.aspect_ratio.as_deref(), Some("1:1"));
        assert_eq!(r.negative_prompt.as_deref(), Some("n"));
        assert_eq!(r.output_format.as_deref(), Some("png"));
    }
}
