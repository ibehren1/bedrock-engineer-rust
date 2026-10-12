//! Rewrites saved references to retired models onto their replacements.
//!
//! Bedrock retires models; once one is removed from the registry, a config that
//! still names it would keep sending requests that fail. The table below is
//! explicit rather than "anything not in the registry", so it works no matter
//! the order in which the registry entries and this migration land, and so an
//! unknown ID the user typed (or an application inference profile ARN) is never
//! touched.
//!
//! Only settings are rewritten. Chat history and background session/execution
//! records are history and keep the model they actually ran on.

use models::{all_models, get_base_model_id, Llm};
use serde_json::{Map, Value};

/// Retired text model base ID -> replacement base ID.
const RETIRED_TEXT: &[(&str, &str)] = &[
    (
        "anthropic.claude-opus-4-20250514-v1:0",
        "anthropic.claude-opus-5-5",
    ),
    (
        "anthropic.claude-opus-4-1-20250805-v1:0",
        "anthropic.claude-opus-5-5",
    ),
    (
        "anthropic.claude-sonnet-4-20250514-v1:0",
        "anthropic.claude-sonnet-5-5",
    ),
    (
        "anthropic.claude-sonnet-4-5-20250929-v1:0",
        "anthropic.claude-sonnet-5-5",
    ),
    (
        "anthropic.claude-3-haiku-20240307-v1:0",
        "anthropic.claude-haiku-5-5",
    ),
    ("amazon.nova-premier-v1:0", "amazon.nova-2-lite-v1:0"),
];

/// Retired image model ID -> replacement. Image model IDs carry no profile prefix.
const RETIRED_IMAGE: &[(&str, &str)] = &[
    ("amazon.nova-canvas-v1:0", "stability.sd3-5-large-v1:0"),
    (
        "amazon.titan-image-generator-v1",
        "stability.sd3-5-large-v1:0",
    ),
    (
        "amazon.titan-image-generator-v2:0",
        "stability.sd3-5-large-v1:0",
    ),
    ("stability.sd3-large-v1:0", "stability.sd3-5-large-v1:0"),
    (
        "stability.stable-image-core-v1:1",
        "stability.sd3-5-large-v1:0",
    ),
    (
        "stability.stable-image-ultra-v1:1",
        "stability.sd3-5-large-v1:0",
    ),
];

/// Applies the remaps in place. A no-op (no key written) when nothing is retired.
pub(crate) fn migrate(data: &mut Map<String, Value>) {
    for key in ["llm", "lightProcessingModel"] {
        if let Some(llm) = data.get_mut(key) {
            migrate_llm(key, llm);
        }
    }

    if let Some(Value::String(id)) = data
        .get_mut("recognizeImageTool")
        .and_then(|t| t.get_mut("modelId"))
    {
        replace_string("recognizeImageTool.modelId", id, text_replacement_id);
    }
    if let Some(Value::String(id)) = data
        .get_mut("generateImageTool")
        .and_then(|t| t.get_mut("modelId"))
    {
        replace_string("generateImageTool.modelId", id, image_replacement);
    }

    if let Some(Value::Array(ids)) = data
        .get_mut("bedrockSettings")
        .and_then(|b| b.get_mut("visibleModelIds"))
    {
        migrate_allowlist(ids);
    }

    // Tasks keep their model at the top level; also cover a nested
    // `agentConfig.modelId` (the `ScheduleConfig` shape) in case one was stored.
    if let Some(Value::Array(tasks)) = data.get_mut("backgroundAgentScheduledTasks") {
        for task in tasks.iter_mut().filter_map(Value::as_object_mut) {
            if let Some(Value::String(id)) = task.get_mut("modelId") {
                replace_string(
                    "backgroundAgentScheduledTasks[].modelId",
                    id,
                    text_replacement_id,
                );
            }
            if let Some(Value::String(id)) = task
                .get_mut("agentConfig")
                .and_then(|c| c.get_mut("modelId"))
            {
                replace_string(
                    "backgroundAgentScheduledTasks[].agentConfig.modelId",
                    id,
                    text_replacement_id,
                );
            }
        }
    }
}

