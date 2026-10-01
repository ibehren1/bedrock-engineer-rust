//! SDK Converse outputs -> the exact JSON the Express server wrote.
//!
//! `POST /converse/stream` wrote one line per event: `JSON.stringify(item) + '\n'`, where `item`
//! is a JS SDK `ConverseStreamOutput` union member. [`stream_event_to_json`] produces the same
//! object for each event (one top-level key naming the event). Shapes:
//!
//! ```text
//! {"messageStart":{"role":"assistant"}}
//! {"contentBlockStart":{"contentBlockIndex":1,"start":{"toolUse":{"name":"readFiles","toolUseId":"tooluse_x"}}}}
//! {"contentBlockDelta":{"contentBlockIndex":0,"delta":{"text":"Hel"}}}
//! {"contentBlockDelta":{"contentBlockIndex":1,"delta":{"toolUse":{"input":"{\"pa"}}}}
//! {"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"text":"Let me"}}}}
//! {"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"signature":"EqQB..."}}}}
//! {"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"redactedContent":{"0":12,"1":34}}}}}
//! {"contentBlockStop":{"contentBlockIndex":0}}
//! {"messageStop":{"stopReason":"tool_use"}}
//! {"metadata":{"metrics":{"latencyMs":812},"usage":{"cacheReadInputTokens":0,"cacheWriteInputTokens":0,"inputTokens":10,"outputTokens":20,"totalTokens":30}}}
//! ```
//!
//! Keys are alphabetical within each object (the JS SDK deserializer's order) and absent
//! optionals are omitted. Blobs are `Uint8Array` JSON (`{"0":..}`), see [`crate::document`].
//!
//! [`converse_output_to_json`] renders the non-streaming `POST /converse` response body
//! (`res.json(result)` of `ConverseCommandOutput`).

use crate::convert::{message_to_json, object};
use crate::document::{blob_to_json, document_to_json};
use aws_sdk_bedrockruntime::operation::converse::ConverseOutput as ConverseResponse;
use aws_sdk_bedrockruntime::types::{
    CitationLocation, ContentBlockDelta, ContentBlockStart, ConverseOutput, ConverseStreamOutput,
    GuardrailTraceAssessment, ImageSource, PerformanceConfiguration, PromptRouterTrace,
    ReasoningContentBlockDelta, ServiceTier, TokenUsage, ToolResultBlockDelta,
};
use serde_json::{json, Value};

/// One stream event -> JSON. `None` for events the SDK could not identify (`Unknown`), which the
/// JS SDK would have surfaced as `{ $unknown: ... }` and the renderer ignored.
pub fn stream_event_to_json(event: &ConverseStreamOutput) -> Option<Value> {
    Some(match event {
        ConverseStreamOutput::MessageStart(e) => {
            json!({ "messageStart": { "role": e.role.as_str() } })
        }
        ConverseStreamOutput::ContentBlockStart(e) => json!({
            "contentBlockStart": object(vec![
                ("contentBlockIndex", Some(json!(e.content_block_index))),
                ("start", e.start.as_ref().and_then(block_start_to_json)),
            ])
        }),
        ConverseStreamOutput::ContentBlockDelta(e) => json!({
            "contentBlockDelta": object(vec![
                ("contentBlockIndex", Some(json!(e.content_block_index))),
                ("delta", e.delta.as_ref().and_then(block_delta_to_json)),
            ])
        }),
        ConverseStreamOutput::ContentBlockStop(e) => {
            json!({ "contentBlockStop": { "contentBlockIndex": e.content_block_index } })
        }
        ConverseStreamOutput::MessageStop(e) => json!({
            "messageStop": object(vec![
                (
                    "additionalModelResponseFields",
                    e.additional_model_response_fields.as_ref().map(document_to_json),
                ),
                ("stopReason", Some(json!(e.stop_reason.as_str()))),
            ])
        }),
        ConverseStreamOutput::Metadata(e) => json!({
            "metadata": object(vec![
                ("metrics", e.metrics.as_ref().map(|m| json!({ "latencyMs": m.latency_ms }))),
                ("performanceConfig", e.performance_config.as_ref().map(performance_to_json)),
                ("serviceTier", e.service_tier.as_ref().map(service_tier_to_json)),
                (
                    "trace",
                    e.trace
                        .as_ref()
                        .map(|t| trace_to_json(t.guardrail.as_ref(), t.prompt_router.as_ref())),
                ),
                ("usage", e.usage.as_ref().map(usage_to_json)),
            ])
        }),
        _ => return None,
    })
}

