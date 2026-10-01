//! Image recognition — port of `src/main/api/bedrock/services/imageRecognitionService.ts`
//! (`bedrock:recognizeImage` IPC handler, used by the `recognizeImage` tool).
//!
//! Nova models go through [`ConverseService::converse`] (so store inference params, thinking
//! and guardrail settings apply exactly as in the TS); everything else is treated as Claude and
//! called with `InvokeModel` and an Anthropic Messages body.

use crate::converse::ConverseService;
use crate::error::{Error, Result};
use crate::request::ConverseRequest;
use crate::sdk::SdkConfigSource;
use crate::settings::ConverseSettings;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Default prompt when none is given.
pub const DEFAULT_RECOGNITION_PROMPT: &str = "Please explain in detail what this image is about.";
/// Default model when none is given.
pub const DEFAULT_RECOGNITION_MODEL: &str = "amazon.nova-lite-v1:0";
/// System prompt used for both model families (kept verbatim from the TS).
pub const RECOGNITION_SYSTEM_PROMPT: &str =
    "あなたは画像認識を行うアシスタントです。提供された画像を詳細に分析し、説明してください。";
/// 3.75 MB, the Claude/Nova image size limit.
pub const MAX_IMAGE_BYTES: u64 = 3_932_160;

const SUPPORTED_EXTENSIONS: [&str; 5] = [".jpg", ".jpeg", ".png", ".gif", ".webp"];

/// `recognizeImage(props)` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeImageRequest {
    pub image_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

impl RecognizeImageRequest {
    /// The `bedrock:recognizeImage` IPC params `{ imagePaths, prompt?, modelId? }`: like the TS
    /// handler, only `imagePaths[0]` is recognized.
    pub fn from_ipc(params: &Value) -> Result<Self> {
        let image_path = params
            .get("imagePaths")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .ok_or_else(|| Error::InvalidRequest("imagePaths[0] is required".into()))?;
        let opt = |k: &str| params.get(k).and_then(Value::as_str).map(str::to_string);
        Ok(Self {
            image_path: image_path.to_string(),
            prompt: opt("prompt"),
            model_id: opt("modelId"),
        })
    }
}

/// `path.extname(p).toLowerCase()`: the last `.ext` of the file name, `""` for dotfiles and
/// names without a dot.
pub fn extname_lower(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

/// Media type for a (lower-case) extension; `application/octet-stream` when unknown.
pub fn media_type_for_extension(ext: &str) -> &'static str {
    match ext {
        ".jpg" | ".jpeg" => "image/jpeg",
        ".png" => "image/png",
        ".gif" => "image/gif",
        ".webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// `isNovaModel(modelId)`: strip a two-letter region prefix, then look for `amazon.nova`.
pub fn is_nova_model(model_id: &str) -> bool {
    let b = model_id.as_bytes();
    let base =
        if b.len() >= 3 && b[0].is_ascii_lowercase() && b[1].is_ascii_lowercase() && b[2] == b'.' {
            &model_id[3..]
        } else {
            model_id
        };
    base.contains("amazon.nova")
}

/// `validateImageFile(imagePath)`: exists, supported extension, at most [`MAX_IMAGE_BYTES`].
pub fn validate_image_file(image_path: &str) -> Result<()> {
    let path = Path::new(image_path);
    if !path.exists() {
        return Err(Error::plain(format!("Image file not found: {image_path}")));
    }
    let ext = extname_lower(image_path);
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(Error::plain(format!("Unsupported image format: {ext}")));
    }
    let size = std::fs::metadata(path)
        .map_err(|e| Error::plain(e.to_string()))?
        .len();
    if size > MAX_IMAGE_BYTES {
        return Err(Error::plain(format!(
            "Image file too large: {size} bytes (max: {MAX_IMAGE_BYTES} bytes)"
        )));
    }
    Ok(())
}

/// The Anthropic Messages body sent to Claude models.
pub fn claude_request_body(base64_data: &str, media_type: &str, prompt: &str) -> Value {
    json!({
        "anthropic_version": "bedrock-2023-05-31",
        "max_tokens": 4096,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "image", "source": { "type": "base64", "media_type": media_type, "data": base64_data } },
                { "type": "text", "text": prompt }
            ]
        }],
        "system": RECOGNITION_SYSTEM_PROMPT
    })
}

