//! Video generation (Amazon Nova Reel) — port of `src/main/api/bedrock/services/movieService.ts`
//! and `src/main/api/bedrock/types/movie.ts`, behind the `bedrock:generateVideo`,
//! `bedrock:startVideoGeneration`, `bedrock:checkVideoStatus` and `bedrock:downloadVideo` IPC
//! handlers (and the `generateVideo` / `checkVideoStatus` / `downloadVideo` tools).
//!
//! * [`start_video_generation`] — `StartAsyncInvoke` with the same `modelInput` bodies
//!   (`TEXT_VIDEO`, `MULTI_SHOT_AUTOMATED`, `MULTI_SHOT_MANUAL` with images uploaded to
//!   `s3://<bucket>/temp-images/<ms>/<file>`), the `MULTI_SHOT_MANUAL` → `MULTI_SHOT_AUTOMATED`
//!   fallback and the same error rewording.
//! * [`get_job_status`] — `GetAsyncInvoke`.
//! * [`download_video_from_s3`] — S3 `GetObject` streamed to a local file.
//! * [`generate_video`] — start, poll every 30 s for up to 30 min, then download.
//!
//! Timestamps (`submitTime`, `endTime`) are ISO strings (`Date` objects in the TS results, which
//! the IPC structured clone kept as `Date`s).
//!
//! Differences from the TS: the region (and so the model id) and clients come from the settings
//! passed to each call instead of the store at startup; I/O error text is Rust's, not Node's.
//! Fixed vs the TS: [`generate_video`] downloads `<prefix>/output.mp4` (where Nova Reel writes
//! the video; see [`output_video_uri`]) instead of the job's output *prefix* URI, which is not
//! an object and failed with `NoSuchKey`. The `downloadVideo` tool uses the same helper.

use crate::document::json_to_document;
use crate::error::{Error, Result};
use crate::retry::random_index;
use crate::sdk::{iso_millis, now_iso, SdkConfigSource};
use crate::settings::AwsSettings;
use aws_sdk_bedrockruntime::types::{AsyncInvokeOutputDataConfig, AsyncInvokeS3OutputDataConfig};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// `NOVA_REEL_V1_1`.
pub const NOVA_REEL_V1_1: &str = "amazon.nova-reel-v1:1";
/// `NOVA_REEL_V1_0`.
pub const NOVA_REEL_V1_0: &str = "amazon.nova-reel-v1:0";
/// `NOVA_REEL_RESOLUTION`.
pub const NOVA_REEL_RESOLUTION: &str = "1280x720";
/// `NOVA_REEL_FPS`.
pub const NOVA_REEL_FPS: u32 = 24;

/// `NOVA_REEL_REGION_SUPPORT` (best model first).
pub const NOVA_REEL_REGION_SUPPORT: [(&str, &[&str]); 3] = [
    ("us-east-1", &[NOVA_REEL_V1_1, NOVA_REEL_V1_0]),
    ("eu-west-1", &[NOVA_REEL_V1_0]),
    ("ap-northeast-1", &[NOVA_REEL_V1_0]),
];

/// `VALID_DURATIONS`.
pub const VALID_DURATIONS: [u32; 20] = [
    6, 12, 18, 24, 30, 36, 42, 48, 54, 60, 66, 72, 78, 84, 90, 96, 102, 108, 114, 120,
];

/// Default `maxWaitTime` of `waitForCompletion` (30 minutes).
pub const DEFAULT_MAX_WAIT: Duration = Duration::from_secs(30 * 60);
/// Default `pollInterval` of `waitForCompletion` (30 seconds).
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// `getNovaReelSupportedRegions()`.
pub fn nova_reel_supported_regions() -> Vec<&'static str> {
    NOVA_REEL_REGION_SUPPORT.iter().map(|(r, _)| *r).collect()
}

/// `isNovaReelSupportedInRegion(region)`.
pub fn is_nova_reel_supported_in_region(region: &str) -> bool {
    NOVA_REEL_REGION_SUPPORT
        .iter()
        .any(|(r, models)| *r == region && !models.is_empty())
}

/// `getNovaReelModelId(region)`.
pub fn nova_reel_model_id(region: &str) -> Result<&'static str> {
    NOVA_REEL_REGION_SUPPORT
        .iter()
        .find(|(r, _)| *r == region)
        .and_then(|(_, models)| models.first().copied())
        .ok_or_else(|| {
            Error::plain(format!(
                "Nova Reel is not available in region {region}. Supported regions: {}",
                nova_reel_supported_regions().join(", ")
            ))
        })
}

/// `isNovaReelV1_0(modelId)`.
pub fn is_nova_reel_v1_0(model_id: &str) -> bool {
    model_id == NOVA_REEL_V1_0
}

