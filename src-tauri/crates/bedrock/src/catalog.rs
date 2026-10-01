//! Model listing — port of `src/main/api/bedrock/services/modelService.ts` and the
//! `bedrock:getModelMaxTokens` handler.
//!
//! * [`list_models`] — the `list_models` command (`GET /listModels`, `LLM[]`): the static
//!   registry's models for the configured region. Application inference profiles are listed
//!   separately ([`crate::inference_profile`]) and merged by the renderer, as before.
//! * [`list_agent_tags`] — the `list_agent_tags` command. The Express route never existed, so
//!   the renderer's `fetch` always failed; this returns an empty list.
//! * [`get_model_max_tokens`] — `bedrock_get_model_max_tokens` → `{ maxTokens }`.

use crate::settings::AwsSettings;
use serde_json::{json, Value};

/// `ModelService.listModels()`: `[]` unless a region and (a profile or an access key) are set.
pub fn list_models(aws: &AwsSettings) -> Vec<models::Llm> {
    if aws.region.is_empty() || (!aws.use_profile.unwrap_or(false) && aws.access_key_id.is_empty())
    {
        tracing::warn!("AWS credentials not configured properly");
        return Vec::new();
    }
    models::get_models_for_region(&aws.region)
}

/// `listAgentTags()`: always empty (see the module docs).
pub fn list_agent_tags() -> Vec<String> {
    Vec::new()
}

/// `bedrock:getModelMaxTokens` → `{ maxTokens }`.
pub fn get_model_max_tokens(model_id: &str) -> Value {
    json!({ "maxTokens": models::get_model_max_tokens(model_id) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aws(region: &str, key: &str, use_profile: Option<bool>) -> AwsSettings {
        AwsSettings {
            region: region.into(),
            access_key_id: key.into(),
            use_profile,
            ..Default::default()
        }
    }

    #[test]
    fn requires_region_and_credentials() {
        assert!(list_models(&aws("", "AKID", None)).is_empty());
        assert!(list_models(&aws("us-west-2", "", None)).is_empty());
        assert!(list_models(&aws("us-west-2", "", Some(false))).is_empty());
        let with_profile = list_models(&aws("us-west-2", "", Some(true)));
        assert_eq!(with_profile, models::get_models_for_region("us-west-2"));
        assert!(!with_profile.is_empty());
        assert_eq!(
            list_models(&aws("us-west-2", "AKID", None)),
            models::get_models_for_region("us-west-2")
        );
    }

    #[test]
    fn llm_json_is_camel_case() {
        let m = &list_models(&aws("us-west-2", "AKID", None))[0];
        let v = serde_json::to_value(m).unwrap();
        assert!(v.get("modelId").is_some());
        assert!(v.get("modelName").is_some());
        assert!(v.get("toolUse").is_some());
    }

    #[test]
    fn agent_tags_and_max_tokens() {
        assert!(list_agent_tags().is_empty());
        assert_eq!(
            get_model_max_tokens("unknown-model"),
            json!({ "maxTokens": 8192 })
        );
    }
}
