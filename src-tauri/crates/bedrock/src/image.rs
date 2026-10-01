//! Image generation — port of `src/main/api/bedrock/services/imageService.ts`
//! (`bedrock:generateImage` IPC handler, used by the `generateImage` tool).
//!
//! Stability (Core / Ultra / SD3), Amazon Nova Canvas and Titan Image Generator through
//! `InvokeModel`, with the same request bodies, size selection and response checks as the TS.

use crate::error::{Error, Result};
use crate::retry::random_index;
use crate::sdk::SdkConfigSource;
use crate::settings::AwsSettings;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// `getSupportedModels()`.
pub const SUPPORTED_IMAGE_MODELS: [&str; 9] = [
    "stability.sd3-large-v1:0",
    "stability.sd3-5-large-v1:0",
    "stability.stable-image-core-v1:0",
    "stability.stable-image-core-v1:1",
    "stability.stable-image-ultra-v1:0",
    "stability.stable-image-ultra-v1:1",
    "amazon.nova-canvas-v1:0",
    "amazon.titan-image-generator-v2:0",
    "amazon.titan-image-generator-v1",
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

/// `ModelType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageModelType {
    Core,
    Ultra,
    Sd3,
    Nova,
    Titan,
}

impl ImageModelType {
    fn is_stability(self) -> bool {
        matches!(self, Self::Core | Self::Ultra | Self::Sd3)
    }
}

/// `isModelSupported(modelId)`.
pub fn is_image_model_supported(model_id: &str) -> bool {
    SUPPORTED_IMAGE_MODELS.contains(&model_id)
}

/// `getModelType(modelId)`: first match of `core`, `ultra`, `sd3`, `nova`, `titan`.
pub fn image_model_type(model_id: &str) -> Result<ImageModelType> {
    let t = if model_id.contains("core") {
        ImageModelType::Core
    } else if model_id.contains("ultra") {
        ImageModelType::Ultra
    } else if model_id.contains("sd3") {
        ImageModelType::Sd3
    } else if model_id.contains("nova") {
        ImageModelType::Nova
    } else if model_id.contains("titan") {
        ImageModelType::Titan
    } else {
        return Err(Error::plain(format!(
            "Unknown model type for modelId: {model_id}"
        )));
    };
    Ok(t)
}

/// A Titan-accepted size (`titanAllowedSizes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitanSize {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: &'static str,
    /// `512x512` or `1024x1024` pricing tier.
    pub large: bool,
}

const fn ts(width: u32, height: u32, aspect_ratio: &'static str, large: bool) -> TitanSize {
    TitanSize {
        width,
        height,
        aspect_ratio,
        large,
    }
}

/// `titanAllowedSizes`, in the TS order (the order decides ties).
pub const TITAN_ALLOWED_SIZES: [TitanSize; 25] = [
    ts(1024, 1024, "1:1", true),
    ts(768, 768, "1:1", false),
    ts(512, 512, "1:1", false),
    ts(768, 1152, "2:3", true),
    ts(384, 576, "2:3", false),
    ts(1152, 768, "3:2", true),
    ts(576, 384, "3:2", false),
    ts(768, 1280, "3:5", true),
    ts(384, 640, "3:5", false),
    ts(1280, 768, "5:3", true),
    ts(640, 384, "5:3", false),
    ts(896, 1152, "7:9", true),
    ts(448, 576, "7:9", false),
    ts(1152, 896, "9:7", true),
    ts(576, 448, "9:7", false),
    ts(768, 1408, "6:11", true),
    ts(384, 704, "6:11", false),
    ts(1408, 768, "11:6", true),
    ts(704, 384, "11:6", false),
    ts(640, 1408, "5:11", true),
    ts(320, 704, "5:11", false),
    ts(1408, 640, "11:5", true),
    ts(704, 320, "11:5", false),
    ts(1152, 640, "9:5", true),
    ts(1173, 640, "16:9", true),
];