/// `isNovaReelV1_1(modelId)`.
pub fn is_nova_reel_v1_1(model_id: &str) -> bool {
    model_id == NOVA_REEL_V1_1
}

/// `isValidDuration(duration)`.
pub fn is_valid_duration(duration: f64) -> bool {
    VALID_DURATIONS.iter().any(|d| f64::from(*d) == duration)
}

/// `getTaskTypeForDuration(duration)`.
pub fn task_type_for_duration(duration: f64) -> &'static str {
    if duration == 6.0 {
        "TEXT_VIDEO"
    } else {
        "MULTI_SHOT_AUTOMATED"
    }
}

/// `getTaskTypeForRequest(duration, hasInputImages)`.
pub fn task_type_for_request(duration: f64, has_input_images: bool) -> &'static str {
    match (duration == 6.0, has_input_images) {
        (true, _) => "TEXT_VIDEO",
        (false, true) => "MULTI_SHOT_MANUAL",
        (false, false) => "MULTI_SHOT_AUTOMATED",
    }
}

/// `GenerateMovieRequest` (the `bedrock:startVideoGeneration` params).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMovieRequest {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub duration_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<f64>,
    #[serde(default)]
    pub s3_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_images: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<String>>,
}

/// `AsyncInvocationStatus` (`bedrock:checkVideoStatus` result).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncInvocationStatus {
    pub invocation_arn: String,
    pub model_id: String,
    /// `InProgress` | `Completed` | `Failed`.
    pub status: String,
    /// ISO timestamp.
    pub submit_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    /// `{ s3OutputDataConfig: { s3Uri, kmsKeyId?, bucketOwner? } }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_data_config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
}

impl AsyncInvocationStatus {
    /// `outputDataConfig?.s3OutputDataConfig?.s3Uri`.
    pub fn s3_uri(&self) -> Option<&str> {
        self.output_data_config
            .as_ref()?
            .pointer("/s3OutputDataConfig/s3Uri")?
            .as_str()
    }

    /// The URI of the finished video: `<output prefix>/output.mp4` (see [`output_video_uri`]).
    pub fn output_video_uri(&self) -> Option<String> {
        self.s3_uri().map(output_video_uri)
    }
}

/// Nova Reel writes the video as `output.mp4` under the job's output prefix:
/// `s3://b/videos/abc` (or `s3://b/videos/abc/`) → `s3://b/videos/abc/output.mp4`.
pub fn output_video_uri(prefix: &str) -> String {
    format!("{}/output.mp4", prefix.trim_end_matches('/'))
}

/// `GeneratedMovie`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedMovie {
    pub invocation_arn: String,
    pub status: AsyncInvocationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The `bedrock:downloadVideo` result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedVideo {
    pub downloaded_path: String,
    pub file_size: u64,
}

fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `path.toLowerCase().split('.').pop()`.
fn extension(path: &str) -> String {
    path.to_lowercase()
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// `detectImageFormat(imagePath)`.
pub fn detect_image_format(path: &str) -> Result<&'static str> {
    match extension(path).as_str() {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpeg"),
        ext => Err(Error::plain(format!("Unsupported image format: {ext}"))),
    }
}

/// `validateRequest(request)`.
pub fn validate_request(req: &GenerateMovieRequest) -> Result<()> {
    if req.prompt.is_empty() {
        return Err(Error::plain("Prompt is required and cannot be empty"));
    }
    if js_len(&req.prompt) > 4000 {
        return Err(Error::plain("Prompt must be 4000 characters or less"));
    }
    if !is_valid_duration(req.duration_seconds) {
        return Err(Error::plain(format!(
            "Duration must be 6 seconds (TEXT_VIDEO) or a multiple of 6 between 12-120 seconds (MULTI_SHOT_AUTOMATED). Valid values: {}",
            VALID_DURATIONS.map(|d| d.to_string()).join(", ")
        )));
    }
    if req.s3_uri.is_empty() {
        return Err(Error::plain("S3 URI is required for video generation"));
    }
    if !req.s3_uri.starts_with("s3://") {
        return Err(Error::plain("S3 URI must start with s3://"));
    }
    if let Some(seed) = req.seed {
        if !(0.0..=2_147_483_646.0).contains(&seed) {
            return Err(Error::plain("Seed must be between 0 and 2147483646"));
        }
    }
    if let Some(images) = req.input_images.as_ref().filter(|i| !i.is_empty()) {
        if req.duration_seconds == 6.0 && images.len() > 1 {
            return Err(Error::plain(
                "TEXT_VIDEO mode (6 seconds) supports only a single input image",
            ));
        }
        for (i, path) in images.iter().enumerate() {
            if !matches!(extension(path).as_str(), "png" | "jpg" | "jpeg") {
                return Err(Error::plain(format!(
                    "Image {} must be PNG or JPEG format: {path}",
                    i + 1
                )));
            }
        }
        if let Some(prompts) = &req.prompts {
            if prompts.len() != images.len() {
                return Err(Error::plain(format!(
                    "Number of prompts ({}) must match number of images ({})",
                    prompts.len(),
                    images.len()
                )));
            }
            for (i, p) in prompts.iter().enumerate() {
                if p.trim().is_empty() {
                    return Err(Error::plain(format!("Prompt {} cannot be empty", i + 1)));
                }
                if js_len(p) > 4000 {
                    return Err(Error::plain(format!(
                        "Prompt {} must be 4000 characters or less",
                        i + 1
                    )));
                }
            }
        }
    }
    if req.prompts.is_some() && req.input_images.is_none() {
        return Err(Error::plain(
            "Prompts can only be used together with input images",
        ));
    }
    Ok(())
}

