//! Model metadata used by the converse path.
//!
//! The request builder needs a few facts from the model registry
//! (`src/common/models/models.ts`). They come through the [`ModelInfo`] trait so tests can
//! supply fixed answers; [`RegistryModelInfo`] answers from the `models` crate and is what
//! [`crate::ConverseService::default`] uses. The value types are the `models` crate's own.

pub use models::{CacheConfig, CacheableField, ThinkingType};

/// Registry lookups used by the converse path. Every method mirrors one TS helper.
pub trait ModelInfo: Send + Sync {
    /// `getThinkingSupportedModelIds().some((id) => modelId.includes(id))`.
    fn supports_thinking(&self, model_id: &str) -> bool;

    /// `getSupportedThinkingTypes(modelId)`.
    fn supported_thinking_types(&self, model_id: &str) -> Vec<ThinkingType>;

    /// `getModelConfig(modelId)?.maxTokensLimit` (used by `clampMaxTokensToModelLimit`).
    fn max_tokens_limit(&self, model_id: &str) -> Option<i64>;

    /// `allModels.find((m) => m.modelId === modelId)?.regions` (used by region failover).
    fn regions(&self, model_id: &str) -> Option<Vec<String>>;

    /// `getModelConfig(modelId)?.cache` (used by the prompt-cache helper).
    fn cache_config(&self, model_id: &str) -> Option<CacheConfig>;
}

/// [`ModelInfo`] backed by the `models` crate registry (the TS helpers in
/// `src/common/models/models.ts`).
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryModelInfo;

impl ModelInfo for RegistryModelInfo {
    fn supports_thinking(&self, model_id: &str) -> bool {
        models::get_thinking_supported_model_ids()
            .iter()
            .any(|id| model_id.contains(id.as_str()))
    }
    fn supported_thinking_types(&self, model_id: &str) -> Vec<ThinkingType> {
        models::get_supported_thinking_types(model_id)
    }
    fn max_tokens_limit(&self, model_id: &str) -> Option<i64> {
        models::get_model_config(model_id).map(|c| i64::from(c.max_tokens_limit))
    }
    fn regions(&self, model_id: &str) -> Option<Vec<String>> {
        models::all_models()
            .iter()
            .find(|m| m.model_id == model_id)
            .map(|m| m.regions.clone())
    }
    fn cache_config(&self, model_id: &str) -> Option<CacheConfig> {
        models::get_model_config(model_id).and_then(|c| c.cache.clone())
    }
}

/// A registry that knows no models: no thinking, no token ceiling, no failover regions, no cache.
/// Useful for tests that must not depend on registry contents.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoModelInfo;

impl ModelInfo for NoModelInfo {
    fn supports_thinking(&self, _model_id: &str) -> bool {
        false
    }
    fn supported_thinking_types(&self, _model_id: &str) -> Vec<ThinkingType> {
        Vec::new()
    }
    fn max_tokens_limit(&self, _model_id: &str) -> Option<i64> {
        None
    }
    fn regions(&self, _model_id: &str) -> Option<Vec<String>> {
        None
    }
    fn cache_config(&self, _model_id: &str) -> Option<CacheConfig> {
        None
    }
}

