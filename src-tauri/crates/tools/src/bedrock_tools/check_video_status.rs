//! Port of `CheckVideoStatusTool.ts`.

use super::{is_string, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const NAME: &str = "checkVideoStatus";
const DESCRIPTION: &str = "Check the status of video generation job using invocation ARN. Returns current status, completion time, and S3 location if completed.";

pub struct CheckVideoStatusTool {
    backend: Arc<dyn BedrockBackend>,
}

impl CheckVideoStatusTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

/// The `invocationArn` checks shared by `checkVideoStatus` and `downloadVideo`.
pub(super) fn validate_invocation_arn(input: &Value, errors: &mut Vec<String>) {
    let arn = input.get("invocationArn");
    if !truthy(arn) {
        errors.push("Invocation ARN is required".into());
    }
    if !is_string(arn) {
        errors.push("Invocation ARN must be a string".into());
    }
    if let Some(Value::String(a)) = arn.filter(|a| truthy(Some(a))) {
        if a.trim().is_empty() {
            errors.push("Invocation ARN cannot be empty".into());
        }
        if !a.starts_with("arn:") {
            errors.push("Invalid invocation ARN format. Must start with \"arn:\"".into());
        }
    }
}

/// Milliseconds since the epoch of an ISO timestamp (`new Date(s).getTime()`).
pub(super) fn parse_millis(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}

/// `new Date(s).toISOString()` (unparsable strings are kept as they are).
fn iso(s: &str) -> String {
    parse_millis(s)
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|| s.to_string())
}

/// `getStatusMessage(status, s3Location)`.
fn status_message(status: &str, s3_location: Option<&str>) -> String {
    match status {
        "InProgress" => "Video generation is in progress. Check again in a few minutes.".into(),
        "Completed" => match s3_location {
            Some(loc) => format!("Video generation completed successfully. S3 location: {loc}. Use downloadVideo to download the video."),
            None => "Video generation completed successfully.".into(),
        },
        "Failed" => {
            "Video generation failed. Check the error details for more information.".into()
        }
        other => format!("Video generation status: {other}"),
    }
}

