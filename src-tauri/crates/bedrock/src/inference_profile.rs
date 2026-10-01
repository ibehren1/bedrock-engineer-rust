//! Application inference profiles — port of
//! `src/main/api/bedrock/services/inferenceProfileService.ts`
//! (`api.bedrock.listApplicationInferenceProfiles` →
//! `bedrock_list_application_inference_profiles`).
//!
//! `ApplicationInferenceProfile` JSON: `{ inferenceProfileArn, inferenceProfileName,
//! description?, status, createdAt, updatedAt, modelSource: { copyFrom }, type }` with the dates as
//! ISO strings (the TS returned `Date`s; missing ones default to "now").

use crate::error::{Error, Result};
use crate::sdk::{iso_millis, now_iso, SdkConfigSource};
use crate::settings::AwsSettings;
use aws_sdk_bedrock::types::{InferenceProfileModel, InferenceProfileType};
use aws_smithy_types::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// `modelSource`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSource {
    pub copy_from: String,
}

/// `ApplicationInferenceProfile` (`src/types/llm.ts`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInferenceProfile {
    pub inference_profile_arn: String,
    pub inference_profile_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub model_source: ModelSource,
    #[serde(rename = "type")]
    pub profile_type: String,
}

fn date_or_now(d: Option<&DateTime>) -> String {
    d.map(iso_millis).unwrap_or_else(now_iso)
}

fn copy_from(models: Option<&[InferenceProfileModel]>) -> String {
    models
        .and_then(|m| m.first())
        .and_then(|m| m.model_arn.clone())
        .unwrap_or_default()
}

/// `listApplicationInferenceProfiles()`: `typeEquals: 'APPLICATION'`, first page only, entries
/// without an ARN dropped. Any failure is logged and yields `[]` so the UI keeps working.
pub async fn list_application_inference_profiles(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
) -> Vec<ApplicationInferenceProfile> {
    let result: Result<Vec<ApplicationInferenceProfile>> = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrock::Client::new(&conf);
        tracing::debug!(region = %aws.region, "Fetching inference profiles list");
        let out = client
            .list_inference_profiles()
            .type_equals(InferenceProfileType::Application)
            .send()
            .await
            .map_err(Error::from)?;
        Ok(out
            .inference_profile_summaries
            .unwrap_or_default()
            .into_iter()
            .map(|p| ApplicationInferenceProfile {
                inference_profile_arn: p.inference_profile_arn.clone(),
                inference_profile_name: p.inference_profile_name.clone(),
                description: p.description.clone(),
                status: p.status.as_str().to_string(),
                created_at: date_or_now(p.created_at.as_ref()),
                updated_at: date_or_now(p.updated_at.as_ref()),
                model_source: ModelSource {
                    copy_from: copy_from(Some(&p.models)),
                },
                profile_type: p.r#type.as_str().to_string(),
            })
            .filter(|p| !p.inference_profile_arn.is_empty())
            .collect())
    }
    .await;
    match result {
        Ok(profiles) => profiles,
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch inference profiles list");
            Vec::new()
        }
    }
}

/// `getApplicationInferenceProfile(arn)`; `None` when missing or on any failure.
pub async fn get_application_inference_profile(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    inference_profile_arn: &str,
) -> Option<ApplicationInferenceProfile> {
    let result: Result<Option<ApplicationInferenceProfile>> = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrock::Client::new(&conf);
        let p = client
            .get_inference_profile()
            .inference_profile_identifier(inference_profile_arn)
            .send()
            .await
            .map_err(Error::from)?;
        if p.inference_profile_arn.is_empty() {
            tracing::warn!(inference_profile_arn, "Inference profile not found");
            return Ok(None);
        }
        Ok(Some(ApplicationInferenceProfile {
            inference_profile_arn: p.inference_profile_arn.clone(),
            inference_profile_name: p.inference_profile_name.clone(),
            description: p.description.clone(),
            status: p.status.as_str().to_string(),
            created_at: date_or_now(p.created_at.as_ref()),
            updated_at: date_or_now(p.updated_at.as_ref()),
            model_source: ModelSource {
                copy_from: copy_from(Some(&p.models)),
            },
            profile_type: p.r#type.as_str().to_string(),
        }))
    }
    .await;
    result.unwrap_or_else(|e| {
        tracing::error!(inference_profile_arn, error = %e, "Failed to fetch inference profile details");
        None
    })
}

/// `extractModelIdFromArn(arn)`: the segment after the last `/`, `unknown` when empty.
pub fn extract_model_id_from_arn(model_arn: &str) -> &str {
    match model_arn.rsplit('/').next() {
        Some(last) if !model_arn.is_empty() && !last.is_empty() => last,
        _ => "unknown",
    }
}

/// `convertProfileToLLM(profile)` (the renderer also has a copy).
pub fn convert_profile_to_llm(profile: &ApplicationInferenceProfile) -> Value {
    let model_name = if profile.inference_profile_name.is_empty() {
        format!(
            "Inference Profile: {}",
            extract_model_id_from_arn(&profile.model_source.copy_from)
        )
    } else {
        profile.inference_profile_name.clone()
    };
    let mut llm = json!({
        "modelId": profile.inference_profile_arn,
        "modelName": model_name,
        "toolUse": true,
        "regions": ["us-east-1", "us-west-2"],
        "isInferenceProfile": true,
        "inferenceProfileArn": profile.inference_profile_arn,
        "maxTokensLimit": 4096,
        "supportsThinking": false,
    });
    if let Some(d) = &profile.description {
        llm["description"] = json!(d);
    }
    llm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_id_from_arn() {
        assert_eq!(
            extract_model_id_from_arn(
                "arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-v2"
            ),
            "anthropic.claude-v2"
        );
        assert_eq!(extract_model_id_from_arn(""), "unknown");
        assert_eq!(extract_model_id_from_arn("arn:x/"), "unknown");
        assert_eq!(extract_model_id_from_arn("plain"), "plain");
    }

    #[test]
    fn profile_to_llm() {
        let mut p = ApplicationInferenceProfile {
            inference_profile_arn: "arn:aws:bedrock:us-west-2:1:application-inference-profile/x"
                .into(),
            model_source: ModelSource {
                copy_from: "arn:aws:bedrock:us-west-2::foundation-model/amazon.nova-pro-v1:0"
                    .into(),
            },
            ..Default::default()
        };
        let llm = convert_profile_to_llm(&p);
        assert_eq!(llm["modelName"], "Inference Profile: amazon.nova-pro-v1:0");
        assert_eq!(llm["modelId"], p.inference_profile_arn);
        assert!(llm.get("description").is_none());
        p.inference_profile_name = "mine".into();
        p.description = Some("d".into());
        let llm = convert_profile_to_llm(&p);
        assert_eq!(llm["modelName"], "mine");
        assert_eq!(llm["description"], "d");
        assert_eq!(llm["maxTokensLimit"], 4096);
    }
}