/// A JSON number, integral when the value is (`6` rather than `6.0`).
fn num(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

/// `request.seed || Math.floor(Math.random() * 2147483647)`.
fn seed_or_random(seed: Option<f64>, random_seed: &mut (dyn FnMut() -> i64 + Send)) -> Value {
    match seed {
        Some(s) if s != 0.0 && !s.is_nan() => num(s),
        _ => json!(random_seed()),
    }
}

/// `readImageAsBase64(imagePath)`.
async fn read_image_base64(path: &str) -> Result<String> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| Error::plain(format!("Failed to read image file {path}: Error: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(data))
}

/// `textToVideoParams` with the single image as base64 bytes when there is exactly one.
async fn text_to_video_params(req: &GenerateMovieRequest) -> Result<Value> {
    let mut params = Map::new();
    params.insert("text".into(), json!(req.prompt));
    if let Some([path]) = req.input_images.as_deref() {
        let bytes = read_image_base64(path).await?;
        params.insert(
            "images".into(),
            json!([{ "format": detect_image_format(path)?, "source": { "bytes": bytes } }]),
        );
    }
    Ok(Value::Object(params))
}

/// `uploadImageToS3(imagePath, s3BaseUri)`: `s3://<bucket>/temp-images/<Date.now()>/<basename>`.
async fn upload_image_to_s3(
    s3: &aws_sdk_s3::Client,
    image_path: &str,
    s3_base_uri: &str,
) -> Result<String> {
    let result = async {
        let bucket = s3_base_uri
            .strip_prefix("s3://")
            .map(|rest| rest.split('/').next().unwrap_or_default())
            .filter(|b| !b.is_empty())
            .ok_or_else(|| format!("Error: Invalid S3 URI format: {s3_base_uri}"))?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let file_name = Path::new(image_path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let key = format!("temp-images/{timestamp}/{file_name}");
        let data = tokio::fs::read(image_path)
            .await
            .map_err(|e| format!("Error: {e}"))?;
        let format = detect_image_format(image_path).map_err(|e| format!("Error: {e}"))?;
        let content_type = if format == "png" {
            "image/png"
        } else {
            "image/jpeg"
        };
        s3.put_object()
            .bucket(bucket)
            .key(&key)
            .body(data.into())
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| js_to_string(&Error::from(e)))?;
        Ok::<_, String>(format!("s3://{bucket}/{key}"))
    }
    .await;
    result.map_err(|e| Error::plain(format!("Failed to upload image to S3: {e}")))
}

/// `buildNovaReelRequest(request)`. `random_seed` supplies
/// `Math.floor(Math.random() * 2147483647)` when no (truthy) seed was given; `s3` uploads the
/// `MULTI_SHOT_MANUAL` shot images.
pub async fn build_nova_reel_request(
    s3: &aws_sdk_s3::Client,
    model_id: &str,
    req: &GenerateMovieRequest,
    random_seed: &mut (dyn FnMut() -> i64 + Send),
) -> Result<Value> {
    let has_input_images = req.input_images.as_ref().is_some_and(|i| !i.is_empty());
    let duration = num(req.duration_seconds);
    // v1.0 supports TEXT_VIDEO only.
    let task_type = if is_nova_reel_v1_0(model_id) {
        "TEXT_VIDEO"
    } else {
        task_type_for_request(req.duration_seconds, has_input_images)
    };
    match task_type {
        "TEXT_VIDEO" => Ok(json!({
            "taskType": "TEXT_VIDEO",
            "textToVideoParams": text_to_video_params(req).await?,
            "videoGenerationConfig": {
                "durationSeconds": duration,
                "fps": NOVA_REEL_FPS,
                "dimension": NOVA_REEL_RESOLUTION,
                "seed": seed_or_random(req.seed, random_seed)
            }
        })),
        "MULTI_SHOT_MANUAL" => {
            let images = req.input_images.as_deref().unwrap_or_default();
            let mut shots = Vec::with_capacity(images.len());
            for (i, path) in images.iter().enumerate() {
                let text = req
                    .prompts
                    .as_ref()
                    .map(|p| p.get(i).cloned().unwrap_or_default())
                    .unwrap_or_else(|| req.prompt.clone());
                let uri = upload_image_to_s3(s3, path, &req.s3_uri).await?;
                shots.push(json!({
                    "text": text,
                    "image": {
                        "format": detect_image_format(path)?,
                        "source": { "s3Location": { "uri": uri } }
                    }
                }));
            }
            // No durationSeconds: the number of shots decides it.
            Ok(json!({
                "taskType": "MULTI_SHOT_MANUAL",
                "multiShotManualParams": { "shots": shots },
                "videoGenerationConfig": {
                    "fps": NOVA_REEL_FPS,
                    "dimension": NOVA_REEL_RESOLUTION,
                    "seed": seed_or_random(req.seed, random_seed)
                }
            }))
        }
        _ => Ok(json!({
            "taskType": "MULTI_SHOT_AUTOMATED",
            "multiShotAutomatedParams": { "text": req.prompt },
            "videoGenerationConfig": {
                "durationSeconds": duration,
                "fps": NOVA_REEL_FPS,
                "dimension": NOVA_REEL_RESOLUTION,
                "seed": seed_or_random(req.seed, random_seed)
            }
        })),
    }
}

/// `String(error)` for an error: `` `${name}: ${message}` ``.
fn js_to_string(e: &Error) -> String {
    format!("{}: {}", e.name(), e.message())
}

/// `awsCredentials.region || 'us-east-1'`.
fn video_region(aws: &AwsSettings) -> &str {
    if aws.region.is_empty() {
        "us-east-1"
    } else {
        &aws.region
    }
}

fn warn_on_settings(aws: &AwsSettings) {
    if aws.region.is_empty()
        || (!aws.use_profile.unwrap_or(false)
            && (aws.access_key_id.is_empty() || aws.secret_access_key.is_empty()))
    {
        tracing::warn!("AWS credentials not configured properly");
    }
    let region = video_region(aws);
    if !is_nova_reel_supported_in_region(region) {
        tracing::warn!(
            "Nova Reel is not available in region {region}. Supported regions: {}",
            nova_reel_supported_regions().join(", ")
        );
    }
}

fn in_progress(invocation_arn: &str, model_id: &str, s3_uri: &str) -> GeneratedMovie {
    GeneratedMovie {
        invocation_arn: invocation_arn.to_string(),
        status: AsyncInvocationStatus {
            invocation_arn: invocation_arn.to_string(),
            model_id: model_id.to_string(),
            status: "InProgress".into(),
            submit_time: now_iso(),
            output_data_config: Some(json!({ "s3OutputDataConfig": { "s3Uri": s3_uri } })),
            ..Default::default()
        },
        ..Default::default()
    }
}

async fn start_async_invoke(
    client: &aws_sdk_bedrockruntime::Client,
    model_id: &str,
    model_input: &Value,
    s3_uri: &str,
) -> Result<String> {
    let output = AsyncInvokeOutputDataConfig::S3OutputDataConfig(
        AsyncInvokeS3OutputDataConfig::builder()
            .s3_uri(s3_uri)
            .build()
            .map_err(|e| Error::InvalidRequest(e.to_string()))?,
    );
    let out = client
        .start_async_invoke()
        .model_id(model_id)
        .model_input(json_to_document(model_input))
        .output_data_config(output)
        .send()
        .await
        .map_err(Error::from)?;
    if out.invocation_arn.is_empty() {
        return Err(Error::plain(
            "Failed to start video generation: No invocation ARN returned",
        ));
    }
    Ok(out.invocation_arn)
}

/// `startVideoGeneration(request)`.
pub async fn start_video_generation(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    req: &GenerateMovieRequest,
) -> Result<GeneratedMovie> {
    warn_on_settings(aws);
    validate_request(req)?;
    let conf = configs.sdk_config(aws).await?;
    let s3 = aws_sdk_s3::Client::new(&conf);
    let runtime = aws_sdk_bedrockruntime::Client::new(&conf);
    let model_id = nova_reel_model_id(video_region(aws))?;
    let mut random = || random_index(2_147_483_647) as i64;
    let request = build_nova_reel_request(&s3, model_id, req, &mut random).await?;

    tracing::debug!(request = %request, model_id, "Nova Reel request");
    let error = match start_async_invoke(&runtime, model_id, &request, &req.s3_uri).await {
        Ok(arn) => return Ok(in_progress(&arn, model_id, &req.s3_uri)),
        Err(e) => e,
    };
    tracing::error!(error = %error, "Error starting video generation");

    if error.service_name() == Some("ValidationException")
        && request["taskType"] == "MULTI_SHOT_MANUAL"
        && error.message().contains("textToVideoParams")
    {
        tracing::info!("MULTI_SHOT_MANUAL failed, attempting fallback to MULTI_SHOT_AUTOMATED...");
        let combined = req
            .prompts
            .as_ref()
            .map(|p| p.join(". "))
            .unwrap_or_else(|| req.prompt.clone());
        let mut config = request["videoGenerationConfig"].clone();
        if let Some(c) = config.as_object_mut() {
            c.insert("durationSeconds".into(), num(req.duration_seconds));
        }
        let fallback = json!({
            "taskType": "MULTI_SHOT_AUTOMATED",
            "multiShotAutomatedParams": { "text": combined },
            "videoGenerationConfig": config
        });
        return match start_async_invoke(&runtime, model_id, &fallback, &req.s3_uri).await {
            Ok(arn) => {
                tracing::info!(
                    "Fallback succeeded! Note: Images were not used due to API limitations."
                );
                Ok(in_progress(&arn, model_id, &req.s3_uri))
            }
            Err(fallback_error) => Err(Error::plain(format!(
                "Both MULTI_SHOT_MANUAL and fallback failed. Original error: {}. Fallback error: {}",
                error.message(),
                js_to_string(&fallback_error)
            ))),
        };
    }

    Err(match error.service_name() {
        Some("UnrecognizedClientException") => Error::plain(
            "AWS authentication failed. Please check your credentials and permissions.",
        ),
        Some("ValidationException") => {
            Error::plain(format!("Invalid request parameters: {}", error.message()))
        }
        Some("AccessDeniedException") => {
            Error::plain("Access denied. Please ensure you have permissions for Bedrock and S3.")
        }
        Some("ThrottlingException") => {
            Error::plain("Request was throttled. Please try again later.")
        }
        _ => error,
    })
}

/// `getJobStatus(invocationArn)`.
pub async fn get_job_status(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    invocation_arn: &str,
) -> Result<AsyncInvocationStatus> {
    let result = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrockruntime::Client::new(&conf);
        let out = client
            .get_async_invoke()
            .invocation_arn(invocation_arn)
            .send()
            .await
            .map_err(Error::from)?;
        let model_id = nova_reel_model_id(video_region(aws))?;
        let output_data_config = out.output_data_config.as_ref().and_then(|c| match c {
            AsyncInvokeOutputDataConfig::S3OutputDataConfig(s3) => {
                let mut m = Map::new();
                m.insert("s3Uri".into(), json!(s3.s3_uri));
                if let Some(k) = &s3.kms_key_id {
                    m.insert("kmsKeyId".into(), json!(k));
                }
                if let Some(o) = &s3.bucket_owner {
                    m.insert("bucketOwner".into(), json!(o));
                }
                Some(json!({ "s3OutputDataConfig": Value::Object(m) }))
            }
            _ => None,
        });
        Ok(AsyncInvocationStatus {
            invocation_arn: if out.invocation_arn.is_empty() {
                invocation_arn.to_string()
            } else {
                out.invocation_arn.clone()
            },
            model_id: model_id.to_string(),
            status: out.status.as_str().to_string(),
            submit_time: iso_millis(&out.submit_time),
            end_time: out.end_time.as_ref().map(iso_millis),
            output_data_config,
            failure_message: out.failure_message.clone(),
        })
    }
    .await;
    result.inspect_err(|e| tracing::error!(error = %e, "Error getting job status"))
}

/// `s3://bucket/key` → `(bucket, key)` (`/^s3:\/\/([^/]+)\/(.+)$/`).
pub fn parse_s3_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("s3://")?;
    let (bucket, key) = rest.split_once('/')?;
    (!bucket.is_empty() && !key.is_empty()).then_some((bucket, key))
}