fn block_start_to_json(start: &ContentBlockStart) -> Option<Value> {
    Some(match start {
        ContentBlockStart::ToolUse(t) => json!({
            "toolUse": object(vec![
                ("name", Some(json!(t.name))),
                ("toolUseId", Some(json!(t.tool_use_id))),
                ("type", t.r#type.as_ref().map(|x| json!(x.as_str()))),
            ])
        }),
        ContentBlockStart::ToolResult(t) => json!({
            "toolResult": object(vec![
                ("status", t.status.as_ref().map(|s| json!(s.as_str()))),
                ("toolUseId", Some(json!(t.tool_use_id))),
                ("type", t.r#type.clone().map(Value::from)),
            ])
        }),
        ContentBlockStart::Image(i) => json!({ "image": { "format": i.format.as_str() } }),
        _ => return None,
    })
}

fn block_delta_to_json(delta: &ContentBlockDelta) -> Option<Value> {
    Some(match delta {
        ContentBlockDelta::Text(t) => json!({ "text": t }),
        ContentBlockDelta::ToolUse(t) => json!({ "toolUse": { "input": t.input } }),
        ContentBlockDelta::ReasoningContent(r) => json!({
            "reasoningContent": match r {
                ReasoningContentBlockDelta::Text(t) => json!({ "text": t }),
                ReasoningContentBlockDelta::Signature(s) => json!({ "signature": s }),
                ReasoningContentBlockDelta::RedactedContent(b) => {
                    json!({ "redactedContent": blob_to_json(b) })
                }
                _ => return None,
            }
        }),
        ContentBlockDelta::ToolResult(items) => json!({
            "toolResult": items.iter().filter_map(|i| match i {
                ToolResultBlockDelta::Text(t) => Some(json!({ "text": t })),
                ToolResultBlockDelta::Json(d) => Some(json!({ "json": document_to_json(d) })),
                _ => None,
            }).collect::<Vec<_>>()
        }),
        ContentBlockDelta::Image(i) => json!({
            "image": object(vec![
                (
                    "error",
                    i.error.as_ref().map(|e| object(vec![("message", e.message.clone().map(Value::from))])),
                ),
                (
                    "source",
                    i.source.as_ref().and_then(|s| match s {
                        ImageSource::Bytes(b) => Some(json!({ "bytes": blob_to_json(b) })),
                        _ => None,
                    }),
                ),
            ])
        }),
        ContentBlockDelta::Citation(c) => json!({
            "citation": object(vec![
                ("location", c.location.as_ref().and_then(citation_location_to_json)),
                ("source", c.source.clone().map(Value::from)),
                (
                    "sourceContent",
                    c.source_content.as_ref().map(|sc| {
                        Value::Array(
                            sc.iter()
                                .map(|s| object(vec![("text", s.text.clone().map(Value::from))]))
                                .collect(),
                        )
                    }),
                ),
                ("title", c.title.clone().map(Value::from)),
            ])
        }),
        _ => return None,
    })
}

fn citation_location_to_json(loc: &CitationLocation) -> Option<Value> {
    let int = |v: Option<i32>| v.map(Value::from);
    Some(match loc {
        CitationLocation::DocumentChar(l) => json!({ "documentChar": object(vec![
            ("documentIndex", int(l.document_index)), ("end", int(l.end)), ("start", int(l.start)),
        ]) }),
        CitationLocation::DocumentChunk(l) => json!({ "documentChunk": object(vec![
            ("documentIndex", int(l.document_index)), ("end", int(l.end)), ("start", int(l.start)),
        ]) }),
        CitationLocation::DocumentPage(l) => json!({ "documentPage": object(vec![
            ("documentIndex", int(l.document_index)), ("end", int(l.end)), ("start", int(l.start)),
        ]) }),
        CitationLocation::Web(w) => json!({ "web": object(vec![
            ("domain", w.domain.clone().map(Value::from)), ("url", w.url.clone().map(Value::from)),
        ]) }),
        _ => return None,
    })
}

/// `TokenUsage` -> `{ cacheDetails?, cacheReadInputTokens?, cacheWriteInputTokens?, inputTokens, outputTokens, totalTokens }`.
pub fn usage_to_json(u: &TokenUsage) -> Value {
    object(vec![
        (
            "cacheDetails",
            u.cache_details.as_ref().map(|ds| {
                Value::Array(
                    ds.iter()
                        .map(|d| json!({ "inputTokens": d.input_tokens, "ttl": d.ttl.as_str() }))
                        .collect(),
                )
            }),
        ),
        (
            "cacheReadInputTokens",
            u.cache_read_input_tokens.map(Value::from),
        ),
        (
            "cacheWriteInputTokens",
            u.cache_write_input_tokens.map(Value::from),
        ),
        ("inputTokens", Some(json!(u.input_tokens))),
        ("outputTokens", Some(json!(u.output_tokens))),
        ("totalTokens", Some(json!(u.total_tokens))),
    ])
}

fn performance_to_json(p: &PerformanceConfiguration) -> Value {
    json!({ "latency": p.latency.as_str() })
}

fn service_tier_to_json(s: &ServiceTier) -> Value {
    json!({ "type": s.r#type.as_str() })
}

/// `trace`: `promptRouter.invokedModelId` and the top-level guardrail fields
/// (`actionReason`, `modelOutput`). Per-policy guardrail assessments are not rendered.
fn trace_to_json(
    guardrail: Option<&GuardrailTraceAssessment>,
    router: Option<&PromptRouterTrace>,
) -> Value {
    object(vec![
        (
            "guardrail",
            guardrail.map(|g| {
                object(vec![
                    ("actionReason", g.action_reason.clone().map(Value::from)),
                    (
                        "modelOutput",
                        g.model_output
                            .as_ref()
                            .map(|m| Value::Array(m.iter().cloned().map(Value::from).collect())),
                    ),
                ])
            }),
        ),
        (
            "promptRouter",
            router.map(|r| {
                object(vec![(
                    "invokedModelId",
                    r.invoked_model_id.clone().map(Value::from),
                )])
            }),
        ),
    ])
}

/// Non-streaming response body, as `res.json(ConverseCommandOutput)` produced it:
/// `{ $metadata, additionalModelResponseFields?, metrics, output: { message }, performanceConfig?,
/// serviceTier?, stopReason, trace?, usage }`.
///
/// `$metadata` carries `httpStatusCode: 200`, `requestId` (when known), `attempts` and
/// `totalRetryDelay` (always `1` / `0` here: SDK-internal retry counts are not exposed).
pub fn converse_output_to_json(out: &ConverseResponse, request_id: Option<&str>) -> Value {
    object(vec![
        (
            "$metadata",
            Some(crate::sdk::metadata_json(200, request_id)),
        ),
        (
            "additionalModelResponseFields",
            out.additional_model_response_fields
                .as_ref()
                .map(document_to_json),
        ),
        (
            "metrics",
            out.metrics
                .as_ref()
                .map(|m| json!({ "latencyMs": m.latency_ms })),
        ),
        (
            "output",
            out.output.as_ref().and_then(|o| match o {
                ConverseOutput::Message(m) => Some(json!({ "message": message_to_json(m) })),
                _ => None,
            }),
        ),
        (
            "performanceConfig",
            out.performance_config.as_ref().map(performance_to_json),
        ),
        (
            "serviceTier",
            out.service_tier.as_ref().map(service_tier_to_json),
        ),
        ("stopReason", Some(json!(out.stop_reason.as_str()))),
        (
            "trace",
            out.trace
                .as_ref()
                .map(|t| trace_to_json(t.guardrail.as_ref(), t.prompt_router.as_ref())),
        ),
        ("usage", out.usage.as_ref().map(usage_to_json)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{
        CacheDetail, CacheTtl, CitationSourceContentDelta, CitationsDelta, ContentBlock,
        ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent, ConversationRole,
        ConverseMetrics, ConverseStreamMetadataEvent, ConverseStreamMetrics, ConverseStreamTrace,
        Message, MessageStartEvent, MessageStopEvent, StopReason, ToolUseBlock, ToolUseBlockDelta,
        ToolUseBlockStart, WebLocation,
    };
    use aws_smithy_types::{Blob, Document};

    fn line(event: ConverseStreamOutput) -> String {
        serde_json::to_string(&stream_event_to_json(&event).unwrap()).unwrap()
    }

    fn delta(d: ContentBlockDelta, idx: i32) -> ConverseStreamOutput {
        ConverseStreamOutput::ContentBlockDelta(
            ContentBlockDeltaEvent::builder()
                .delta(d)
                .content_block_index(idx)
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn message_start_and_stop() {
        let start = ConverseStreamOutput::MessageStart(
            MessageStartEvent::builder()
                .role(ConversationRole::Assistant)
                .build()
                .unwrap(),
        );
        assert_eq!(line(start), r#"{"messageStart":{"role":"assistant"}}"#);

        let stop = ConverseStreamOutput::MessageStop(
            MessageStopEvent::builder()
                .stop_reason(StopReason::ToolUse)
                .build()
                .unwrap(),
        );
        assert_eq!(line(stop), r#"{"messageStop":{"stopReason":"tool_use"}}"#);

        let stop = ConverseStreamOutput::MessageStop(
            MessageStopEvent::builder()
                .stop_reason(StopReason::EndTurn)
                .additional_model_response_fields(Document::Object(
                    [("x".to_string(), Document::Bool(true))]
                        .into_iter()
                        .collect(),
                ))
                .build()
                .unwrap(),
        );
        assert_eq!(
            line(stop),
            r#"{"messageStop":{"additionalModelResponseFields":{"x":true},"stopReason":"end_turn"}}"#
        );
    }

    #[test]
    fn text_and_tool_use_deltas() {
        assert_eq!(
            line(delta(ContentBlockDelta::Text("Hel".into()), 0)),
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"text":"Hel"}}}"#
        );
        assert_eq!(
            line(delta(
                ContentBlockDelta::ToolUse(
                    ToolUseBlockDelta::builder().input("{\"pa").build().unwrap()
                ),
                1
            )),
            r#"{"contentBlockDelta":{"contentBlockIndex":1,"delta":{"toolUse":{"input":"{\"pa"}}}}"#
        );
    }

    #[test]
    fn reasoning_deltas_with_signature_and_redacted() {
        assert_eq!(
            line(delta(
                ContentBlockDelta::ReasoningContent(ReasoningContentBlockDelta::Text(
                    "Let me".into()
                )),
                0
            )),
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"text":"Let me"}}}}"#
        );
        assert_eq!(
            line(delta(
                ContentBlockDelta::ReasoningContent(ReasoningContentBlockDelta::Signature(
                    "EqQB".into()
                )),
                0
            )),
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"signature":"EqQB"}}}}"#
        );
        assert_eq!(
            line(delta(
                ContentBlockDelta::ReasoningContent(ReasoningContentBlockDelta::RedactedContent(
                    Blob::new(vec![12u8, 34])
                )),
                0
            )),
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"redactedContent":{"0":12,"1":34}}}}}"#
        );
    }

    #[test]
    fn tool_use_start_and_stop() {
        let start = ConverseStreamOutput::ContentBlockStart(
            ContentBlockStartEvent::builder()
                .start(ContentBlockStart::ToolUse(
                    ToolUseBlockStart::builder()
                        .tool_use_id("tooluse_x")
                        .name("readFiles")
                        .build()
                        .unwrap(),
                ))
                .content_block_index(1)
                .build()
                .unwrap(),
        );
        assert_eq!(
            line(start),
            r#"{"contentBlockStart":{"contentBlockIndex":1,"start":{"toolUse":{"name":"readFiles","toolUseId":"tooluse_x"}}}}"#
        );
        let stop = ConverseStreamOutput::ContentBlockStop(
            ContentBlockStopEvent::builder()
                .content_block_index(1)
                .build()
                .unwrap(),
        );
        assert_eq!(
            line(stop),
            r#"{"contentBlockStop":{"contentBlockIndex":1}}"#
        );
    }

    #[test]
    fn start_without_payload_omits_start() {
        let start = ConverseStreamOutput::ContentBlockStart(
            ContentBlockStartEvent::builder()
                .content_block_index(0)
                .build()
                .unwrap(),
        );
        assert_eq!(
            line(start),
            r#"{"contentBlockStart":{"contentBlockIndex":0}}"#
        );
    }

    #[test]
    fn metadata_with_cache_usage_and_trace() {
        let md = ConverseStreamOutput::Metadata(
            ConverseStreamMetadataEvent::builder()
                .usage(
                    TokenUsage::builder()
                        .input_tokens(10)
                        .output_tokens(20)
                        .total_tokens(30)
                        .cache_read_input_tokens(5)
                        .cache_write_input_tokens(0)
                        .build()
                        .unwrap(),
                )
                .metrics(
                    ConverseStreamMetrics::builder()
                        .latency_ms(812)
                        .build()
                        .unwrap(),
                )
                .trace(
                    ConverseStreamTrace::builder()
                        .prompt_router(PromptRouterTrace::builder().invoked_model_id("m").build())
                        .build(),
                )
                .build(),
        );
        assert_eq!(
            line(md),
            r#"{"metadata":{"metrics":{"latencyMs":812},"trace":{"promptRouter":{"invokedModelId":"m"}},"usage":{"cacheReadInputTokens":5,"cacheWriteInputTokens":0,"inputTokens":10,"outputTokens":20,"totalTokens":30}}}"#
        );
    }

    #[test]
    fn usage_cache_details() {
        let u = TokenUsage::builder()
            .input_tokens(1)
            .output_tokens(2)
            .total_tokens(3)
            .cache_details(
                CacheDetail::builder()
                    .ttl(CacheTtl::FiveMinutes)
                    .input_tokens(7)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        assert_eq!(
            usage_to_json(&u)["cacheDetails"],
            json!([{ "inputTokens": 7, "ttl": "5m" }])
        );
    }

    #[test]
    fn citation_delta() {
        let d = ContentBlockDelta::Citation(
            CitationsDelta::builder()
                .title("t")
                .source_content(CitationSourceContentDelta::builder().text("quote").build())
                .location(CitationLocation::Web(
                    WebLocation::builder().url("https://x").build(),
                ))
                .build(),
        );
        assert_eq!(
            stream_event_to_json(&delta(d, 2)).unwrap(),
            json!({ "contentBlockDelta": { "contentBlockIndex": 2, "delta": { "citation": {
                "location": { "web": { "url": "https://x" } },
                "sourceContent": [{ "text": "quote" }],
                "title": "t"
            } } } })
        );
    }

    #[test]
    fn non_streaming_output_body() {
        let out = ConverseResponse::builder()
            .output(ConverseOutput::Message(
                Message::builder()
                    .role(ConversationRole::Assistant)
                    .content(ContentBlock::Text("hi".into()))
                    .content(ContentBlock::ToolUse(
                        ToolUseBlock::builder()
                            .tool_use_id("t1")
                            .name("x")
                            .input(Document::Object(Default::default()))
                            .build()
                            .unwrap(),
                    ))
                    .build()
                    .unwrap(),
            ))
            .stop_reason(StopReason::ToolUse)
            .usage(
                TokenUsage::builder()
                    .input_tokens(1)
                    .output_tokens(2)
                    .total_tokens(3)
                    .build()
                    .unwrap(),
            )
            .metrics(ConverseMetrics::builder().latency_ms(5).build().unwrap())
            .build()
            .unwrap();
        assert_eq!(
            converse_output_to_json(&out, Some("rid")),
            json!({
                "$metadata": { "httpStatusCode": 200, "requestId": "rid", "attempts": 1, "totalRetryDelay": 0 },
                "metrics": { "latencyMs": 5 },
                "output": { "message": { "content": [
                    { "text": "hi" },
                    { "toolUse": { "input": {}, "name": "x", "toolUseId": "t1" } }
                ], "role": "assistant" } },
                "stopReason": "tool_use",
                "usage": { "inputTokens": 1, "outputTokens": 2, "totalTokens": 3 }
            })
        );
    }
}