/// `'16:9'.split(':').map(Number)`; unparsable parts are `NaN` like `Number()`.
fn split_ratio(ratio: &str) -> (f64, f64) {
    let mut parts = ratio.split(':').map(|p| {
        let p = p.trim();
        if p.is_empty() {
            0.0
        } else {
            p.parse::<f64>().unwrap_or(f64::NAN)
        }
    });
    let w = parts.next().unwrap_or(f64::NAN);
    let h = parts.next().unwrap_or(f64::NAN);
    (w, h)
}

fn ratio_value(ratio: &str) -> f64 {
    let (w, h) = split_ratio(ratio);
    w / h
}

/// `findClosestTitanSize(aspectRatio)`: the allowed size with the nearest aspect ratio; on a tie
/// a later 1024-tier entry replaces the current pick.
pub fn find_closest_titan_size(aspect_ratio: Option<&str>) -> TitanSize {
    let Some(ratio) = aspect_ratio else {
        return TITAN_ALLOWED_SIZES[0];
    };
    let target = ratio_value(ratio);
    let mut closest = TITAN_ALLOWED_SIZES[0];
    for current in &TITAN_ALLOWED_SIZES[1..] {
        let current_diff = (ratio_value(current.aspect_ratio) - target).abs();
        let prev_diff = (ratio_value(closest.aspect_ratio) - target).abs();
        if current_diff < prev_diff || (current_diff == prev_diff && current.large) {
            closest = *current;
        }
    }
    closest
}

/// `getImageDimensions(aspectRatio, modelType)`. Non-finite results (unparsable ratio) are `NaN`
/// in the TS and serialize as `null`; here they are `None`.
pub fn image_dimensions(
    aspect_ratio: Option<&str>,
    model_type: ImageModelType,
) -> (Option<i64>, Option<i64>) {
    if model_type == ImageModelType::Titan {
        let s = find_closest_titan_size(aspect_ratio);
        return (Some(i64::from(s.width)), Some(i64::from(s.height)));
    }
    let Some(ratio) = aspect_ratio else {
        return (Some(1024), Some(1024));
    };
    let (w, h) = split_ratio(ratio);
    let base = 1024.0;
    let to_int = |v: f64| v.is_finite().then(|| js_round(v) as i64);
    if w > h {
        (Some(1024), to_int(base * h / w))
    } else {
        (to_int(base * w / h), Some(1024))
    }
}

/// `Math.round` (halves round toward +infinity).
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

fn int_or_null(v: Option<i64>) -> Value {
    v.map_or(Value::Null, Value::from)
}

/// `buildRequestBody(modelType, request)`. `random_seed` supplies
/// `Math.floor(Math.random() * 2147483647)` for Nova/Titan when no (truthy) seed was given.
pub fn build_image_request_body(
    model_type: ImageModelType,
    req: &GenerateImageRequest,
    random_seed: impl FnOnce() -> i64,
) -> Value {
    let output_format = req.output_format.clone().unwrap_or_else(|| "png".into());
    let (width, height) = image_dimensions(req.aspect_ratio.as_deref(), model_type);
    if model_type.is_stability() {
        // JSON.stringify drops the undefined members.
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
        return Value::Object(body);
    }
    let mut text_params = Map::new();
    text_params.insert("text".into(), json!(req.prompt));
    if let Some(n) = req.negative_prompt.as_deref().filter(|n| !n.is_empty()) {
        text_params.insert("negativeText".into(), json!(n));
    }
    let seed = match req.seed {
        Some(s) if s != 0 => s,
        _ => random_seed(),
    };
    let mut config = Map::new();
    config.insert("numberOfImages".into(), json!(1));
    config.insert("height".into(), int_or_null(height));
    config.insert("width".into(), int_or_null(width));
    if model_type == ImageModelType::Nova {
        config.insert("quality".into(), json!("standard"));
    }
    config.insert("cfgScale".into(), json!(8));
    config.insert("seed".into(), json!(seed));
    json!({
        "taskType": "TEXT_IMAGE",
        "textToImageParams": Value::Object(text_params),
        "imageGenerationConfig": Value::Object(config),
    })
}

