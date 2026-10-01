//! `ApplyGuardrail` — port of `src/main/api/bedrock/services/guardrailService.ts`
//! (`api.bedrock.applyGuardrail` → `bedrock_apply_guardrail { request }`).
//!
//! Input: the `ApplyGuardrailRequest` JSON (`{ guardrailIdentifier, guardrailVersion, content,
//! source }`; like the TS, any other member such as `outputScope` is dropped). Image bytes in
//! `content[].image.source.bytes` may be base64 or any byte-array JSON form.
//!
//! Output: `ApplyGuardrailCommandOutput` JSON — `$metadata` plus the response body as the service
//! sent it (`action`, `actionReason`, `outputs`, `assessments`, `usage`, `guardrailCoverage`).
//!
//! Throttling / service-unavailable errors are retried up to 3 times, 1 s apart.

use crate::error::{Error, Result};
use crate::retry::{random_index, with_retries, FailoverConfig, RetryPolicy};
use crate::sdk::{blobs_in_list, object_of, required_str, JsonBody, RawBody, SdkConfigSource};
use crate::settings::AwsSettings;
use aws_sdk_bedrockruntime::types::GuardrailContentSource;
use serde_json::{Map, Value};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// `MAX_RETRIES = 3`, `RETRY_DELAY = 1000`.
pub const GUARDRAIL_RETRY_POLICY: RetryPolicy = RetryPolicy {
    max_retries: 3,
    delay: Duration::from_secs(1),
};

/// The request body sent to the service: `{ source?, content? }` with blobs as base64.
pub fn guardrail_request_body(request: &Value) -> Result<Value> {
    let m = object_of(request, "ApplyGuardrail request")?;
    let mut body = Map::new();
    for key in ["source", "content"] {
        if let Some(v) = m.get(key).filter(|v| !v.is_null()) {
            body.insert(key.into(), v.clone());
        }
    }
    let mut body = Value::Object(body);
    blobs_in_list(&mut body, &["content"], &["image", "source", "bytes"])?;
    Ok(body)
}

/// `applyGuardrail(request)` with the TS retry policy.
pub async fn apply_guardrail(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    request: &Value,
) -> Result<Value> {
    apply_guardrail_with_policy(configs, aws, request, GUARDRAIL_RETRY_POLICY).await
}

/// [`apply_guardrail`] with an explicit retry policy (tests).
pub async fn apply_guardrail_with_policy(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    request: &Value,
    policy: RetryPolicy,
) -> Result<Value> {
    let m = object_of(request, "ApplyGuardrail request")?;
    let identifier = required_str(m, "guardrailIdentifier")?;
    let version = required_str(m, "guardrailVersion")?;
    let source = m.get("source").and_then(Value::as_str).map(str::to_string);
    let body = JsonBody::new(&guardrail_request_body(request)?);
    tracing::debug!(guardrail_id = %identifier, guardrail_version = %version, region = %aws.region, "Sending apply guardrail request");

    let result = with_retries(
        policy,
        &FailoverConfig::default(),
        &CancellationToken::new(),
        random_index,
        |_| {
            let (identifier, version, source, body) = (
                identifier.clone(),
                version.clone(),
                source.clone(),
                body.clone(),
            );
            async move {
                let conf = configs.sdk_config(aws).await?;
                let client = aws_sdk_bedrockruntime::Client::new(&conf);
                let raw = RawBody::default();
                client
                    .apply_guardrail()
                    .guardrail_identifier(identifier)
                    .guardrail_version(version)
                    .set_source(source.as_deref().map(GuardrailContentSource::from))
                    .customize()
                    .interceptor(body)
                    .interceptor(raw.clone())
                    .send()
                    .await
                    .map_err(Error::from)?;
                let mut out = Map::new();
                out.insert("$metadata".into(), raw.metadata());
                out.extend(raw.json_object());
                Ok(Value::Object(out))
            }
        },
    )
    .await;
    if let Err(e) = &result {
        tracing::error!(guardrail_id = %identifier, error = %e, "Error in applyGuardrail");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_keeps_only_source_and_content() {
        let body = guardrail_request_body(&json!({
            "guardrailIdentifier": "g", "guardrailVersion": "DRAFT", "source": "INPUT",
            "outputScope": "FULL",
            "content": [
                { "text": { "text": "hi", "qualifiers": ["query"] } },
                { "image": { "format": "png", "source": { "bytes": { "0": 1, "1": 2 } } } }
            ]
        }))
        .unwrap();
        assert_eq!(
            body,
            json!({
                "source": "INPUT",
                "content": [
                    { "text": { "text": "hi", "qualifiers": ["query"] } },
                    { "image": { "format": "png", "source": { "bytes": "AQI=" } } }
                ]
            })
        );
    }
}
