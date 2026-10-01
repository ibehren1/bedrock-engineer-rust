//! Converse request building — port of `ConverseService.prepareCommandParameters`
//! (`src/main/api/bedrock/services/converseService.ts`).
//!
//! [`prepare_request`] is a pure function over JSON so every model quirk is unit-testable without
//! the SDK. [`PreparedRequest::to_sdk`] then converts the result into SDK types.

use crate::convert::{
    inference_config_to_sdk, message_from_json, system_block_from_json, tool_config_from_json,
};
use crate::document::json_to_document;
use crate::error::Result;
use crate::model_info::{clamp_max_tokens, ModelInfo, ThinkingType};
use crate::settings::{ConverseSettings, InferenceConfig};
use aws_sdk_bedrockruntime::types::{
    InferenceConfiguration, Message, SystemContentBlock, ToolConfiguration,
};
use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Suffix appended to the first system prompt block for Amazon Nova models.
pub const NOVA_SEQUENTIAL_TOOL_HINT: &str =
    "\n Do not run ToolUse in parallel, but proceed step by step.";

/// Beta flag sent when interleaved thinking is on.
pub const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";

/// The request body the renderer sends today to `POST /converse/stream` and `POST /converse`
/// (`CallConverseAPIProps` in `src/main/api/bedrock/types.ts`). Deserialize the command argument
/// straight into this.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverseRequest {
    pub model_id: String,
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrail_config: Option<Value>,
    /// Overrides the stored `inferenceParams` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
    /// Disable extended thinking for this request (title generation etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_thinking: Option<bool>,
}

/// The equivalent of the TS `commandParams` object, still in JSON form.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedRequest {
    pub model_id: String,
    pub messages: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<Value>,
    pub inference_config: InferenceConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_model_request_fields: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrail_config: Option<Value>,
}

/// SDK-typed pieces of a Converse / ConverseStream request.
#[derive(Debug, Clone)]
pub struct SdkRequestParts {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system: Option<Vec<SystemContentBlock>>,
    pub tool_config: Option<ToolConfiguration>,
    pub inference_config: InferenceConfiguration,
    pub additional_model_request_fields: Option<Document>,
    /// Kept as JSON; converted per operation (`GuardrailConfiguration` for Converse,
    /// `GuardrailStreamConfiguration` for ConverseStream).
    pub guardrail_config: Option<Value>,
}

impl PreparedRequest {
    /// Convert into SDK types. Fails with [`crate::Error::InvalidRequest`] on malformed blocks.
    pub fn to_sdk(&self) -> Result<SdkRequestParts> {
        Ok(SdkRequestParts {
            model_id: self.model_id.clone(),
            messages: self
                .messages
                .iter()
                .map(message_from_json)
                .collect::<Result<Vec<_>>>()?,
            system: match &self.system {
                Some(blocks) => Some(
                    blocks
                        .iter()
                        .map(system_block_from_json)
                        .collect::<Result<Vec<_>>>()?,
                ),
                None => None,
            },
            tool_config: match &self.tool_config {
                Some(tc) => Some(tool_config_from_json(tc)?),
                None => None,
            },
            inference_config: inference_config_to_sdk(&self.inference_config)?,
            additional_model_request_fields: self
                .additional_model_request_fields
                .as_ref()
                .map(json_to_document),
            guardrail_config: self.guardrail_config.clone(),
        })
    }
}

/// JS truthiness for JSON values.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `!block.text || !block.text.trim()` for a block that has an own `text` key.
fn is_blank_text(text: &Value) -> bool {
    match text {
        Value::String(s) => s.trim().is_empty(),
        other => !truthy(Some(other)),
    }
}

