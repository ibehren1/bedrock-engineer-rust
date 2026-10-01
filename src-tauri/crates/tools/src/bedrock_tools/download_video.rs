//! Port of `DownloadVideoTool.ts`: looks up the job's S3 output, then downloads
//! `<output prefix>/output.mp4`.
//!
//! A job that is not `Completed` fails with an `ExecutionError` whose `cause` is the inner
//! `ExecutionError`'s `toJSON()` minus `stack` (not reproducible).

use super::check_video_status::validate_invocation_arn;
use super::{is_string, present, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const NAME: &str = "downloadVideo";
const DESCRIPTION: &str = "Download a completed video from S3 using invocation ARN. Automatically retrieves S3 location from job status and downloads to local path.";

pub struct DownloadVideoTool {
    backend: Arc<dyn BedrockBackend>,
}

impl DownloadVideoTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

/// `Math.round((bytes / 1024 / 1024) * 100) / 100` as `${n}`.
fn megabytes(bytes: u64) -> String {
    let v = ((bytes as f64 / 1024.0 / 1024.0) * 100.0 + 0.5).floor() / 100.0;
    format!("{v}")
}

/// `new Date().toISOString().replace(/[:.]/g, '-')`.
fn file_timestamp() -> String {
    crate::util::js::iso_now().replace([':', '.'], "-")
}

enum Failure {
    /// A plain `Error` (backend failure): `cause` serializes as `{}`.
    Plain(String),
    /// The inner `ExecutionError` (`toJSON()` without `stack`).
    Execution(ToolError),
}

#[async_trait]
impl Tool for DownloadVideoTool {
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
                    "invocationArn": {
                        "type": "string",
                        "description": "ARN of the completed video generation job"
                    },
                    "localPath": {
                        "type": "string",
                        "description": "Optional. Local path to save the video file. Uses project path with timestamp if not specified."
                    }
                },
                "required": ["invocationArn"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        validate_invocation_arn(input, &mut errors);
        if present(input, "localPath") && !is_string(input.get("localPath")) {
            errors.push("Local path must be a string".into());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let arn = input
            .get("invocationArn")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let local_path = input
            .get("localPath")
            .and_then(Value::as_str)
            .map(str::to_string);
        let start = Instant::now();
        let settings = &ctx.settings.converse;

        let result = async {
            let status = self
                .backend
                .check_video_status(settings, &arn)
                .await
                .map_err(Failure::Plain)?;
            if status.status != "Completed" {
                let mut extra = Map::new();
                extra.insert("invocationArn".into(), json!(arn));
                extra.insert("currentStatus".into(), json!(status.status));
                return Err(Failure::Execution(ToolError::execution(
                    format!(
                        "Video generation is not completed yet. Current status: {}",
                        status.status
                    ),
                    NAME,
                    None,
                    Some(extra),
                )));
            }
            let s3_uri = status.output_video_uri().ok_or_else(|| {
                let mut extra = Map::new();
                extra.insert("invocationArn".into(), json!(arn));
                Failure::Execution(ToolError::execution(
                    "Completed job has no S3 output location",
                    NAME,
                    None,
                    Some(extra),
                ))
            })?;

            let mut final_path = local_path.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| {
                let name = format!("downloaded-video-{}.mp4", file_timestamp());
                match ctx.settings.project_path.as_deref() {
                    Some(project) => Path::new(project).join(name).to_string_lossy().into_owned(),
                    None => name,
                }
            });
            if !final_path.ends_with(".mp4") {
                final_path.push_str(".mp4");
            }
            tracing::info!(invocation_arn = %arn, s3_uri = %s3_uri, local_path = %final_path, "Downloading video from S3");

            let download = self
                .backend
                .download_video(settings, &s3_uri, &final_path)
                .await
                .map_err(Failure::Plain)?;
            Ok(download)
        }
        .await;

        match result {
            Ok(download) => {
                let download_time = start.elapsed().as_millis() as u64;
                let path = &download.downloaded_path;
                let message = match tokio::fs::metadata(path).await {
                    Ok(meta) => {
                        if meta.len() == 0 {
                            tracing::warn!(path = %path, "Downloaded video file is empty");
                        }
                        format!(
                            "Video downloaded successfully to {path} ({} MB)",
                            megabytes(download.file_size)
                        )
                    }
                    Err(e) => {
                        tracing::warn!(path = %path, error = %e, "Failed to verify downloaded video");
                        format!("Video downloaded to {path} but file verification failed")
                    }
                };
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": message,
                    "result": {
                        "downloadedPath": path,
                        "fileSize": download.file_size,
                        "invocationArn": arn,
                        "downloadTime": download_time
                    }
                })))
            }
            Err(failure) => {
                let (message, cause) = match failure {
                    Failure::Plain(m) => (m, json!({})),
                    Failure::Execution(e) => {
                        let cause = json!({
                            "name": e.name,
                            "message": e.message,
                            "type": "EXECUTION",
                            "metadata": Value::Object(e.metadata.clone())
                        });
                        (e.message, cause)
                    }
                };
                tracing::error!(error = %message, invocation_arn = %arn, "Error downloading video");
                let mut extra = Map::new();
                extra.insert("invocationArn".into(), json!(arn));
                if let Some(p) = local_path {
                    extra.insert("localPath".into(), json!(p));
                }
                Err(ToolError::execution(
                    format!("Error downloading video: {message}"),
                    NAME,
                    Some(cause),
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
    use bedrock::video::{AsyncInvocationStatus, DownloadedVideo};

    const ARN: &str = "arn:aws:bedrock:us-east-1:1:async-invoke/x";

    fn status(s: &str) -> AsyncInvocationStatus {
        AsyncInvocationStatus {
            invocation_arn: ARN.into(),
            status: s.into(),
            submit_time: "2025-01-01T00:00:00.000Z".into(),
            output_data_config: Some(json!({ "s3OutputDataConfig": { "s3Uri": "s3://b/v/x" } })),
            ..Default::default()
        }
    }

    fn ctx(project: Option<&str>) -> ToolContext {
        ToolContext::new(ToolSettings {
            project_path: project.map(str::to_string),
            ..Default::default()
        })
    }

    #[test]
    fn validation_and_formatting() {
        let t = DownloadVideoTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({"invocationArn": "x", "localPath": 1})),
            vec![
                "Invalid invocation ARN format. Must start with \"arn:\"",
                "Local path must be a string"
            ]
        );
        assert!(t.validate_input(&json!({"invocationArn": ARN})).is_empty());
        assert_eq!(megabytes(0), "0");
        assert_eq!(megabytes(1024 * 1024 * 3 / 2), "1.5");
        assert_eq!(megabytes(5_000_000), "4.77");
        let ts = file_timestamp();
        assert!(!ts.contains(':') && !ts.contains('.') && ts.ends_with('Z'));
    }

    #[tokio::test]
    async fn downloads_output_mp4_to_project_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mp4");
        std::fs::write(&file, b"MP4").unwrap();
        let file_s = file.to_string_lossy().into_owned();

        let fake = Arc::new(FakeBedrock::default());
        *fake.video_status.lock().unwrap() = vec![Ok(status("Completed"))];
        *fake.download.lock().unwrap() = Some(Ok(DownloadedVideo {
            downloaded_path: file_s.clone(),
            file_size: 3 * 1024 * 1024,
        }));
        let t = DownloadVideoTool::new(fake.clone());
        let v = run_tool(
            &t,
            json!({"type": NAME, "invocationArn": ARN, "localPath": dir.path().join("clip").to_string_lossy()}),
            &ctx(None),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["success"], true);
        assert_eq!(
            v["message"],
            format!("Video downloaded successfully to {file_s} (3 MB)")
        );
        assert_eq!(v["result"]["downloadedPath"], file_s);
        assert_eq!(v["result"]["fileSize"], 3 * 1024 * 1024);
        assert_eq!(v["result"]["invocationArn"], ARN);
        assert!(v["result"]["downloadTime"].is_u64());
        let calls = fake.calls();
        assert_eq!(calls[0].0, "checkVideoStatus");
        assert_eq!(
            calls[1],
            (
                "downloadVideo".to_string(),
                json!({ "s3Uri": "s3://b/v/x/output.mp4", "localPath": format!("{}.mp4", dir.path().join("clip").to_string_lossy()) })
            )
        );

        // Default path: <projectPath>/downloaded-video-<timestamp>.mp4; unverifiable file.
        *fake.download.lock().unwrap() = Some(Ok(DownloadedVideo {
            downloaded_path: "/definitely/missing.mp4".into(),
            file_size: 0,
        }));
        let v = run_tool(
            &t,
            json!({"type": NAME, "invocationArn": ARN}),
            &ctx(Some("/proj")),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v["message"],
            "Video downloaded to /definitely/missing.mp4 but file verification failed"
        );
        let local = fake.calls()[3].1["localPath"].as_str().unwrap().to_string();
        assert!(
            local.starts_with("/proj/downloaded-video-") && local.ends_with("Z.mp4"),
            "{local}"
        );
    }

    #[tokio::test]
    async fn not_completed_and_backend_errors() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.video_status.lock().unwrap() = vec![Ok(status("InProgress"))];
        let t = DownloadVideoTool::new(fake.clone());
        let err = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx(None))
            .await
            .unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Error downloading video: Video generation is not completed yet. Current status: InProgress",
                "type": "EXECUTION",
                "toolName": "downloadVideo",
                "cause": {
                    "name": "ExecutionError",
                    "message": "Video generation is not completed yet. Current status: InProgress",
                    "type": "EXECUTION",
                    "metadata": {
                        "toolName": "downloadVideo",
                        "invocationArn": ARN,
                        "currentStatus": "InProgress"
                    }
                },
                "invocationArn": ARN
            })
        );

        *fake.video_status.lock().unwrap() = vec![Ok(status("Completed"))];
        *fake.download.lock().unwrap() = Some(Err("The specified key does not exist.".into()));
        let err = run_tool(
            &t,
            json!({"type": NAME, "invocationArn": ARN, "localPath": "/tmp/x.mp4"}),
            &ctx(None),
        )
        .await
        .unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Error downloading video: The specified key does not exist.",
                "type": "EXECUTION",
                "toolName": "downloadVideo",
                "cause": {},
                "invocationArn": ARN,
                "localPath": "/tmp/x.mp4"
            })
        );

        // No output location: a clear error, and no download is attempted.
        let mut s = status("Completed");
        s.output_data_config = None;
        *fake.video_status.lock().unwrap() = vec![Ok(s)];
        let calls_before = fake.calls().len();
        let err = run_tool(
            &t,
            json!({"type": NAME, "invocationArn": ARN, "localPath": "/tmp/x.mp4"}),
            &ctx(None),
        )
        .await
        .unwrap_err();
        let r = response(&err);
        assert_eq!(
            r["error"],
            "Error downloading video: Completed job has no S3 output location"
        );
        assert_eq!(r["cause"]["metadata"]["invocationArn"], ARN);
        let new_calls = &fake.calls()[calls_before..];
        assert!(
            new_calls.iter().all(|c| c.0 != "downloadVideo"),
            "{new_calls:?}"
        );
    }
}
