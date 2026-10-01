//! Prompt-cache point placement. Port of `src/common/models/promptCache.ts`.
//!
//! The TS version works on AWS SDK Converse types; here messages, system blocks and tool
//! configuration are the Converse JSON shape as [`serde_json::Value`] (`{ role, content: [...] }`,
//! content blocks keyed `text` / `toolUse` / `toolResult` / `cachePoint`), so the crate stays
//! SDK-agnostic. [`PromptCacheManager::message_cache_point_indices`] exposes the placement
//! decision on its own for callers that hold typed SDK messages.

use serde_json::{json, Value};

use crate::registry::{get_model_config, CacheConfig, CacheableField};

fn cache_point() -> Value {
    json!({ "cachePoint": { "type": "default" } })
}

/// Decides where cache points go for a given model.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptCacheManager {
    cache: Option<CacheConfig>,
}

impl PromptCacheManager {
    pub fn new(model_id: &str) -> Self {
        Self {
            cache: get_model_config(model_id).and_then(|c| c.cache.clone()),
        }
    }

    /// Whether the model supports prompt caching at all.
    pub fn is_supported(&self) -> bool {
        self.cache.as_ref().is_some_and(|c| c.supported)
    }

    /// Fields the model allows cache points on (empty when unknown).
    pub fn get_cacheable_fields(&self) -> &[CacheableField] {
        self.cache
            .as_ref()
            .map_or(&[], |c| c.cacheable_fields.as_slice())
    }

    fn allows(&self, field: CacheableField) -> bool {
        self.is_supported() && self.get_cacheable_fields().contains(&field)
    }

    /// Message indices that receive a cache point: `first_cache_point` (if given) and the last
    /// message, de-duplicated. Unless the model can also cache `tools`, a message containing a
    /// `toolResult` is skipped (Amazon Nova rejects a cache point right after a tool result).
    /// Returns an empty list when messages are not cacheable for this model.
    ///
    /// `has_tool_result(i)` reports whether message `i` contains a tool result. An out-of-range
    /// `first_cache_point` is ignored (the TS code throws in that case).
    pub fn message_cache_point_indices(
        &self,
        message_count: usize,
        first_cache_point: Option<usize>,
        has_tool_result: impl Fn(usize) -> bool,
    ) -> Vec<usize> {
        if !self.allows(CacheableField::Messages) || message_count == 0 {
            return Vec::new();
        }
        let last = message_count - 1;
        let mut indices: Vec<usize> = first_cache_point.into_iter().collect();
        if !indices.contains(&last) {
            indices.push(last);
        }
        let tools_cacheable = self.get_cacheable_fields().contains(&CacheableField::Tools);
        indices.retain(|&i| i < message_count && (tools_cacheable || !has_tool_result(i)));
        indices
    }

    /// Returns `messages` with a `cachePoint` block appended to the selected messages
    /// (`addCachePointsToMessages`). Messages whose `content` is not an array are left as is.
    pub fn add_cache_points_to_messages(
        &self,
        messages: &[Value],
        first_cache_point: Option<usize>,
    ) -> Vec<Value> {
        let indices = self.message_cache_point_indices(messages.len(), first_cache_point, |i| {
            messages[i]
                .get("content")
                .and_then(Value::as_array)
                .is_some_and(|blocks| blocks.iter().any(|b| is_truthy(b.get("toolResult"))))
        });
        messages
            .iter()
            .enumerate()
            .map(|(i, message)| {
                let mut message = message.clone();
                if indices.contains(&i) {
                    if let Some(content) = message.get_mut("content").and_then(Value::as_array_mut)
                    {
                        content.push(cache_point());
                    }
                }
                message
            })
            .collect()
    }

    /// Appends a `cachePoint` to a non-empty system prompt when the model can cache `system`
    /// (`addCachePointToSystem`).
    pub fn add_cache_point_to_system(&self, system: &[Value]) -> Vec<Value> {
        let mut out = system.to_vec();
        if self.allows(CacheableField::System) && !out.is_empty() {
            out.push(cache_point());
        }
        out
    }

    /// Appends a `cachePoint` entry to `toolConfig.tools` when it is non-empty and the model can
    /// cache `tools` (`addCachePointToTools`). `None` passes through.
    pub fn add_cache_point_to_tools(&self, tool_config: Option<&Value>) -> Option<Value> {
        let mut config = tool_config?.clone();
        if self.allows(CacheableField::Tools) {
            if let Some(tools) = config.get_mut("tools").and_then(Value::as_array_mut) {
                if !tools.is_empty() {
                    tools.push(cache_point());
                }
            }
        }
        Some(config)
    }
}