/// `processMessages` + `processImageContent`: image blocks whose `source.bytes` is set are
/// rebuilt as `{ image: { format, source: { bytes } } }` (bytes decoding happens in conversion).
pub fn process_messages(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|msg| {
            let mut msg = msg.clone();
            if let Some(content) = msg.get_mut("content").and_then(Value::as_array_mut) {
                for block in content.iter_mut() {
                    let Some(image) = block.get("image").filter(|i| i.is_object()) else {
                        continue;
                    };
                    let bytes = image
                        .get("source")
                        .filter(|s| s.is_object())
                        .and_then(|s| s.get("bytes"))
                        .filter(|b| truthy(Some(b)))
                        .filter(|b| b.is_string() || b.is_object() || b.is_array())
                        .cloned();
                    if let Some(bytes) = bytes {
                        let mut img = Map::new();
                        if let Some(format) = image.get("format") {
                            img.insert("format".into(), format.clone());
                        }
                        img.insert("source".into(), json!({ "bytes": bytes }));
                        *block = json!({ "image": Value::Object(img) });
                    }
                }
            }
            msg
        })
        .collect()
}

fn fix_empty_tool_input(block: &mut Value) {
    if let Some(tool_use) = block.get_mut("toolUse").filter(|t| truthy(Some(t))) {
        if tool_use.get("input") == Some(&Value::String(String::new())) {
            tool_use["input"] = json!({});
            tracing::debug!(
                tool_name = ?tool_use.get("name"),
                "Empty toolUse.input converted to empty JSON object"
            );
        }
    }
}

