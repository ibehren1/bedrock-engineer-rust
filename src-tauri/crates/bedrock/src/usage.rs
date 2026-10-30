//! Usage metering: every billable call reports a [`UsageEvent`] to the process-wide
//! [`UsageSink`], which the app points at the usage ledger (`crates/usage`) at startup.
//!
//! The sink is global rather than passed to each service because several crates build their own
//! `ConverseService` (tools, structured output); one sink catches them all. With no sink set
//! (tests, tools run outside the app) events are dropped.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

/// What triggered a call, for the report's per-feature breakdown. The renderer sends it as
/// `usageContext` on Converse requests; Rust callers fill it in themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageContext {
    /// `chat`, `website`, `diagram`, `stepfunctions`, `background`, `subagent`, `title`,
    /// `structured`, `kb-ingest`, `kb-search`, ... Empty means unknown.
    #[serde(default)]
    pub feature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl UsageContext {
    pub fn feature(feature: &str) -> Self {
        Self {
            feature: feature.to_string(),
            ..Default::default()
        }
    }
}

/// The kind of billable call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageService {
    Converse,
    Embed,
    /// Image generation (priced per image).
    Image,
    /// Nova Reel (priced per second of video).
    Video,
    /// Bedrock Agents (`InvokeAgent`; tokens from the trace, priced by foundation model).
    Agent,
    /// `ApplyGuardrail` (priced per text unit and policy).
    Guardrail,
    /// Amazon Translate (priced per character).
    Translate,
    /// Knowledge Base `Retrieve` / `RetrieveAndGenerate` (no usage reported: counted, unpriced).
    Retrieve,
    /// `InvokeFlow` (no usage reported: counted, unpriced).
    Flow,
}

impl UsageService {
    pub const ALL: [UsageService; 9] = [
        UsageService::Converse,
        UsageService::Embed,
        UsageService::Image,
        UsageService::Video,
        UsageService::Agent,
        UsageService::Guardrail,
        UsageService::Translate,
        UsageService::Retrieve,
        UsageService::Flow,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            UsageService::Converse => "converse",
            UsageService::Embed => "embed",
            UsageService::Image => "image",
            UsageService::Video => "video",
            UsageService::Agent => "agent",
            UsageService::Guardrail => "guardrail",
            UsageService::Translate => "translate",
            UsageService::Retrieve => "retrieve",
            UsageService::Flow => "flow",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// What a non-token service bills by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    Images,
    VideoSeconds,
    Characters,
    GuardrailTextUnits,
}

impl UnitKind {
    pub const ALL: [UnitKind; 4] = [
        UnitKind::Images,
        UnitKind::VideoSeconds,
        UnitKind::Characters,
        UnitKind::GuardrailTextUnits,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            UnitKind::Images => "images",
            UnitKind::VideoSeconds => "video_seconds",
            UnitKind::Characters => "characters",
            UnitKind::GuardrailTextUnits => "guardrail_text_units",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// One billable call.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    /// Unix milliseconds.
    pub ts_ms: i64,
    pub service: UsageService,
    /// The model id as sent (may be a cross-region profile, an ARN or a prompt router).
    pub raw_model_id: String,
    /// The model that actually ran when Bedrock reports it (prompt routers).
    pub invoked_model_id: Option<String>,
    pub region: String,
    pub context: UsageContext,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Token counts were estimated rather than reported by the service.
    pub estimated: bool,
    /// Billed quantity of a non-token service (with `unit_kind`).
    pub units: f64,
    pub unit_kind: Option<UnitKind>,
    /// Bedrock calls this event stands for (1, or a batch's size when its calls are recorded
    /// together, as Titan embeddings are); tokens and units are the batch's totals.
    pub calls: u32,
}

impl UsageEvent {
    pub fn new(service: UsageService, model_id: &str, region: &str, context: UsageContext) -> Self {
        Self {
            ts_ms: now_ms(),
            service,
            raw_model_id: model_id.to_string(),
            invoked_model_id: None,
            region: region.to_string(),
            context,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated: false,
            units: 0.0,
            unit_kind: None,
            calls: 1,
        }
    }