/// Response-body handling of `generateImage`.
pub fn parse_image_response(model_type: ImageModelType, body: &[u8]) -> Result<GeneratedImage> {
    let result: Value = serde_json::from_slice(body).map_err(|e| Error::Custom {
        name: "SyntaxError".into(),
        message: e.to_string(),
        fields: Map::new(),
    })?;
    let images = result.get("images").and_then(Value::as_array);
    let strings = |items: &Vec<Value>| {
        items
            .iter()
            .map(|v| v.as_str().map(str::to_string).unwrap_or_default())
            .collect::<Vec<_>>()
    };
    let error = result
        .get("error")
        .filter(|e| crate::sdk::truthy(Some(e)))
        .map(|e| {
            e.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| e.to_string())
        });
    match model_type {
        t if t.is_stability() => {
            let images =
                images.ok_or_else(|| Error::plain("Invalid response format from Bedrock"))?;
            let list = |key: &str| result.get(key).and_then(Value::as_array).cloned();
            Ok(GeneratedImage {
                images: strings(images),
                seeds: list("seeds"),
                finish_reasons: list("finish_reasons"),
            })
        }
        ImageModelType::Nova => {
            if let Some(e) = error {
                return Err(Error::plain(format!("Nova error: {e}")));
            }
            let images = images.ok_or_else(|| Error::plain("Invalid response format from Nova"))?;
            Ok(GeneratedImage {
                images: strings(images),
                ..Default::default()
            })
        }
        _ => {
            if let Some(e) = error {
                return Err(Error::plain(format!("Titan error: {e}")));
            }
            let images =
                images.ok_or_else(|| Error::plain("Invalid response format from Titan"))?;
            Ok(GeneratedImage {
                images: strings(images),
                ..Default::default()
            })
        }
    }
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
    let body = build_image_request_body(model_type, req, || random_index(2_147_483_647) as i64);
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
        parse_image_response(model_type, out.body.as_ref())
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
            image_model_type("stability.stable-image-core-v1:1").unwrap(),
            Core
        );
        assert_eq!(
            image_model_type("stability.stable-image-ultra-v1:0").unwrap(),
            Ultra
        );
        assert_eq!(image_model_type("stability.sd3-5-large-v1:0").unwrap(), Sd3);
        assert_eq!(image_model_type("amazon.nova-canvas-v1:0").unwrap(), Nova);
        assert_eq!(
            image_model_type("amazon.titan-image-generator-v1").unwrap(),
            Titan
        );
        assert_eq!(
            image_model_type("x").unwrap_err().to_string(),
            "Unknown model type for modelId: x"
        );
        assert!(is_image_model_supported(
            "amazon.titan-image-generator-v2:0"
        ));
        assert!(!is_image_model_supported("amazon.nova-reel-v1:0"));
    }

    #[test]
    fn titan_sizes() {
        assert_eq!(find_closest_titan_size(None), TITAN_ALLOWED_SIZES[0]);
        // Exact ratio beats 9:5.
        let s = find_closest_titan_size(Some("16:9"));
        assert_eq!((s.width, s.height), (1173, 640));
        // Ties keep the first 1024-tier entry of the closest ratio.
        let s = find_closest_titan_size(Some("1:1"));
        assert_eq!((s.width, s.height), (1024, 1024));
        let s = find_closest_titan_size(Some("4:5"));
        assert_eq!((s.width, s.height), (896, 1152));
        let s = find_closest_titan_size(Some("3:2"));
        assert_eq!((s.width, s.height), (1152, 768));
        let s = find_closest_titan_size(Some("9:21"));
        assert_eq!((s.width, s.height), (640, 1408));
    }

    #[test]
    fn stability_and_nova_dimensions() {
        use ImageModelType::*;
        assert_eq!(image_dimensions(None, Nova), (Some(1024), Some(1024)));
        assert_eq!(
            image_dimensions(Some("16:9"), Nova),
            (Some(1024), Some(576))
        );
        assert_eq!(
            image_dimensions(Some("9:16"), Core),
            (Some(576), Some(1024))
        );
        assert_eq!(
            image_dimensions(Some("1:1"), Nova),
            (Some(1024), Some(1024))
        );
        assert_eq!(image_dimensions(Some("2:3"), Nova), (Some(683), Some(1024)));
        assert_eq!(image_dimensions(Some("x:y"), Nova), (None, Some(1024)));
    }

    #[test]
    fn stability_bodies_drop_undefined_members() {
        let body = build_image_request_body(
            ImageModelType::Core,
            &req("stability.stable-image-core-v1:1"),
            || unreachable!(),
        );
        assert_eq!(body, json!({ "prompt": "a cat", "output_format": "png" }));

        let mut r = req("stability.sd3-large-v1:0");
        r.negative_prompt = Some("blurry".into());
        r.aspect_ratio = Some("16:9".into());
        r.seed = Some(0);
        r.output_format = Some("jpeg".into());
        let body = build_image_request_body(ImageModelType::Sd3, &r, || unreachable!());
        assert_eq!(
            body,
            json!({
                "prompt": "a cat", "negative_prompt": "blurry", "aspect_ratio": "16:9",
                "seed": 0, "mode": "text-to-image", "output_format": "jpeg"
            })
        );
    }

    #[test]
    fn nova_and_titan_bodies() {
        let mut r = req("amazon.nova-canvas-v1:0");
        r.aspect_ratio = Some("16:9".into());
        r.negative_prompt = Some(String::new());
        let body = build_image_request_body(ImageModelType::Nova, &r, || 42);
        assert_eq!(
            body,
            json!({
                "taskType": "TEXT_IMAGE",
                "textToImageParams": { "text": "a cat" },
                "imageGenerationConfig": {
                    "numberOfImages": 1, "height": 576, "width": 1024, "quality": "standard",
                    "cfgScale": 8, "seed": 42
                }
            })
        );

        let mut r = req("amazon.titan-image-generator-v2:0");
        r.negative_prompt = Some("ugly".into());
        r.seed = Some(7);
        let body = build_image_request_body(ImageModelType::Titan, &r, || unreachable!());
        assert_eq!(
            body,
            json!({
                "taskType": "TEXT_IMAGE",
                "textToImageParams": { "text": "a cat", "negativeText": "ugly" },
                "imageGenerationConfig": {
                    "numberOfImages": 1, "height": 1024, "width": 1024, "cfgScale": 8, "seed": 7
                }
            })
        );
    }

    #[test]
    fn response_parsing() {
        use ImageModelType::*;
        let ok = parse_image_response(
            Core,
            br#"{"images":["AAA"],"seeds":[1],"finish_reasons":[null]}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&ok).unwrap(),
            json!({ "images": ["AAA"], "seeds": [1], "finish_reasons": [null] })
        );
        assert_eq!(
            parse_image_response(Ultra, br#"{}"#)
                .unwrap_err()
                .to_string(),
            "Invalid response format from Bedrock"
        );
        assert_eq!(
            parse_image_response(Nova, br#"{"images":[],"error":"bad prompt"}"#)
                .unwrap_err()
                .to_string(),
            "Nova error: bad prompt"
        );
        assert_eq!(
            parse_image_response(Nova, br#"{}"#)
                .unwrap_err()
                .to_string(),
            "Invalid response format from Nova"
        );
        assert_eq!(
            parse_image_response(Titan, br#"{"error":"x"}"#)
                .unwrap_err()
                .to_string(),
            "Titan error: x"
        );
        let t = parse_image_response(Titan, br#"{"images":["B"],"error":null}"#).unwrap();
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            json!({ "images": ["B"] })
        );
        assert_eq!(
            parse_image_response(Titan, b"nope").unwrap_err().name(),
            "SyntaxError"
        );
    }

    #[test]
    fn request_json_uses_ts_keys() {
        let r: GenerateImageRequest = serde_json::from_value(json!({
            "modelId": "amazon.nova-canvas-v1:0", "prompt": "p", "negativePrompt": "n",
            "aspect_ratio": "1:1", "seed": 3, "output_format": "png"
        }))
        .unwrap();
        assert_eq!(r.aspect_ratio.as_deref(), Some("1:1"));
        assert_eq!(r.negative_prompt.as_deref(), Some("n"));
        assert_eq!(r.output_format.as_deref(), Some("png"));
    }
}