/// `normalizeMessages` (+ `sanitizeContentBlocks`):
///
/// * text blocks that are blank are dropped unless they also carry another member;
/// * blank text left in multi-member blocks becomes `" "`;
/// * `toolUse.input === ""` becomes `{}`;
/// * a message left with no blocks gets `[{ text: " " }]`;
/// * non-array `content` is removed.
pub fn normalize_messages(messages: Vec<Value>) -> Vec<Value> {
    messages
        .into_iter()
        .map(|mut message| {
            let Some(obj) = message.as_object_mut() else {
                return message;
            };
            if !truthy(obj.get("content")) {
                return message;
            }
            match obj.get("content") {
                Some(Value::Array(blocks)) => {
                    let mut valid: Vec<Value> = blocks
                        .iter()
                        .filter(|block| match block.as_object() {
                            Some(b) if b.contains_key("text") && is_blank_text(&b["text"]) => {
                                b.len() > 1
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    for block in valid.iter_mut() {
                        fix_empty_tool_input(block);
                    }
                    if valid.is_empty() {
                        obj.insert("content".into(), json!([{ "text": " " }]));
                    } else {
                        for block in valid.iter_mut() {
                            if let Some(b) = block.as_object_mut() {
                                if b.contains_key("text") && is_blank_text(&b["text"]) {
                                    b.insert("text".into(), json!(" "));
                                }
                            }
                            fix_empty_tool_input(block);
                        }
                        obj.insert("content".into(), Value::Array(valid));
                    }
                }
                _ => {
                    obj.remove("content");
                }
            }
            message
        })
        .collect()
}

fn log_empty_text_fields(messages: &[Value]) {
    let count = messages
        .iter()
        .filter(|m| {
            m.get("content").and_then(Value::as_array).is_some_and(|c| {
                c.iter().any(|b| {
                    b.as_object()
                        .is_some_and(|o| o.contains_key("text") && is_blank_text(&o["text"]))
                })
            })
        })
        .count();
    if count > 0 {
        tracing::debug!(
            count,
            "Found empty text fields in content blocks before sanitization"
        );
    }
}

/// Port of `prepareCommandParameters`. Applies, in the TS order:
///
/// 1. image normalization and message sanitization;
/// 2. inference config = request override or stored `inferenceParams`, `maxTokens` clamped;
/// 3. Anthropic/Claude: drop `topP` when `temperature` is also set;
/// 4. Claude Fable 5: adaptive thinking always on, `temperature = 1`, no `topP`, forced tool
///    choice relaxed to `auto`;
/// 5. thinking-capable models with thinking on (and not disabled, not forcing a tool): `thinking`
///    field (enabled+budget or adaptive per model support), optional interleaved beta,
///    `temperature = 1`, no `topP`;
/// 6. OpenAI GPT-5.x/6 and xAI Grok: no sampling fields, `reasoning.effort` from the budget;
/// 7. Kimi K3: no sampling fields;
/// 8. Amazon Nova: `inferenceConfig.topK = 1`, `topP = temperature = 1`, sequential tool hint in
///    the system prompt;
/// 9. guardrail: request override, else the stored guardrail when enabled.
pub fn prepare_request(
    req: &ConverseRequest,
    settings: &ConverseSettings,
    info: &dyn ModelInfo,
) -> PreparedRequest {
    let model_id = req.model_id.as_str();
    let mut tool_config = req.tool_config.clone();

    // Bedrock rejects extended thinking together with a forced tool choice.
    let forces_tool_use = truthy(
        tool_config
            .as_ref()
            .and_then(|t| t.get("toolChoice"))
            .and_then(|c| c.get("tool")),
    );

    let processed = process_messages(&req.messages);
    log_empty_text_fields(&processed);
    let messages = normalize_messages(processed);

    let mut inference = req
        .inference_config
        .clone()
        .unwrap_or_else(|| settings.inference_params.clone());
    let clamped = clamp_max_tokens(info, model_id, inference.max_tokens);
    if clamped != inference.max_tokens {
        tracing::debug!(model_id, requested = ?inference.max_tokens, applied = ?clamped, "Clamped maxTokens to the model limit");
        inference.max_tokens = clamped;
    }

    let thinking_mode = settings.thinking_mode.as_ref();
    let thinking_kind = thinking_mode.and_then(|t| t.kind.as_deref());
    let thinking_on = thinking_kind.is_some_and(|k| k != "disabled");
    let disable_thinking = req.disable_thinking.unwrap_or(false);

    let mut additional: Option<Value> = None;
    let mut system = req.system.clone();

    let is_openai_reasoning =
        model_id.contains("openai.gpt-5") || model_id.contains("openai.gpt-6");
    let is_grok_reasoning = model_id.contains("xai.grok");
    let is_reasoning_effort = is_openai_reasoning || is_grok_reasoning;
    let is_kimi_k3 = model_id.contains("kimi-k3");

    if (model_id.contains("anthropic") || model_id.contains("claude"))
        && inference.temperature.is_some()
        && inference.top_p.is_some()
    {
        inference.top_p = None;
    }

    let is_fable5 = model_id.contains("claude-fable-5");

    if is_fable5 && forces_tool_use {
        if let Some(tc) = tool_config.as_mut().and_then(Value::as_object_mut) {
            tc.insert("toolChoice".into(), json!({ "auto": {} }));
            tracing::debug!(
                model_id,
                "Relaxed forced tool choice to auto (Fable 5 keeps thinking on)"
            );
        }
    }

    if is_fable5 {
        additional = Some(json!({ "thinking": { "type": "adaptive" } }));
        inference.temperature = Some(1.0);
        inference.top_p = None;
    }

    if !is_fable5
        && !is_reasoning_effort
        && !disable_thinking
        && !forces_tool_use
        && info.supports_thinking(model_id)
        && thinking_on
    {
        let supported = info.supported_thinking_types(model_id);
        let mut thinking_type = thinking_kind.unwrap_or_default();
        if thinking_type == "enabled"
            && !supported.contains(&ThinkingType::Enabled)
            && supported.contains(&ThinkingType::Adaptive)
        {
            thinking_type = "adaptive";
        }
        if thinking_type == "adaptive"
            && !supported.contains(&ThinkingType::Adaptive)
            && supported.contains(&ThinkingType::Enabled)
        {
            thinking_type = "enabled";
        }

        let mut fields = Map::new();
        if thinking_type == "adaptive" {
            fields.insert("thinking".into(), json!({ "type": "adaptive" }));
        } else {
            let mut thinking = Map::new();
            thinking.insert("type".into(), json!("enabled"));
            if let Some(budget) = thinking_mode.and_then(|t| t.budget_tokens) {
                thinking.insert("budget_tokens".into(), json!(budget));
            }
            fields.insert("thinking".into(), Value::Object(thinking));
        }
        if settings.interleave_thinking {
            fields.insert("anthropic_beta".into(), json!([INTERLEAVED_THINKING_BETA]));
        }
        additional = Some(Value::Object(fields));
        inference.top_p = None;
        inference.temperature = Some(1.0);
        tracing::debug!(model_id, thinking_type, "Enabling Thinking Mode");
    }

    if is_reasoning_effort {
        inference.temperature = None;
        inference.top_p = None;
        let reasoning_enabled = !disable_thinking && thinking_on;
        additional = if reasoning_enabled {
            let budget = thinking_mode.and_then(|t| t.budget_tokens).unwrap_or(0);
            Some(json!({ "reasoning": { "effort": reasoning_effort(budget) } }))
        } else {
            None
        };
    }

    if is_kimi_k3 {
        inference.temperature = None;
        inference.top_p = None;
    }

    if model_id.contains("nova") {
        additional = Some(json!({ "inferenceConfig": { "topK": 1 } }));
        inference.top_p = Some(1.0);
        inference.temperature = Some(1.0);
        // TS mutates `system[0].text`; a missing/non-text first block would throw there. Here it
        // is left alone instead.
        if let Some(first) = system.as_mut().and_then(|s| s.first_mut()) {
            if let Some(Value::String(text)) = first.get_mut("text") {
                text.push_str(NOVA_SEQUENTIAL_TOOL_HINT);
            }
        }
    }

    let guardrail_config = if truthy(req.guardrail_config.as_ref()) {
        req.guardrail_config.clone()
    } else {
        settings
            .guardrail_settings
            .as_ref()
            .filter(|g| g.enabled && !g.guardrail_identifier.is_empty())
            .map(|g| {
                let mut m = Map::new();
                m.insert("guardrailIdentifier".into(), json!(g.guardrail_identifier));
                m.insert("guardrailVersion".into(), json!(g.guardrail_version));
                if let Some(trace) = &g.trace {
                    m.insert("trace".into(), json!(trace));
                }
                Value::Object(m)
            })
    };

    PreparedRequest {
        model_id: model_id.to_string(),
        messages,
        system,
        tool_config,
        inference_config: inference,
        additional_model_request_fields: additional,
        guardrail_config,
    }
}

/// Thinking budget -> reasoning effort for reasoning-effort models.
pub fn reasoning_effort(budget: i64) -> &'static str {
    if budget >= 32768 {
        "xhigh"
    } else if budget >= 8000 {
        "high"
    } else if budget >= 2000 {
        "medium"
    } else {
        "low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_info::CacheConfig;
    use crate::settings::{GuardrailSettings, ThinkingMode};

    /// Registry stub: thinking support by substring, per-model thinking types and limits.
    struct Info {
        thinking_ids: Vec<&'static str>,
        types: Vec<ThinkingType>,
        limit: Option<i64>,
    }

    impl ModelInfo for Info {
        fn supports_thinking(&self, model_id: &str) -> bool {
            self.thinking_ids.iter().any(|id| model_id.contains(id))
        }
        fn supported_thinking_types(&self, _: &str) -> Vec<ThinkingType> {
            self.types.clone()
        }
        fn max_tokens_limit(&self, _: &str) -> Option<i64> {
            self.limit
        }
        fn regions(&self, _: &str) -> Option<Vec<String>> {
            None
        }
        fn cache_config(&self, _: &str) -> Option<CacheConfig> {
            None
        }
    }

    fn claude_info() -> Info {
        Info {
            thinking_ids: vec!["claude-sonnet-4", "claude-opus-5"],
            types: vec![ThinkingType::Enabled],
            limit: Some(64000),
        }
    }

    fn settings(thinking: Option<(&str, Option<i64>)>) -> ConverseSettings {
        ConverseSettings {
            inference_params: InferenceConfig {
                max_tokens: Some(4096),
                temperature: Some(0.5),
                top_p: Some(0.9),
                stop_sequences: None,
            },
            thinking_mode: thinking.map(|(k, b)| ThinkingMode {
                kind: Some(k.to_string()),
                budget_tokens: b,
            }),
            ..Default::default()
        }
    }

    fn req(model_id: &str) -> ConverseRequest {
        ConverseRequest {
            model_id: model_id.to_string(),
            messages: vec![json!({ "role": "user", "content": [{ "text": "hi" }] })],
            system: Some(vec![json!({ "text": "You are helpful." })]),
            ..Default::default()
        }
    }

    #[test]
    fn deserializes_renderer_body() {
        let r: ConverseRequest = serde_json::from_value(json!({
            "modelId": "anthropic.claude-sonnet-4",
            "system": [{ "text": "s" }],
            "messages": [{ "role": "user", "content": [{ "text": "hi" }] }],
            "toolConfig": { "tools": [] },
            "inferenceConfig": { "maxTokens": 100, "temperature": 0.2 },
            "disableThinking": true
        }))
        .unwrap();
        assert_eq!(r.inference_config.unwrap().max_tokens, Some(100));
        assert_eq!(r.disable_thinking, Some(true));
        // system may be omitted (StreamChatCompletionProps.system is optional)
        let r: ConverseRequest =
            serde_json::from_value(json!({ "modelId": "m", "messages": [] })).unwrap();
        assert!(r.system.is_none());
    }

    #[test]
    fn plain_model_uses_stored_inference_params() {
        let p = prepare_request(&req("meta.llama3"), &settings(None), &NoInfo);
        assert_eq!(p.inference_config, settings(None).inference_params);
        assert_eq!(p.additional_model_request_fields, None);
        assert_eq!(p.guardrail_config, None);
        assert_eq!(p.system, Some(vec![json!({ "text": "You are helpful." })]));
    }

    struct NoInfo;
    impl ModelInfo for NoInfo {
        fn supports_thinking(&self, _: &str) -> bool {
            false
        }
        fn supported_thinking_types(&self, _: &str) -> Vec<ThinkingType> {
            vec![]
        }
        fn max_tokens_limit(&self, _: &str) -> Option<i64> {
            None
        }
        fn regions(&self, _: &str) -> Option<Vec<String>> {
            None
        }
        fn cache_config(&self, _: &str) -> Option<CacheConfig> {
            None
        }
    }

    #[test]
    fn request_inference_config_overrides_store_and_is_clamped() {
        let mut r = req("anthropic.claude-sonnet-4");
        r.inference_config = Some(InferenceConfig {
            max_tokens: Some(128000),
            temperature: None,
            top_p: Some(0.7),
            stop_sequences: None,
        });
        let p = prepare_request(&r, &settings(None), &claude_info());
        assert_eq!(p.inference_config.max_tokens, Some(64000));
        // temperature not set -> topP kept
        assert_eq!(p.inference_config.top_p, Some(0.7));
    }

    #[test]
    fn claude_drops_top_p_when_temperature_set() {
        let p = prepare_request(&req("anthropic.claude-haiku"), &settings(None), &NoInfo);
        assert_eq!(p.inference_config.temperature, Some(0.5));
        assert_eq!(p.inference_config.top_p, None);
    }

    #[test]
    fn thinking_enabled_with_budget() {
        let p = prepare_request(
            &req("us.anthropic.claude-sonnet-4-20250514-v1:0"),
            &settings(Some(("enabled", Some(4096)))),
            &claude_info(),
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "enabled", "budget_tokens": 4096 } }))
        );
        assert_eq!(p.inference_config.temperature, Some(1.0));
        assert_eq!(p.inference_config.top_p, None);
    }

    #[test]
    fn thinking_interleave_adds_beta() {
        let mut s = settings(Some(("enabled", Some(1024))));
        s.interleave_thinking = true;
        let p = prepare_request(&req("anthropic.claude-sonnet-4"), &s, &claude_info());
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({
                "thinking": { "type": "enabled", "budget_tokens": 1024 },
                "anthropic_beta": ["interleaved-thinking-2025-05-14"]
            }))
        );
    }

    #[test]
    fn thinking_enabled_without_budget_omits_budget_tokens() {
        let p = prepare_request(
            &req("anthropic.claude-sonnet-4"),
            &settings(Some(("enabled", None))),
            &claude_info(),
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "enabled" } }))
        );
    }

    #[test]
    fn thinking_type_follows_model_support() {
        let adaptive_only = Info {
            thinking_ids: vec!["claude-opus-5"],
            types: vec![ThinkingType::Adaptive],
            limit: None,
        };
        let p = prepare_request(
            &req("anthropic.claude-opus-5"),
            &settings(Some(("enabled", Some(4096)))),
            &adaptive_only,
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "adaptive" } }))
        );

        let enabled_only = claude_info();
        let p = prepare_request(
            &req("anthropic.claude-sonnet-4"),
            &settings(Some(("adaptive", Some(2048)))),
            &enabled_only,
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "enabled", "budget_tokens": 2048 } }))
        );

        let both = Info {
            thinking_ids: vec!["claude-opus-5"],
            types: vec![ThinkingType::Enabled, ThinkingType::Adaptive],
            limit: None,
        };
        let p = prepare_request(
            &req("anthropic.claude-opus-5"),
            &settings(Some(("adaptive", Some(2048)))),
            &both,
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "adaptive" } }))
        );
    }

    #[test]
    fn thinking_skipped_when_disabled_unsupported_or_forced_tool() {
        let info = claude_info();
        // disabled mode
        let p = prepare_request(
            &req("anthropic.claude-sonnet-4"),
            &settings(Some(("disabled", None))),
            &info,
        );
        assert_eq!(p.additional_model_request_fields, None);
        assert_eq!(p.inference_config.temperature, Some(0.5));
        // no thinkingMode at all
        let p = prepare_request(&req("anthropic.claude-sonnet-4"), &settings(None), &info);
        assert_eq!(p.additional_model_request_fields, None);
        // request-level disable (title generation)
        let mut r = req("anthropic.claude-sonnet-4");
        r.disable_thinking = Some(true);
        let p = prepare_request(&r, &settings(Some(("enabled", Some(4096)))), &info);
        assert_eq!(p.additional_model_request_fields, None);
        // model without thinking support
        let p = prepare_request(
            &req("anthropic.claude-3-haiku"),
            &settings(Some(("enabled", Some(4096)))),
            &info,
        );
        assert_eq!(p.additional_model_request_fields, None);
        // forced tool choice (structured output)
        let mut r = req("anthropic.claude-sonnet-4");
        r.tool_config = Some(json!({ "tools": [], "toolChoice": { "tool": { "name": "out" } } }));
        let p = prepare_request(&r, &settings(Some(("enabled", Some(4096)))), &info);
        assert_eq!(p.additional_model_request_fields, None);
        assert_eq!(p.tool_config, r.tool_config);
    }

    #[test]
    fn fable5_always_adaptive_and_relaxes_forced_tool() {
        let mut r = req("us.anthropic.claude-fable-5-1");
        r.tool_config = Some(
            json!({ "tools": [{ "toolSpec": { "name": "out" } }], "toolChoice": { "tool": { "name": "out" } } }),
        );
        let p = prepare_request(&r, &settings(Some(("disabled", None))), &NoInfo);
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "thinking": { "type": "adaptive" } }))
        );
        assert_eq!(p.tool_config.unwrap()["toolChoice"], json!({ "auto": {} }));
        assert_eq!(p.inference_config.temperature, Some(1.0));
        assert_eq!(p.inference_config.top_p, None);
    }

    #[test]
    fn reasoning_effort_models() {
        for model in ["openai.gpt-5-1", "openai.gpt-6", "xai.grok-4-6"] {
            let p = prepare_request(
                &req(model),
                &settings(Some(("enabled", Some(16384)))),
                &claude_info(),
            );
            assert_eq!(p.inference_config.temperature, None, "{model}");
            assert_eq!(p.inference_config.top_p, None, "{model}");
            assert_eq!(p.inference_config.max_tokens, Some(4096));
            assert_eq!(
                p.additional_model_request_fields,
                Some(json!({ "reasoning": { "effort": "high" } })),
                "{model}"
            );
        }
        let p = prepare_request(
            &req("openai.gpt-5"),
            &settings(Some(("disabled", None))),
            &NoInfo,
        );
        assert_eq!(p.additional_model_request_fields, None);
        let mut r = req("openai.gpt-5");
        r.disable_thinking = Some(true);
        let p = prepare_request(&r, &settings(Some(("enabled", Some(40000)))), &NoInfo);
        assert_eq!(p.additional_model_request_fields, None);
        // gpt-oss is not a reasoning-effort model
        let p = prepare_request(
            &req("openai.gpt-oss-120b"),
            &settings(Some(("enabled", Some(4096)))),
            &NoInfo,
        );
        assert_eq!(p.inference_config.temperature, Some(0.5));
    }

    #[test]
    fn reasoning_effort_thresholds() {
        assert_eq!(reasoning_effort(0), "low");
        assert_eq!(reasoning_effort(1999), "low");
        assert_eq!(reasoning_effort(2000), "medium");
        assert_eq!(reasoning_effort(7999), "medium");
        assert_eq!(reasoning_effort(8000), "high");
        assert_eq!(reasoning_effort(32767), "high");
        assert_eq!(reasoning_effort(32768), "xhigh");
    }

    #[test]
    fn kimi_k3_drops_sampling() {
        let p = prepare_request(&req("moonshot.kimi-k3"), &settings(None), &NoInfo);
        assert_eq!(p.inference_config.temperature, None);
        assert_eq!(p.inference_config.top_p, None);
        assert_eq!(p.inference_config.max_tokens, Some(4096));
    }

    #[test]
    fn nova_greedy_decoding_and_system_hint() {
        let p = prepare_request(
            &req("us.amazon.nova-pro-v1:0"),
            &settings(Some(("enabled", Some(4096)))),
            &NoInfo,
        );
        assert_eq!(
            p.additional_model_request_fields,
            Some(json!({ "inferenceConfig": { "topK": 1 } }))
        );
        assert_eq!(p.inference_config.top_p, Some(1.0));
        assert_eq!(p.inference_config.temperature, Some(1.0));
        assert_eq!(
            p.system.unwrap()[0]["text"],
            json!(format!("You are helpful.{NOVA_SEQUENTIAL_TOOL_HINT}"))
        );
        // no system prompt: nothing to append, no panic
        let mut r = req("amazon.nova-lite");
        r.system = None;
        assert!(prepare_request(&r, &settings(None), &NoInfo)
            .system
            .is_none());
    }

    #[test]
    fn guardrail_from_request_or_settings() {
        let mut s = settings(None);
        s.guardrail_settings = Some(GuardrailSettings {
            enabled: true,
            guardrail_identifier: "gid".into(),
            guardrail_version: "DRAFT".into(),
            trace: Some("enabled".into()),
        });
        let p = prepare_request(&req("m"), &s, &NoInfo);
        assert_eq!(
            p.guardrail_config,
            Some(
                json!({ "guardrailIdentifier": "gid", "guardrailVersion": "DRAFT", "trace": "enabled" })
            )
        );

        let mut r = req("m");
        r.guardrail_config =
            Some(json!({ "guardrailIdentifier": "other", "guardrailVersion": "2" }));
        assert_eq!(
            prepare_request(&r, &s, &NoInfo).guardrail_config,
            r.guardrail_config
        );

        // disabled or empty identifier -> none
        s.guardrail_settings
            .as_mut()
            .unwrap()
            .guardrail_identifier
            .clear();
        assert_eq!(
            prepare_request(&req("m"), &s, &NoInfo).guardrail_config,
            None
        );
        s.guardrail_settings.as_mut().unwrap().guardrail_identifier = "gid".into();
        s.guardrail_settings.as_mut().unwrap().enabled = false;
        assert_eq!(
            prepare_request(&req("m"), &s, &NoInfo).guardrail_config,
            None
        );
    }

    #[test]
    fn normalize_drops_blank_text_and_fills_empty_messages() {
        let out = normalize_messages(vec![
            json!({ "role": "user", "content": [{ "text": "" }, { "text": "  \n" }, { "text": "ok" }] }),
            json!({ "role": "assistant", "content": [{ "text": "" }] }),
            json!({ "role": "assistant", "content": [] }),
        ]);
        assert_eq!(out[0]["content"], json!([{ "text": "ok" }]));
        assert_eq!(out[1]["content"], json!([{ "text": " " }]));
        // empty array is truthy in JS -> becomes [{ text: ' ' }]
        assert_eq!(out[2]["content"], json!([{ "text": " " }]));
    }

    #[test]
    fn normalize_keeps_multi_member_blank_text_as_space() {
        let out = normalize_messages(vec![json!({
            "role": "assistant",
            "content": [{ "text": "", "toolUse": { "toolUseId": "t", "name": "n", "input": "" } }]
        })]);
        assert_eq!(
            out[0]["content"],
            json!([{ "text": " ", "toolUse": { "toolUseId": "t", "name": "n", "input": {} } }])
        );
    }

    #[test]
    fn normalize_fixes_empty_tool_input_and_removes_non_array_content() {
        let out = normalize_messages(vec![
            json!({ "role": "assistant", "content": [{ "toolUse": { "toolUseId": "t", "name": "n", "input": "" } }] }),
            json!({ "role": "user", "content": "plain string" }),
            json!({ "role": "user" }),
        ]);
        assert_eq!(out[0]["content"][0]["toolUse"]["input"], json!({}));
        assert!(out[1].get("content").is_none());
        assert!(out[2].get("content").is_none());
    }

    #[test]
    fn process_messages_rebuilds_image_blocks() {
        let out = process_messages(&[json!({
            "role": "user",
            "content": [
                { "image": { "format": "png", "source": { "bytes": "AQID", "extra": 1 }, "junk": true } },
                { "image": { "format": "png", "source": { "s3Location": { "uri": "s3://x" } } } },
                { "text": "t" }
            ]
        })]);
        assert_eq!(
            out[0]["content"][0],
            json!({ "image": { "format": "png", "source": { "bytes": "AQID" } } })
        );
        assert_eq!(
            out[0]["content"][1],
            json!({ "image": { "format": "png", "source": { "s3Location": { "uri": "s3://x" } } } })
        );
    }

    #[test]
    fn prepared_request_converts_to_sdk() {
        let mut r = req("anthropic.claude-sonnet-4");
        r.messages = vec![
            json!({ "role": "user", "content": [{ "text": "hi" }, { "image": { "format": "png", "source": { "bytes": { "0": 1 } } } }] }),
            json!({ "role": "assistant", "content": [
                { "reasoningContent": { "reasoningText": { "text": "hmm", "signature": "s" } } },
                { "toolUse": { "toolUseId": "t1", "name": "x", "input": { "a": 1 } } }
            ] }),
            json!({ "role": "user", "content": [
                { "toolResult": { "toolUseId": "t1", "content": [{ "json": { "ok": true } }] } },
                { "cachePoint": { "type": "default" } }
            ] }),
        ];
        r.system = Some(vec![
            json!({ "text": "s" }),
            json!({ "cachePoint": { "type": "default" } }),
        ]);
        r.tool_config = Some(
            json!({ "tools": [{ "toolSpec": { "name": "x", "inputSchema": { "json": {} } } }] }),
        );
        let p = prepare_request(&r, &settings(Some(("enabled", Some(4096)))), &claude_info());
        let sdk = p.to_sdk().unwrap();
        assert_eq!(sdk.messages.len(), 3);
        assert_eq!(sdk.messages[1].content.len(), 2);
        assert_eq!(sdk.system.as_ref().unwrap().len(), 2);
        assert_eq!(sdk.tool_config.as_ref().unwrap().tools.len(), 1);
        assert_eq!(sdk.inference_config.temperature, Some(1.0));
        assert!(sdk.additional_model_request_fields.is_some());
    }

    #[test]
    fn prepared_request_serializes_like_command_params() {
        let p = prepare_request(&req("meta.llama"), &settings(None), &NoInfo);
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({
                "modelId": "meta.llama",
                "messages": [{ "role": "user", "content": [{ "text": "hi" }] }],
                "system": [{ "text": "You are helpful." }],
                "inferenceConfig": { "maxTokens": 4096, "temperature": 0.5, "topP": 0.9 }
            })
        );
    }
}