/// JS truthiness for an optional JSON value (`block.toolResult`).
fn is_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: &str = "us.anthropic.claude-sonnet-4-20250514-v1:0";

    fn msg(blocks: Value) -> Value {
        json!({ "role": "user", "content": blocks })
    }

    fn nova_style_manager() -> PromptCacheManager {
        // Messages + system cacheable, tools not: the Amazon Nova shape.
        PromptCacheManager {
            cache: Some(CacheConfig {
                supported: true,
                cacheable_fields: vec![CacheableField::Messages, CacheableField::System],
            }),
        }
    }

    #[test]
    fn unknown_model_is_unsupported_and_passes_through() {
        let m = PromptCacheManager::new("unknown-model");
        assert!(!m.is_supported());
        assert!(m.get_cacheable_fields().is_empty());
        let messages = vec![msg(json!([{ "text": "hi" }]))];
        assert_eq!(m.add_cache_points_to_messages(&messages, Some(0)), messages);
        let system = vec![json!({ "text": "sys" })];
        assert_eq!(m.add_cache_point_to_system(&system), system);
        let tools = json!({ "tools": [{ "toolSpec": {} }] });
        assert_eq!(m.add_cache_point_to_tools(Some(&tools)), Some(tools));
    }

    #[test]
    fn adds_cache_points_to_first_and_last_message() {
        let m = PromptCacheManager::new(CLAUDE);
        assert!(m.is_supported());
        let messages = vec![
            msg(json!([{ "text": "a" }])),
            msg(json!([{ "text": "b" }])),
            msg(json!([{ "text": "c" }])),
        ];
        let out = m.add_cache_points_to_messages(&messages, Some(0));
        assert_eq!(out[0]["content"][1], cache_point());
        assert_eq!(out[1], messages[1]);
        assert_eq!(out[2]["content"][1], cache_point());
    }

    #[test]
    fn duplicate_indices_get_one_cache_point() {
        let m = PromptCacheManager::new(CLAUDE);
        let messages = vec![msg(json!([{ "text": "a" }]))];
        let out = m.add_cache_points_to_messages(&messages, Some(0));
        assert_eq!(out[0]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_messages_stay_empty() {
        let m = PromptCacheManager::new(CLAUDE);
        assert!(m.add_cache_points_to_messages(&[], None).is_empty());
    }

    #[test]
    fn skips_tool_result_messages_when_tools_are_not_cacheable() {
        let messages = vec![
            msg(json!([{ "text": "a" }])),
            msg(json!([{ "toolResult": { "toolUseId": "t", "content": [] } }])),
        ];
        let out = nova_style_manager().add_cache_points_to_messages(&messages, Some(0));
        assert_eq!(out[0]["content"][1], cache_point());
        assert_eq!(out[1], messages[1]);

        // A model that caches tools keeps the cache point on the tool-result message.
        let out = PromptCacheManager::new(CLAUDE).add_cache_points_to_messages(&messages, None);
        assert_eq!(out[1]["content"][1], cache_point());
    }

    #[test]
    fn system_cache_point_only_when_non_empty() {
        let m = PromptCacheManager::new(CLAUDE);
        assert!(m.add_cache_point_to_system(&[]).is_empty());
        let out = m.add_cache_point_to_system(&[json!({ "text": "sys" })]);
        assert_eq!(out, vec![json!({ "text": "sys" }), cache_point()]);
    }

    #[test]
    fn tools_cache_point_respects_field_and_emptiness() {
        let m = PromptCacheManager::new(CLAUDE);
        assert_eq!(m.add_cache_point_to_tools(None), None);
        let empty = json!({ "tools": [] });
        assert_eq!(m.add_cache_point_to_tools(Some(&empty)), Some(empty));
        let tools =
            json!({ "tools": [{ "toolSpec": { "name": "x" } }], "toolChoice": { "auto": {} } });
        let out = m.add_cache_point_to_tools(Some(&tools)).unwrap();
        assert_eq!(out["tools"][1], cache_point());
        assert_eq!(out["toolChoice"], tools["toolChoice"]);

        let out = nova_style_manager().add_cache_point_to_tools(Some(&tools));
        assert_eq!(out, Some(tools));
    }
}