    /// A non-token call: `units` of `kind` (images, seconds, characters, text units).
    pub fn units(
        service: UsageService,
        model_id: &str,
        region: &str,
        context: UsageContext,
        kind: UnitKind,
        units: f64,
    ) -> Self {
        let mut e = Self::new(service, model_id, region, context);
        e.unit_kind = Some(kind);
        e.units = units;
        e
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Receives usage events. `record` must not block for long: it runs on the request path.
pub trait UsageSink: Send + Sync {
    fn record(&self, event: UsageEvent);
}

static SINK: OnceLock<Arc<dyn UsageSink>> = OnceLock::new();

/// Install the process-wide sink. Only the first call takes effect.
pub fn set_sink(sink: Arc<dyn UsageSink>) {
    let _ = SINK.set(sink);
}

/// Report an event to the sink, if one is installed.
pub fn record(event: UsageEvent) {
    if let Some(sink) = SINK.get() {
        sink.record(event);
    }
}

/// Record a non-token call: `units` of `kind`, labeled with `feature`.
pub fn record_units(
    service: UsageService,
    id: &str,
    region: &str,
    feature: &str,
    kind: UnitKind,
    units: f64,
) {
    if units > 0.0 {
        record(UsageEvent::units(
            service,
            id,
            region,
            UsageContext::feature(feature),
            kind,
            units,
        ));
    }
}

/// Record a call that reports no usage (counted, never priced).
pub fn record_call(service: UsageService, id: &str, region: &str, feature: &str) {
    record(UsageEvent::new(
        service,
        id,
        region,
        UsageContext::feature(feature),
    ));
}

/// Guardrail text units per policy (`ApplyGuardrail`'s `usage`), as `guardrail:<policy>` ids.
/// Free units (regex sensitive-information filters) and image units are left out.
pub fn guardrail_units(usage: &Value) -> Vec<(&'static str, f64)> {
    [
        ("guardrail:content", "contentPolicyUnits"),
        ("guardrail:topic", "topicPolicyUnits"),
        ("guardrail:word", "wordPolicyUnits"),
        (
            "guardrail:sensitiveInformation",
            "sensitiveInformationPolicyUnits",
        ),
        (
            "guardrail:contextualGrounding",
            "contextualGroundingPolicyUnits",
        ),
        (
            "guardrail:automatedReasoning",
            "automatedReasoningPolicyUnits",
        ),
    ]
    .into_iter()
    .filter_map(|(id, key)| {
        let n = usage.get(key).and_then(Value::as_f64).unwrap_or(0.0);
        (n > 0.0).then_some((id, n))
    })
    .collect()
}

/// Tokens per foundation model in a Bedrock Agent's traces
/// (`trace.<kind>.modelInvocationOutput.metadata.usage`, paired with the same trace's
/// `modelInvocationInput.foundationModel`; `""` when the trace doesn't name the model).
pub fn agent_trace_usage(traces: &[Value]) -> Vec<(String, u64, u64)> {
    let mut by_model: Vec<(String, u64, u64)> = Vec::new();
    let mut last_model = String::new();
    for part in traces {
        let Some(kinds) = part.get("trace").and_then(Value::as_object) else {
            continue;
        };
        for step in kinds.values() {
            if let Some(m) = step
                .pointer("/modelInvocationInput/foundationModel")
                .and_then(Value::as_str)
            {
                last_model = m.to_string();
            }
            let Some(usage) = step.pointer("/modelInvocationOutput/metadata/usage") else {
                continue;
            };
            let (i, o) = (count(usage, "inputTokens"), count(usage, "outputTokens"));
            if i + o == 0 {
                continue;
            }
            match by_model.iter_mut().find(|(m, _, _)| *m == last_model) {
                Some(entry) => {
                    entry.1 += i;
                    entry.2 += o;
                }
                None => by_model.push((last_model.clone(), i, o)),
            }
        }
    }
    by_model
}

fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// Fill token counts from a Converse `usage` JSON object (`inputTokens`, `outputTokens`,
/// `cacheReadInputTokens`, `cacheWriteInputTokens`) and the prompt router's invoked model from
/// `trace`. `holder` is the Converse output or a stream `metadata` event body. Returns `false`
/// when there is no usage.
pub fn fill_from_converse(event: &mut UsageEvent, holder: &Value) -> bool {
    let Some(usage) = holder.get("usage").filter(|u| u.is_object()) else {
        return false;
    };
    event.input_tokens = count(usage, "inputTokens");
    event.output_tokens = count(usage, "outputTokens");
    event.cache_read_tokens = count(usage, "cacheReadInputTokens");
    event.cache_write_tokens = count(usage, "cacheWriteInputTokens");
    if let Some(routed) = holder
        .pointer("/trace/promptRouter/invokedModelId")
        .and_then(Value::as_str)
    {
        event.invoked_model_id = Some(routed.to_string());
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fills_tokens_and_router_model() {
        let mut e = UsageEvent::new(
            UsageService::Converse,
            "router",
            "us-east-1",
            Default::default(),
        );
        let holder = json!({
            "usage": { "inputTokens": 10, "outputTokens": 20, "cacheReadInputTokens": 5, "totalTokens": 35 },
            "trace": { "promptRouter": { "invokedModelId": "anthropic.claude-x" } }
        });
        assert!(fill_from_converse(&mut e, &holder));
        assert_eq!(
            (
                e.input_tokens,
                e.output_tokens,
                e.cache_read_tokens,
                e.cache_write_tokens
            ),
            (10, 20, 5, 0)
        );
        assert_eq!(e.invoked_model_id.as_deref(), Some("anthropic.claude-x"));
        assert!(!fill_from_converse(&mut e, &json!({ "metrics": {} })));
        // A model resolved beforehand (application inference profile) is kept.
        let mut e = UsageEvent::new(UsageService::Converse, "arn:x", "", Default::default());
        e.invoked_model_id = Some("anthropic.claude-y".into());
        fill_from_converse(&mut e, &json!({ "usage": { "inputTokens": 1 } }));
        assert_eq!(e.invoked_model_id.as_deref(), Some("anthropic.claude-y"));
    }

    #[test]
    fn guardrail_units_per_policy() {
        let usage = json!({
            "contentPolicyUnits": 3, "topicPolicyUnits": 0, "wordPolicyUnits": 2,
            "sensitiveInformationPolicyFreeUnits": 9, "contextualGroundingPolicyUnits": 1
        });
        assert_eq!(
            guardrail_units(&usage),
            vec![
                ("guardrail:content", 3.0),
                ("guardrail:word", 2.0),
                ("guardrail:contextualGrounding", 1.0)
            ]
        );
    }

    #[test]
    fn agent_traces_sum_per_model() {
        let traces = vec![
            json!({ "trace": { "orchestrationTrace": { "modelInvocationInput": { "foundationModel": "anthropic.claude-x" } } } }),
            json!({ "trace": { "orchestrationTrace": { "modelInvocationOutput": { "metadata": { "usage": { "inputTokens": 10, "outputTokens": 5 } } } } } }),
            json!({ "trace": { "orchestrationTrace": { "modelInvocationOutput": { "metadata": { "usage": { "inputTokens": 1, "outputTokens": 1 } } } } } }),
            json!({ "trace": { "preProcessingTrace": { "modelInvocationInput": { "foundationModel": "amazon.nova-pro-v1:0" }, "modelInvocationOutput": { "metadata": { "usage": { "inputTokens": 7, "outputTokens": 0 } } } } } }),
        ];
        assert_eq!(
            agent_trace_usage(&traces),
            vec![
                ("anthropic.claude-x".to_string(), 11, 6),
                ("amazon.nova-pro-v1:0".to_string(), 7, 0)
            ]
        );
    }

    #[test]
    fn context_round_trips_camel_case() {
        let c: UsageContext =
            serde_json::from_value(json!({ "feature": "chat", "agentId": "a", "sessionId": "s" }))
                .unwrap();
        assert_eq!(c.agent_id.as_deref(), Some("a"));
        assert_eq!(serde_json::to_value(&c).unwrap()["sessionId"], "s");
        let empty: UsageContext = serde_json::from_value(json!({})).unwrap();
        assert_eq!(empty, UsageContext::default());
    }
}