/// `downloadVideoFromS3(s3Uri, localPath)`: returns `localPath`.
pub async fn download_video_from_s3(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    s3_uri: &str,
    local_path: &str,
) -> Result<String> {
    let result = async {
        let (bucket, key) = parse_s3_uri(s3_uri)
            .ok_or_else(|| Error::plain(format!("Invalid S3 URI format: {s3_uri}")))?;
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_s3::Client::new(&conf);
        let out = client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(Error::from)?;
        let path = Path::new(local_path);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| Error::plain(format!("{e}, mkdir '{}'", dir.to_string_lossy())))?;
        }
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(|e| Error::plain(format!("{e}, open '{local_path}'")))?;
        let mut body = out.body;
        while let Some(chunk) = body
            .try_next()
            .await
            .map_err(|e| Error::plain(e.to_string()))?
        {
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|e| Error::plain(e.to_string()))?;
        }
        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| Error::plain(e.to_string()))?;
        Ok(local_path.to_string())
    }
    .await;
    result.inspect_err(|e| tracing::error!(error = %e, "Error downloading video from S3"))
}

/// The `bedrock:downloadVideo` handler: download, then `{ downloadedPath, fileSize }`.
pub async fn download_video(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    s3_uri: &str,
    local_path: &str,
) -> Result<DownloadedVideo> {
    let downloaded_path = download_video_from_s3(configs, aws, s3_uri, local_path).await?;
    let meta = tokio::fs::metadata(&downloaded_path)
        .await
        .map_err(|e| Error::plain(format!("{e}, stat '{downloaded_path}'")))?;
    Ok(DownloadedVideo {
        downloaded_path,
        file_size: meta.len(),
    })
}