/// Text of a Claude Messages response: `text` items joined by `\n`, or a string `content`.
pub fn claude_response_text(result: &Value) -> Result<String> {
    let description = match result.get("content") {
        Some(Value::Array(items)) => items
            .iter()
            .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
            .map(|i| i.get("text").and_then(Value::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    if description.is_empty() {
        return Err(Error::plain("No text content in response"));
    }
    Ok(description)
}

/// Text of a Converse response (`output.message.content[].text` joined by `\n`).
pub fn converse_response_text(response: &Value) -> Result<String> {
    let description = response
        .pointer("/output/message/content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("text").and_then(Value::as_str))
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if description.is_empty() {
        return Err(Error::plain("No text content in Nova response"));
    }
    Ok(description)
}

/// The Converse request used for Nova models.
pub fn nova_converse_request(
    model_id: &str,
    base64_data: &str,
    media_type: &str,
    prompt: &str,
) -> ConverseRequest {
    let format = media_type.split('/').nth(1).unwrap_or(media_type);
    ConverseRequest {
        model_id: model_id.to_string(),
        messages: vec![json!({
            "role": "user",
            "content": [
                { "image": { "format": format, "source": { "bytes": base64_data } } },
                { "text": prompt }
            ]
        })],
        system: Some(vec![json!({ "text": RECOGNITION_SYSTEM_PROMPT })]),
        ..Default::default()
    }
}

/// `recognizeImage(props)`: the model's description of the image.
///
/// `converse` should share `configs` (see [`crate::sdk::SdkConfigClients`]).
pub async fn recognize_image(
    configs: &dyn SdkConfigSource,
    converse: &ConverseService,
    settings: &ConverseSettings,
    req: &RecognizeImageRequest,
) -> Result<String> {
    let prompt = req.prompt.as_deref().unwrap_or(DEFAULT_RECOGNITION_PROMPT);
    let model_id = req.model_id.as_deref().unwrap_or(DEFAULT_RECOGNITION_MODEL);
    let result = async {
        validate_image_file(&req.image_path)?;
        let bytes = std::fs::read(&req.image_path).map_err(|e| Error::plain(e.to_string()))?;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(bytes);
        let media_type = media_type_for_extension(&extname_lower(&req.image_path));
        if is_nova_model(model_id) {
            tracing::info!(model_id, "Using Nova model for image recognition");
            let request = nova_converse_request(model_id, &base64_data, media_type, prompt);
            let response = converse
                .converse(settings, &request, &CancellationToken::new())
                .await?;
            converse_response_text(&response)
        } else {
            tracing::info!(model_id, "Using Claude model for image recognition");
            let conf = configs.sdk_config(&settings.aws).await?;
            let client = aws_sdk_bedrockruntime::Client::new(&conf);
            let body = claude_request_body(&base64_data, media_type, prompt);
            let out = client
                .invoke_model()
                .model_id(model_id)
                .content_type("application/json")
                .accept("application/json")
                .body(aws_smithy_types::Blob::new(
                    serde_json::to_vec(&body).unwrap_or_default(),
                ))
                .send()
                .await
                .map_err(Error::from)?;
            let result: Value =
                serde_json::from_slice(out.body.as_ref()).map_err(|e| Error::Custom {
                    name: "SyntaxError".into(),
                    message: e.to_string(),
                    fields: Default::default(),
                })?;
            claude_response_text(&result)
        }
    }
    .await;
    if let Err(e) = &result {
        tracing::error!(image_path = %req.image_path, model_id, error = %e, "Error in image recognition");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extname_matches_node() {
        assert_eq!(extname_lower("/a/b/Photo.JPG"), ".jpg");
        assert_eq!(extname_lower("/a/b.dir/file"), "");
        assert_eq!(extname_lower("/a/.hidden"), "");
        assert_eq!(extname_lower("x.tar.gz"), ".gz");
        assert_eq!(media_type_for_extension(".jpeg"), "image/jpeg");
        assert_eq!(media_type_for_extension(".bmp"), "application/octet-stream");
    }

    #[test]
    fn nova_detection() {
        assert!(is_nova_model("amazon.nova-lite-v1:0"));
        assert!(is_nova_model("us.amazon.nova-pro-v1:0"));
        assert!(is_nova_model("apac.amazon.nova-pro-v1:0"));
        assert!(!is_nova_model("us.anthropic.claude-sonnet-4-20250514-v1:0"));
    }

    #[test]
    fn file_validation() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.png");
        let missing = missing.to_str().unwrap();
        assert_eq!(
            validate_image_file(missing).unwrap_err().to_string(),
            format!("Image file not found: {missing}")
        );
        let bmp = dir.path().join("a.bmp");
        std::fs::write(&bmp, b"x").unwrap();
        assert_eq!(
            validate_image_file(bmp.to_str().unwrap())
                .unwrap_err()
                .to_string(),
            "Unsupported image format: .bmp"
        );
        let big = dir.path().join("big.PNG");
        let mut f = std::fs::File::create(&big).unwrap();
        f.write_all(&vec![0u8; (MAX_IMAGE_BYTES + 1) as usize])
            .unwrap();
        assert_eq!(
            validate_image_file(big.to_str().unwrap())
                .unwrap_err()
                .to_string(),
            format!(
                "Image file too large: {} bytes (max: 3932160 bytes)",
                MAX_IMAGE_BYTES + 1
            )
        );
        let ok = dir.path().join("ok.webp");
        std::fs::write(&ok, b"x").unwrap();
        validate_image_file(ok.to_str().unwrap()).unwrap();
    }

    #[test]
    fn response_text_extraction() {
        assert_eq!(
            claude_response_text(&json!({ "content": [
                { "type": "text", "text": "a" }, { "type": "tool_use" }, { "type": "text", "text": "b" }
            ] }))
            .unwrap(),
            "a\nb"
        );
        assert_eq!(
            claude_response_text(&json!({ "content": "plain" })).unwrap(),
            "plain"
        );
        assert_eq!(
            claude_response_text(&json!({})).unwrap_err().to_string(),
            "No text content in response"
        );
        assert_eq!(
            converse_response_text(&json!({ "output": { "message": { "content": [
                { "text": "x" }, { "reasoningContent": {} }, { "text": "y" }
            ] } } }))
            .unwrap(),
            "x\ny"
        );
        assert_eq!(
            converse_response_text(&json!({ "output": {} }))
                .unwrap_err()
                .to_string(),
            "No text content in Nova response"
        );
    }

    #[test]
    fn request_shapes() {
        let body = claude_request_body("QUJD", "image/png", "what?");
        assert_eq!(
            body["messages"][0]["content"][0]["source"]["media_type"],
            "image/png"
        );
        assert_eq!(body["messages"][0]["content"][1]["text"], "what?");
        assert_eq!(body["max_tokens"], 4096);
        let r = nova_converse_request("amazon.nova-lite-v1:0", "QUJD", "image/jpeg", "p");
        assert_eq!(r.messages[0]["content"][0]["image"]["format"], "jpeg");
        assert_eq!(
            r.messages[0]["content"][0]["image"]["source"]["bytes"],
            "QUJD"
        );
        assert_eq!(r.system.unwrap()[0]["text"], RECOGNITION_SYSTEM_PROMPT);
    }

    #[test]
    fn ipc_params_use_first_path() {
        let r = RecognizeImageRequest::from_ipc(
            &json!({ "imagePaths": ["/a.png", "/b.png"], "prompt": "p" }),
        )
        .unwrap();
        assert_eq!(r.image_path, "/a.png");
        assert_eq!(r.prompt.as_deref(), Some("p"));
        assert_eq!(r.model_id, None);
        assert!(RecognizeImageRequest::from_ipc(&json!({ "imagePaths": [] })).is_err());
    }
}