#[async_trait]
impl Tool for CheckVideoStatusTool {
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
                        "description": "ARN of the video generation job to check status for"
                    }
                },
                "required": ["invocationArn"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        validate_invocation_arn(input, &mut errors);
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let arn = input
            .get("invocationArn")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match self
            .backend
            .check_video_status(&ctx.settings.converse, &arn)
            .await
        {
            Ok(status) => {
                let s3_location = status.s3_uri().map(str::to_string);
                tracing::info!(
                    invocation_arn = %arn,
                    status = %status.status,
                    has_s3_location = s3_location.is_some(),
                    "Video status checked successfully"
                );
                let submit = (!status.submit_time.is_empty()).then_some(&status.submit_time);
                let mut progress = None;
                if status.status == "InProgress" {
                    if let Some(ms) = submit.and_then(|s| parse_millis(s)) {
                        let elapsed = (crate::util::js::now_millis() - ms).div_euclid(60_000);
                        if elapsed > 0 {
                            progress = Some(json!({
                                "estimatedTimeRemaining":
                                    format!("{} minutes (estimated)", (5 - elapsed).max(0))
                            }));
                        }
                    }
                }
                let mut r = Map::new();
                r.insert("invocationArn".into(), json!(arn));
                r.insert("status".into(), json!(status.status));
                r.insert(
                    "submitTime".into(),
                    json!(submit.map_or_else(crate::util::js::iso_now, |s| iso(s))),
                );
                if let Some(end) = status.end_time.as_deref().filter(|e| !e.is_empty()) {
                    r.insert("endTime".into(), json!(iso(end)));
                }
                if let Some(loc) = &s3_location {
                    r.insert("s3Location".into(), json!(loc));
                }
                if status.status == "Failed" {
                    if let Some(m) = &status.failure_message {
                        r.insert("error".into(), json!(m));
                    }
                }
                if let Some(p) = progress {
                    r.insert("progress".into(), p);
                }
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": status_message(&status.status, s3_location.as_deref()),
                    "result": Value::Object(r)
                })))
            }
            Err(e) => {
                tracing::error!(error = %e, invocation_arn = %arn, "Error checking video status");
                let mut extra = Map::new();
                extra.insert("invocationArn".into(), json!(arn));
                Err(ToolError::execution(
                    format!("Error checking video status: {e}"),
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
    use bedrock::video::AsyncInvocationStatus;

    const ARN: &str = "arn:aws:bedrock:us-east-1:1:async-invoke/x";

    fn ctx() -> ToolContext {
        ToolContext::new(ToolSettings::default())
    }

    fn status(s: &str, submit: &str) -> AsyncInvocationStatus {
        AsyncInvocationStatus {
            invocation_arn: ARN.into(),
            model_id: "amazon.nova-reel-v1:1".into(),
            status: s.into(),
            submit_time: submit.into(),
            ..Default::default()
        }
    }

    #[test]
    fn validation_messages() {
        let t = CheckVideoStatusTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({})),
            vec![
                "Invocation ARN is required",
                "Invocation ARN must be a string"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"invocationArn": "  "})),
            vec![
                "Invocation ARN cannot be empty",
                "Invalid invocation ARN format. Must start with \"arn:\""
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"invocationArn": 5})),
            vec!["Invocation ARN must be a string"]
        );
        assert!(t.validate_input(&json!({"invocationArn": ARN})).is_empty());
    }

    #[tokio::test]
    async fn completed_failed_and_in_progress() {
        let fake = Arc::new(FakeBedrock::default());
        let t = CheckVideoStatusTool::new(fake.clone());
        let mut done = status("Completed", "2025-01-02T03:04:05Z");
        done.end_time = Some("2025-01-02T03:10:00.000Z".into());
        done.output_data_config = Some(json!({ "s3OutputDataConfig": { "s3Uri": "s3://b/v/x" } }));
        *fake.video_status.lock().unwrap() = vec![Ok(done)];
        let v = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx())
            .await
            .unwrap()
            .into_value();
        assert_eq!(
            v,
            json!({
                "success": true,
                "name": "checkVideoStatus",
                "message": "Video generation completed successfully. S3 location: s3://b/v/x. Use downloadVideo to download the video.",
                "result": {
                    "invocationArn": ARN,
                    "status": "Completed",
                    "submitTime": "2025-01-02T03:04:05.000Z",
                    "endTime": "2025-01-02T03:10:00.000Z",
                    "s3Location": "s3://b/v/x"
                }
            })
        );
        assert_eq!(fake.calls()[0].1, json!({ "invocationArn": ARN }));

        let mut failed = status("Failed", "2025-01-02T03:04:05.000Z");
        failed.failure_message = Some("filtered".into());
        *fake.video_status.lock().unwrap() = vec![Ok(failed)];
        let v = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx())
            .await
            .unwrap()
            .into_value();
        assert_eq!(
            v["message"],
            "Video generation failed. Check the error details for more information."
        );
        assert_eq!(v["result"]["error"], "filtered");

        // Long-running: the 5-minute heuristic bottoms out at 0.
        *fake.video_status.lock().unwrap() =
            vec![Ok(status("InProgress", "2020-01-01T00:00:00.000Z"))];
        let v = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx())
            .await
            .unwrap()
            .into_value();
        assert_eq!(
            v["message"],
            "Video generation is in progress. Check again in a few minutes."
        );
        assert_eq!(
            v["result"]["progress"],
            json!({ "estimatedTimeRemaining": "0 minutes (estimated)" })
        );
        // Just submitted: no progress yet.
        *fake.video_status.lock().unwrap() =
            vec![Ok(status("InProgress", &crate::util::js::iso_now()))];
        let v = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx())
            .await
            .unwrap()
            .into_value();
        assert!(v["result"].get("progress").is_none());
        assert_eq!(
            status_message("Weird", None),
            "Video generation status: Weird"
        );
        assert_eq!(
            status_message("Completed", None),
            "Video generation completed successfully."
        );
    }

    #[tokio::test]
    async fn backend_errors_are_wrapped() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.video_status.lock().unwrap() = vec![Err("bad arn".into())];
        let t = CheckVideoStatusTool::new(fake);
        let err = run_tool(&t, json!({"type": NAME, "invocationArn": ARN}), &ctx())
            .await
            .unwrap_err();
        assert_eq!(
            response(&err),
            json!({
                "success": false,
                "error": "Error checking video status: bad arn",
                "type": "EXECUTION",
                "toolName": "checkVideoStatus",
                "cause": {},
                "invocationArn": ARN
            })
        );
    }
}