/// `waitForCompletion(invocationArn, { maxWaitTime, pollInterval, onProgress })`.
pub async fn wait_for_completion(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    invocation_arn: &str,
    max_wait: Duration,
    poll_interval: Duration,
    mut on_progress: impl FnMut(&AsyncInvocationStatus) + Send,
) -> Result<AsyncInvocationStatus> {
    let start = Instant::now();
    while start.elapsed() < max_wait {
        let status = get_job_status(configs, aws, invocation_arn).await?;
        on_progress(&status);
        match status.status.as_str() {
            "Completed" => return Ok(status),
            "Failed" => {
                return Err(Error::plain(format!(
                    "Video generation failed: {}",
                    status
                        .failure_message
                        .as_deref()
                        .filter(|m| !m.is_empty())
                        .unwrap_or("Unknown error")
                )))
            }
            _ => {}
        }
        tokio::time::sleep(poll_interval).await;
    }
    let secs = max_wait.as_secs_f64();
    Err(Error::plain(format!(
        "Video generation timed out after {} seconds",
        num(secs)
    )))
}

/// `generateVideo(request)` with explicit polling settings.
pub async fn generate_video_with(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    req: &GenerateMovieRequest,
    max_wait: Duration,
    poll_interval: Duration,
) -> Result<GeneratedMovie> {
    let initial = start_video_generation(configs, aws, req).await?;
    let status = wait_for_completion(
        configs,
        aws,
        &initial.invocation_arn,
        max_wait,
        poll_interval,
        |s| tracing::info!("Video generation status: {}", s.status),
    )
    .await?;
    let mut local_path = None;
    if status.status == "Completed" {
        if let (Some(uri), Some(out)) = (status.output_video_uri(), req.output_path.as_deref()) {
            let out = if out.ends_with(".mp4") {
                out.to_string()
            } else {
                format!("{out}.mp4")
            };
            local_path = Some(download_video_from_s3(configs, aws, &uri, &out).await?);
        }
    }
    Ok(GeneratedMovie {
        invocation_arn: initial.invocation_arn,
        output_location: status.s3_uri().map(str::to_string),
        local_path,
        error: if status.status == "Failed" {
            status.failure_message.clone()
        } else {
            None
        },
        status,
    })
}