/// `clampMaxTokensToModelLimit`: the lesser of the requested value and the model ceiling; models
/// without a (truthy) ceiling pass through untouched.
pub fn clamp_max_tokens(
    info: &dyn ModelInfo,
    model_id: &str,
    requested: Option<i64>,
) -> Option<i64> {
    let requested = requested?;
    match info.max_tokens_limit(model_id) {
        Some(limit) if limit != 0 => Some(requested.min(limit)),
        _ => Some(requested),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Limit(Option<i64>);
    impl ModelInfo for Limit {
        fn supports_thinking(&self, _: &str) -> bool {
            false
        }
        fn supported_thinking_types(&self, _: &str) -> Vec<ThinkingType> {
            vec![]
        }
        fn max_tokens_limit(&self, _: &str) -> Option<i64> {
            self.0
        }
        fn regions(&self, _: &str) -> Option<Vec<String>> {
            None
        }
        fn cache_config(&self, _: &str) -> Option<CacheConfig> {
            None
        }
    }

    #[test]
    fn clamps_to_model_ceiling() {
        assert_eq!(
            clamp_max_tokens(&Limit(Some(64000)), "m", Some(128000)),
            Some(64000)
        );
        assert_eq!(
            clamp_max_tokens(&Limit(Some(64000)), "m", Some(4096)),
            Some(4096)
        );
    }

    #[test]
    fn unknown_models_pass_through() {
        assert_eq!(
            clamp_max_tokens(&Limit(None), "m", Some(128000)),
            Some(128000)
        );
        assert_eq!(
            clamp_max_tokens(&Limit(Some(0)), "m", Some(128000)),
            Some(128000)
        );
        assert_eq!(clamp_max_tokens(&Limit(Some(10)), "m", None), None);
        assert_eq!(clamp_max_tokens(&NoModelInfo, "m", Some(5)), Some(5));
    }

    const SONNET_4: &str = "us.anthropic.claude-sonnet-4-20250514-v1:0";
    const NOVA_PRO: &str = "us.amazon.nova-pro-v1:0";

    #[test]
    fn registry_answers_match_the_models_crate() {
        let info = RegistryModelInfo;
        assert!(info.supports_thinking(SONNET_4));
        assert!(!info.supports_thinking(NOVA_PRO));
        assert!(!info.supports_thinking("unknown-model"));
        assert_eq!(
            info.supported_thinking_types(SONNET_4),
            models::get_supported_thinking_types(SONNET_4)
        );
        assert!(info.supported_thinking_types("unknown-model").is_empty());

        let limit = models::get_model_config(SONNET_4).unwrap().max_tokens_limit;
        assert_eq!(info.max_tokens_limit(SONNET_4), Some(i64::from(limit)));
        assert_eq!(info.max_tokens_limit("unknown-model"), None);
        assert_eq!(
            clamp_max_tokens(&info, SONNET_4, Some(10_000_000)),
            Some(i64::from(limit))
        );

        // regions: exact model ID match only (allModels.find(m => m.modelId === id)).
        let regions = info.regions(SONNET_4).unwrap();
        assert!(regions.iter().any(|r| r == "us-east-1"));
        assert_eq!(info.regions("anthropic.claude-sonnet-4"), None);

        assert!(info.cache_config(SONNET_4).unwrap().supported);
        assert_eq!(info.cache_config("unknown-model"), None);
    }

    /// Prompt caching now comes from `models::PromptCacheManager`; these pin the behaviors the
    /// removed bedrock-local manager tested that the models crate tests do not cover.
    #[test]
    fn prompt_cache_manager_agrees_with_cache_config() {
        use crate::PromptCacheManager;
        for id in [SONNET_4, NOVA_PRO, "unknown-model"] {
            let m = PromptCacheManager::new(id);
            let cfg = RegistryModelInfo.cache_config(id);
            assert_eq!(m.is_supported(), cfg.as_ref().is_some_and(|c| c.supported));
            assert_eq!(
                m.get_cacheable_fields(),
                cfg.map(|c| c.cacheable_fields)
                    .unwrap_or_default()
                    .as_slice()
            );
        }
    }

    #[test]
    fn prompt_cache_first_point_on_last_message_marks_only_last() {
        use serde_json::json;
        let m = crate::PromptCacheManager::new(SONNET_4);
        let msgs = vec![
            json!({ "role": "user", "content": [{ "text": "a" }] }),
            json!({ "role": "assistant", "content": [{ "text": "b" }] }),
            json!({ "role": "user", "content": [{ "text": "c" }] }),
        ];
        let out = m.add_cache_points_to_messages(&msgs, Some(2));
        assert_eq!(out[0], msgs[0]);
        assert_eq!(out[1], msgs[1]);
        assert_eq!(out[2]["content"].as_array().unwrap().len(), 2);
        // Out-of-range first point is ignored (TS would throw); the last message still gets one.
        let out = m.add_cache_points_to_messages(&msgs, Some(9));
        assert_eq!(out[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(out[0], msgs[0]);
    }

    #[test]
    fn prompt_cache_real_nova_model_skips_tool_results() {
        use serde_json::json;
        let m = crate::PromptCacheManager::new(NOVA_PRO);
        assert!(m.is_supported());
        assert!(!m.get_cacheable_fields().contains(&CacheableField::Tools));
        let msgs = vec![
            json!({ "role": "user", "content": [{ "text": "a" }] }),
            json!({ "role": "user", "content": [{ "toolResult": { "toolUseId": "t", "content": [] } }] }),
        ];
        let out = m.add_cache_points_to_messages(&msgs, Some(0));
        assert_eq!(out[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(out[1], msgs[1]);
        let tools = json!({ "tools": [{ "toolSpec": { "name": "x" } }] });
        assert_eq!(m.add_cache_point_to_tools(Some(&tools)), Some(tools));
    }
}