/// The registry entry that replaces a retired text model ID, keeping its
/// cross-region prefix when the replacement offers it, else `global.`, else the
/// replacement's first profile. `None` for IDs that are not retired.
fn text_replacement(model_id: &str) -> Option<&'static Llm> {
    let base = get_base_model_id(model_id);
    let (_, new_base) = RETIRED_TEXT.iter().find(|(old, _)| *old == base)?;
    let prefix = &model_id[..model_id.len() - base.len()];
    let offered: Vec<&Llm> = all_models()
        .iter()
        .filter(|m| get_base_model_id(&m.model_id) == *new_base)
        .collect();
    let with_prefix = |p: &str| {
        offered
            .iter()
            .copied()
            .find(|m| m.model_id == format!("{p}{new_base}"))
    };
    with_prefix(prefix)
        .or_else(|| with_prefix("global."))
        .or_else(|| offered.first().copied())
}

fn image_replacement(model_id: &str) -> Option<String> {
    RETIRED_IMAGE
        .iter()
        .find(|(old, _)| *old == model_id)
        .map(|(_, new)| (*new).to_string())
}

fn text_replacement_id(model_id: &str) -> Option<String> {
    text_replacement(model_id).map(|m| m.model_id.clone())
}

/// Replaces a whole LLM object, so name, limits and thinking types match the
/// replacement and not just the ID.
fn migrate_llm(key: &str, llm: &mut Value) {
    let Some(old) = llm.get("modelId").and_then(Value::as_str) else {
        return;
    };
    let Some(new) = text_replacement(old) else {
        return;
    };
    let Ok(value) = serde_json::to_value(new) else {
        return;
    };
    tracing::info!(key, from = old, to = %new.model_id, "Replaced retired model");
    *llm = value;
}

fn replace_string(key: &str, id: &mut String, replace: fn(&str) -> Option<String>) {
    if let Some(new) = replace(id) {
        tracing::info!(key, from = %id, to = %new, "Replaced retired model");
        *id = new;
    }
}