/// `generateVideo(request)`: start, poll every 30 s for up to 30 minutes, download to
/// `outputPath` (`.mp4` appended when missing).
pub async fn generate_video(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    req: &GenerateMovieRequest,
) -> Result<GeneratedMovie> {
    generate_video_with(configs, aws, req, DEFAULT_MAX_WAIT, DEFAULT_POLL_INTERVAL).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(prompt: &str, duration: f64) -> GenerateMovieRequest {
        GenerateMovieRequest {
            prompt: prompt.into(),
            duration_seconds: duration,
            s3_uri: "s3://bucket/videos/".into(),
            ..Default::default()
        }
    }

    fn err(r: &GenerateMovieRequest) -> String {
        validate_request(r).unwrap_err().to_string()
    }

    fn dummy_s3() -> aws_sdk_s3::Client {
        let conf = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .build();
        aws_sdk_s3::Client::from_conf(conf)
    }

    #[test]
    fn region_support_and_model_ids() {
        assert_eq!(nova_reel_model_id("us-east-1").unwrap(), NOVA_REEL_V1_1);
        assert_eq!(nova_reel_model_id("eu-west-1").unwrap(), NOVA_REEL_V1_0);
        assert_eq!(
            nova_reel_model_id("ap-northeast-1").unwrap(),
            NOVA_REEL_V1_0
        );
        assert_eq!(
            nova_reel_model_id("us-west-2").unwrap_err().to_string(),
            "Nova Reel is not available in region us-west-2. Supported regions: us-east-1, eu-west-1, ap-northeast-1"
        );
        assert!(is_nova_reel_supported_in_region("eu-west-1"));
        assert!(!is_nova_reel_supported_in_region("us-west-2"));
        assert!(is_nova_reel_v1_0(NOVA_REEL_V1_0) && !is_nova_reel_v1_0(NOVA_REEL_V1_1));
        assert!(is_nova_reel_v1_1(NOVA_REEL_V1_1));
    }

    #[test]
    fn durations_and_task_types() {
        assert!(is_valid_duration(6.0) && is_valid_duration(120.0) && is_valid_duration(66.0));
        assert!(!is_valid_duration(0.0) && !is_valid_duration(7.0) && !is_valid_duration(126.0));
        assert_eq!(task_type_for_duration(6.0), "TEXT_VIDEO");
        assert_eq!(task_type_for_duration(12.0), "MULTI_SHOT_AUTOMATED");
        assert_eq!(task_type_for_request(6.0, true), "TEXT_VIDEO");
        assert_eq!(task_type_for_request(12.0, true), "MULTI_SHOT_MANUAL");
        assert_eq!(task_type_for_request(12.0, false), "MULTI_SHOT_AUTOMATED");
    }

    #[test]
    fn validation_messages() {
        assert_eq!(err(&req("", 6.0)), "Prompt is required and cannot be empty");
        assert_eq!(
            err(&req(&"a".repeat(4001), 6.0)),
            "Prompt must be 4000 characters or less"
        );
        assert!(err(&req("p", 7.0)).starts_with(
            "Duration must be 6 seconds (TEXT_VIDEO) or a multiple of 6 between 12-120 seconds (MULTI_SHOT_AUTOMATED). Valid values: 6, 12, 18,"
        ));
        assert!(err(&req("p", 7.0)).ends_with("108, 114, 120"));
        let mut r = req("p", 6.0);
        r.s3_uri = String::new();
        assert_eq!(err(&r), "S3 URI is required for video generation");
        r.s3_uri = "https://x".into();
        assert_eq!(err(&r), "S3 URI must start with s3://");
        let mut r = req("p", 6.0);
        r.seed = Some(2_147_483_647.0);
        assert_eq!(err(&r), "Seed must be between 0 and 2147483646");
        r.seed = Some(-1.0);
        assert_eq!(err(&r), "Seed must be between 0 and 2147483646");
        let mut r = req("p", 6.0);
        r.input_images = Some(vec!["a.png".into(), "b.png".into()]);
        assert_eq!(
            err(&r),
            "TEXT_VIDEO mode (6 seconds) supports only a single input image"
        );
        let mut r = req("p", 12.0);
        r.input_images = Some(vec!["a.PNG".into(), "b.gif".into()]);
        assert_eq!(err(&r), "Image 2 must be PNG or JPEG format: b.gif");
        r.input_images = Some(vec!["a.png".into(), "b.jpg".into()]);
        r.prompts = Some(vec!["x".into()]);
        assert_eq!(
            err(&r),
            "Number of prompts (1) must match number of images (2)"
        );
        r.prompts = Some(vec!["x".into(), "  ".into()]);
        assert_eq!(err(&r), "Prompt 2 cannot be empty");
        r.prompts = Some(vec!["x".repeat(4001), "y".into()]);
        assert_eq!(err(&r), "Prompt 1 must be 4000 characters or less");
        r.prompts = Some(vec!["x".into(), "y".into()]);
        assert!(validate_request(&r).is_ok());
        let mut r = req("p", 12.0);
        r.prompts = Some(vec!["x".into()]);
        assert_eq!(
            err(&r),
            "Prompts can only be used together with input images"
        );
        // An empty image list skips the image checks but still allows prompts.
        r.input_images = Some(vec![]);
        assert!(validate_request(&r).is_ok());
    }

    #[test]
    fn image_formats() {
        assert_eq!(detect_image_format("/x/a.PNG").unwrap(), "png");
        assert_eq!(detect_image_format("a.jpg").unwrap(), "jpeg");
        assert_eq!(detect_image_format("a.jpeg").unwrap(), "jpeg");
        assert_eq!(
            detect_image_format("a.gif").unwrap_err().to_string(),
            "Unsupported image format: gif"
        );
        assert_eq!(
            detect_image_format("noext").unwrap_err().to_string(),
            "Unsupported image format: noext"
        );
    }

    #[tokio::test]
    async fn text_video_and_automated_bodies() {
        let s3 = dummy_s3();
        let mut seed = || 99;
        let body = build_nova_reel_request(&s3, NOVA_REEL_V1_1, &req("a cat", 6.0), &mut seed)
            .await
            .unwrap();
        assert_eq!(
            body,
            json!({
                "taskType": "TEXT_VIDEO",
                "textToVideoParams": { "text": "a cat" },
                "videoGenerationConfig": {
                    "durationSeconds": 6, "fps": 24, "dimension": "1280x720", "seed": 99
                }
            })
        );

        let mut r = req("a dog", 18.0);
        r.seed = Some(7.0);
        let body = build_nova_reel_request(&s3, NOVA_REEL_V1_1, &r, &mut || unreachable!())
            .await
            .unwrap();
        assert_eq!(
            body,
            json!({
                "taskType": "MULTI_SHOT_AUTOMATED",
                "multiShotAutomatedParams": { "text": "a dog" },
                "videoGenerationConfig": {
                    "durationSeconds": 18, "fps": 24, "dimension": "1280x720", "seed": 7
                }
            })
        );

        // v1.0 always sends TEXT_VIDEO; seed 0 is falsy and replaced.
        r.seed = Some(0.0);
        let body = build_nova_reel_request(&s3, NOVA_REEL_V1_0, &r, &mut || 5)
            .await
            .unwrap();
        assert_eq!(body["taskType"], "TEXT_VIDEO");
        assert_eq!(body["textToVideoParams"], json!({ "text": "a dog" }));
        assert_eq!(body["videoGenerationConfig"]["durationSeconds"], 18);
        assert_eq!(body["videoGenerationConfig"]["seed"], 5);
    }

    #[tokio::test]
    async fn single_image_is_inlined_as_base64() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("frame.jpg");
        std::fs::write(&img, b"JPG").unwrap();
        let mut r = req("p", 6.0);
        r.input_images = Some(vec![img.to_string_lossy().into_owned()]);
        let body = build_nova_reel_request(&dummy_s3(), NOVA_REEL_V1_1, &r, &mut || 1)
            .await
            .unwrap();
        assert_eq!(
            body["textToVideoParams"],
            json!({ "text": "p", "images": [{ "format": "jpeg", "source": { "bytes": "SlBH" } }] })
        );

        r.input_images = Some(vec!["/definitely/missing.png".into()]);
        let e = build_nova_reel_request(&dummy_s3(), NOVA_REEL_V1_1, &r, &mut || 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            e.starts_with("Failed to read image file /definitely/missing.png: Error: "),
            "{e}"
        );
    }

    #[test]
    fn output_video_uri_appends_output_mp4() {
        assert_eq!(output_video_uri("s3://b/v/abc"), "s3://b/v/abc/output.mp4");
        assert_eq!(output_video_uri("s3://b/v/abc/"), "s3://b/v/abc/output.mp4");
        let mut st = AsyncInvocationStatus {
            output_data_config: Some(json!({ "s3OutputDataConfig": { "s3Uri": "s3://b/v/abc" } })),
            ..Default::default()
        };
        assert_eq!(
            st.output_video_uri().as_deref(),
            Some("s3://b/v/abc/output.mp4")
        );
        st.output_data_config = None;
        assert_eq!(st.output_video_uri(), None);
    }

    #[test]
    fn s3_uri_parsing() {
        assert_eq!(
            parse_s3_uri("s3://b/k/output.mp4"),
            Some(("b", "k/output.mp4"))
        );
        assert_eq!(parse_s3_uri("s3://b/"), None);
        assert_eq!(parse_s3_uri("s3://b"), None);
        assert_eq!(parse_s3_uri("undefined/output.mp4"), None);
    }

    #[test]
    fn result_json_shapes() {
        let s = AsyncInvocationStatus {
            invocation_arn: "arn:x".into(),
            model_id: NOVA_REEL_V1_1.into(),
            status: "Completed".into(),
            submit_time: "2025-01-01T00:00:00.000Z".into(),
            output_data_config: Some(json!({ "s3OutputDataConfig": { "s3Uri": "s3://b/p" } })),
            ..Default::default()
        };
        assert_eq!(s.s3_uri(), Some("s3://b/p"));
        let m = GeneratedMovie {
            invocation_arn: "arn:x".into(),
            status: s,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            json!({
                "invocationArn": "arn:x",
                "status": {
                    "invocationArn": "arn:x",
                    "modelId": "amazon.nova-reel-v1:1",
                    "status": "Completed",
                    "submitTime": "2025-01-01T00:00:00.000Z",
                    "outputDataConfig": { "s3OutputDataConfig": { "s3Uri": "s3://b/p" } }
                }
            })
        );
        let r: GenerateMovieRequest = serde_json::from_value(json!({
            "prompt": "p", "durationSeconds": 12, "s3Uri": "s3://b", "inputImages": ["a.png"],
            "prompts": ["x"], "seed": 3, "outputPath": "/o"
        }))
        .unwrap();
        assert_eq!(r.duration_seconds, 12.0);
        assert_eq!(r.input_images.as_deref(), Some(&["a.png".to_string()][..]));
        assert_eq!(
            serde_json::to_value(DownloadedVideo {
                downloaded_path: "/p.mp4".into(),
                file_size: 3
            })
            .unwrap(),
            json!({ "downloadedPath": "/p.mp4", "fileSize": 3 })
        );
    }
}