/// Remaps the chat model allowlist and drops duplicates (two retired models can
/// share a replacement, or the replacement may already be listed), keeping the
/// first occurrence's position.
fn migrate_allowlist(ids: &mut Vec<Value>) {
    let mut changed = false;
    for id in ids.iter_mut() {
        if let Value::String(s) = id {
            if let Some(new) = text_replacement_id(s) {
                tracing::info!(key = "bedrockSettings.visibleModelIds", from = %s, to = %new, "Replaced retired model");
                *s = new;
                changed = true;
            }
        }
    }
    if changed {
        let mut seen = Vec::with_capacity(ids.len());
        ids.retain(|v| {
            if seen.contains(v) {
                false
            } else {
                seen.push(v.clone());
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(v: Value) -> Map<String, Value> {
        let mut m = v.as_object().unwrap().clone();
        migrate(&mut m);
        m
    }

    fn llm(id: &str) -> Value {
        serde_json::to_value(all_models().iter().find(|m| m.model_id == id).unwrap()).unwrap()
    }

    fn stale_llm(id: &str) -> Value {
        json!({
            "modelId": id,
            "modelName": "Old Model",
            "toolUse": true,
            "regions": ["us-east-1"],
            "maxTokensLimit": 8192,
            "supportsThinking": false
        })
    }

    #[test]
    fn llm_object_fully_replaced_keeping_prefix() {
        let m = run(json!({ "llm": stale_llm("us.anthropic.claude-sonnet-4-20250514-v1:0") }));
        assert_eq!(m["llm"], llm("us.anthropic.claude-sonnet-5-5"));
        assert_ne!(m["llm"]["modelName"], json!("Old Model"));
        assert!(m["llm"]["regions"].as_array().unwrap().len() > 1);
    }

    #[test]
    fn light_processing_model_replaced_null_kept() {
        let m = run(
            json!({ "lightProcessingModel": stale_llm("global.anthropic.claude-opus-4-1-20250805-v1:0") }),
        );
        assert_eq!(
            m["lightProcessingModel"],
            llm("global.anthropic.claude-opus-5-5")
        );
        let m = run(json!({ "lightProcessingModel": null }));
        assert_eq!(m["lightProcessingModel"], Value::Null);
    }

    #[test]
    fn unoffered_prefix_falls_back_to_global() {
        // Claude Haiku 5.5 has no in-region or `apac.` profile.
        for old in [
            "anthropic.claude-3-haiku-20240307-v1:0",
            "apac.anthropic.claude-3-haiku-20240307-v1:0",
        ] {
            assert_eq!(
                text_replacement_id(old).as_deref(),
                Some("global.anthropic.claude-haiku-5-5")
            );
        }
        assert_eq!(
            text_replacement_id("jp.anthropic.claude-3-haiku-20240307-v1:0").as_deref(),
            Some("jp.anthropic.claude-haiku-5-5")
        );
        assert_eq!(
            text_replacement_id("us.amazon.nova-premier-v1:0").as_deref(),
            Some("us.amazon.nova-2-lite-v1:0")
        );
    }

    #[test]
    fn tool_model_ids_replaced() {
        let m = run(json!({
            "recognizeImageTool": { "modelId": "eu.anthropic.claude-sonnet-4-5-20250929-v1:0" },
            "generateImageTool": { "modelId": "amazon.titan-image-generator-v2:0" }
        }));
        assert_eq!(
            m["recognizeImageTool"]["modelId"],
            json!("eu.anthropic.claude-sonnet-5-5")
        );
        assert_eq!(
            m["generateImageTool"]["modelId"],
            json!("stability.sd3-5-large-v1:0")
        );
        for old in [
            "amazon.nova-canvas-v1:0",
            "amazon.titan-image-generator-v1",
            "stability.sd3-large-v1:0",
            "stability.stable-image-core-v1:1",
            "stability.stable-image-ultra-v1:1",
        ] {
            assert_eq!(
                image_replacement(old).as_deref(),
                Some("stability.sd3-5-large-v1:0")
            );
        }
    }

    #[test]
    fn allowlist_remapped_and_deduplicated_in_order() {
        let m = run(json!({ "bedrockSettings": {
            "enableRegionFailover": false,
            "visibleModelIds": [
                "global.anthropic.claude-opus-4-20250514-v1:0",
                "us.anthropic.claude-haiku-4-5-20251001-v1:0",
                "global.anthropic.claude-opus-4-1-20250805-v1:0",
                "global.anthropic.claude-opus-5-5",
                "anthropic.claude-3-haiku-20240307-v1:0"
            ]
        }}));
        assert_eq!(
            m["bedrockSettings"]["visibleModelIds"],
            json!([
                "global.anthropic.claude-opus-5-5",
                "us.anthropic.claude-haiku-4-5-20251001-v1:0",
                "global.anthropic.claude-haiku-5-5"
            ])
        );
        assert_eq!(m["bedrockSettings"]["enableRegionFailover"], json!(false));
    }

    #[test]
    fn scheduled_tasks_replaced() {
        let m = run(json!({ "backgroundAgentScheduledTasks": [
            { "id": "t1", "modelId": "us.anthropic.claude-sonnet-4-20250514-v1:0" },
            { "id": "t2", "modelId": "x", "agentConfig": { "modelId": "amazon.nova-premier-v1:0" } }
        ]}));
        let tasks = &m["backgroundAgentScheduledTasks"];
        assert_eq!(tasks[0]["modelId"], json!("us.anthropic.claude-sonnet-5-5"));
        assert_eq!(tasks[1]["modelId"], json!("x"));
        // Nova 2 Lite has no in-region ID, so the bare ID goes to `global.`.
        assert_eq!(
            tasks[1]["agentConfig"]["modelId"],
            json!("global.amazon.nova-2-lite-v1:0")
        );
    }

    #[test]
    fn non_retired_values_untouched() {
        let input = json!({
            "llm": stale_llm("global.anthropic.claude-opus-5-5"),
            "lightProcessingModel": { "modelId": "arn:aws:bedrock:us-east-1:1:application-inference-profile/x" },
            "recognizeImageTool": { "modelId": "us.anthropic.claude-haiku-4-5-20251001-v1:0" },
            "generateImageTool": { "modelId": "stability.sd3-5-large-v1:0" },
            "bedrockSettings": { "visibleModelIds": ["a", "a"] },
            "backgroundAgentScheduledTasks": [{ "modelId": "anthropic.claude" }]
        });
        assert_eq!(Value::Object(run(input.clone())), input);
    }

    #[test]
    fn idempotent() {
        let once = run(json!({
            "llm": stale_llm("anthropic.claude-3-haiku-20240307-v1:0"),
            "generateImageTool": { "modelId": "amazon.nova-canvas-v1:0" },
            "bedrockSettings": { "visibleModelIds": ["amazon.nova-premier-v1:0"] }
        }));
        let mut twice = once.clone();
        migrate(&mut twice);
        assert_eq!(once, twice);
        assert_eq!(once["llm"], llm("global.anthropic.claude-haiku-5-5"));
    }
}
