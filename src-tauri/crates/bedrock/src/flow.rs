//! Bedrock Flows — port of `src/main/api/bedrock/services/flowService.ts` (`bedrock:invokeFlow`,
//! used by the `invokeFlow` tool).
//!
//! Output (`InvokeFlowResult`): `{ $metadata, executionId, flowStatus?, outputs, events,
//! requiresInput, promptId?, inputNodeName? }`, where `events` holds one
//! `{ flowCompletionEvent? | flowOutputEvent? | flowMultiTurnInputRequestEvent? | flowTraceEvent? }`
//! object per stream event and `outputs` the extracted `{ content: { document }, nodeName,
//! nodeOutputName }` items.

use crate::error::{Error, Result};
use crate::sdk::{
    decode_event_frames, object_of, required_str, timestamps_to_iso, JsonBody, RawBody,
    SdkConfigSource,
};
use crate::settings::AwsSettings;
use serde_json::{json, Map, Value};

/// Timestamp members in flow trace events.
pub const FLOW_TIMESTAMP_KEYS: [&str; 1] = ["timestamp"];

const FLOW_EVENT_KINDS: [&str; 4] = [
    "flowCompletionEvent",
    "flowOutputEvent",
    "flowMultiTurnInputRequestEvent",
    "flowTraceEvent",
];

/// The `bedrock:invokeFlow` handler's normalization: `inputs`, or the legacy single `input`
/// wrapped in an array, or `[]`.
pub fn invoke_flow_input_from_ipc(params: &Value) -> Value {
    let inputs = match (params.get("inputs"), params.get("input")) {
        (Some(inputs), _) if crate::sdk::truthy(Some(inputs)) => inputs.clone(),
        (_, Some(input)) if crate::sdk::truthy(Some(input)) => json!([input]),
        _ => json!([]),
    };
    let mut out = Map::new();
    for key in ["flowIdentifier", "flowAliasIdentifier"] {
        if let Some(v) = params.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    out.insert("inputs".into(), inputs);
    if let Some(v) = params.get("enableTrace") {
        out.insert("enableTrace".into(), v.clone());
    }
    Value::Object(out)
}

/// `typeof document === 'string' ? document : JSON.stringify(document) || ''`.
fn document_string(document: Option<&Value>) -> String {
    match document {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => serde_json::to_string(v).unwrap_or_default(),
    }
}

fn str_or<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}

/// `processResponseStream` over decoded `(event type, payload)` pairs (the stream succeeded).
pub fn process_flow_events(events: &[(String, Value)]) -> Map<String, Value> {
    let mut outputs = Vec::new();
    let mut recorded = Vec::new();
    let mut flow_status: Option<String> = None;
    let mut requires_input = false;
    let mut prompt_id: Option<Value> = None;
    let mut input_node_name: Option<String> = None;

    for (kind, payload) in events {
        if !FLOW_EVENT_KINDS.contains(&kind.as_str()) || payload.is_null() {
            continue;
        }
        let mut payload = payload.clone();
        match kind.as_str() {
            "flowCompletionEvent" => {
                flow_status = Some(str_or(&payload, "completionReason", "COMPLETED").to_string());
                let results = payload.get("outputResults").and_then(Value::as_array);
                for result in results.into_iter().flatten() {
                    if let Some(output) =
                        result.get("output").filter(|o| crate::sdk::truthy(Some(o)))
                    {
                        outputs.push(json!({
                            "content": { "document": document_string(output.pointer("/content/document")) },
                            "nodeName": str_or(output, "nodeName", ""),
                            "nodeOutputName": str_or(output, "nodeOutputName", ""),
                        }));
                    }
                }
            }
            "flowOutputEvent" => {
                let node_name = payload.get("nodeName").and_then(Value::as_str);
                let content = payload
                    .get("content")
                    .filter(|c| crate::sdk::truthy(Some(c)));
                if let (Some(content), Some(node_name)) =
                    (content, node_name.filter(|n| !n.is_empty()))
                {
                    outputs.push(json!({
                        "content": { "document": document_string(content.get("document")) },
                        "nodeName": node_name,
                        "nodeOutputName": str_or(&payload, "nodeOutputName", "document"),
                    }));
                }
            }
            "flowMultiTurnInputRequestEvent" => {
                requires_input = true;
                if let Some(id) = payload
                    .pointer("/inputPrompt/promptId")
                    .filter(|v| crate::sdk::truthy(Some(v)))
                {
                    prompt_id = Some(id.clone());
                }
                input_node_name = payload
                    .get("nodeName")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .map(str::to_string);
            }
            _ => timestamps_to_iso(&mut payload, &FLOW_TIMESTAMP_KEYS),
        }
        let mut event = Map::new();
        event.insert(kind.clone(), payload);
        recorded.push(Value::Object(event));
    }

    let mut out = Map::new();
    if let Some(s) = flow_status {
        out.insert("flowStatus".into(), json!(s));
    }
    out.insert("outputs".into(), Value::Array(outputs));
    out.insert("events".into(), Value::Array(recorded));
    out.insert("requiresInput".into(), json!(requires_input));
    if let Some(p) = prompt_id {
        out.insert("promptId".into(), p);
    }
    if let Some(n) = input_node_name {
        out.insert("inputNodeName".into(), json!(n));
    }
    out
}

/// The result `processResponseStream` returns for an expired/invalid flow session.
pub fn session_error_result() -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("flowStatus".into(), json!("SESSION_ERROR"));
    out.insert("outputs".into(), json!([]));
    out.insert("events".into(), json!([]));
    out.insert("requiresInput".into(), json!(false));
    out
}

fn is_session_context_error(e: &Error) -> bool {
    e.service_name() == Some("ValidationException")
        && e.message().contains("Error retrieving session context")
}

/// `invokeFlow(params)`. `params` is `InvokeFlowInput` JSON: `flowIdentifier`,
/// `flowAliasIdentifier`, `inputs` (`[{ content: { document }, nodeName, nodeOutputName }]`) and
/// `enableTrace` (default `false`).
pub async fn invoke_flow(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    params: &Value,
) -> Result<Value> {
    let m = object_of(params, "InvokeFlow input")?;
    let flow_id = required_str(m, "flowIdentifier")?;
    let alias_id = required_str(m, "flowAliasIdentifier")?;
    let body = JsonBody::new(&json!({
        "inputs": m.get("inputs").cloned().unwrap_or_else(|| json!([])),
        "enableTrace": m.get("enableTrace").and_then(Value::as_bool).unwrap_or(false),
    }));

    let result = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrockagentruntime::Client::new(&conf);
        let raw = RawBody::default();
        let mut out = client
            .invoke_flow()
            .flow_identifier(flow_id)
            .flow_alias_identifier(alias_id)
            .customize()
            .interceptor(body)
            .interceptor(raw.clone())
            .send()
            .await
            .map_err(Error::from)?;
        let mut drained = Ok(());
        loop {
            match out.response_stream.recv().await {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    drained = Err(Error::from(e));
                    break;
                }
            }
        }
        let processed = match drained {
            Ok(()) => process_flow_events(&decode_event_frames(&raw.bytes())),
            Err(e) if is_session_context_error(&e) => session_error_result(),
            Err(e) => {
                tracing::error!(error = %e, "Error processing response stream");
                return Err(e);
            }
        };
        let mut result = Map::new();
        result.insert("$metadata".into(), raw.metadata());
        result.insert(
            "executionId".into(),
            json!(out.execution_id.unwrap_or_default()),
        );
        result.extend(processed);
        Ok(Value::Object(result))
    }
    .await;
    if let Err(e) = &result {
        tracing::error!(error = %e, "Error invoking flow");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceError;

    fn ev(kind: &str, payload: Value) -> (String, Value) {
        (kind.to_string(), payload)
    }

    #[test]
    fn outputs_and_status_from_events() {
        let out = process_flow_events(&[
            ev(
                "flowTraceEvent",
                json!({ "trace": { "nodeInputTrace": { "timestamp": 1, "nodeName": "In" } } }),
            ),
            ev(
                "flowOutputEvent",
                json!({ "nodeName": "Out", "nodeType": "FlowOutputNode", "content": { "document": "hello" } }),
            ),
            ev(
                "flowOutputEvent",
                json!({ "nodeName": "Obj", "content": { "document": { "a": 1 } } }),
            ),
            ev(
                "flowOutputEvent",
                json!({ "content": { "document": "no node" } }),
            ),
            ev(
                "flowCompletionEvent",
                json!({ "completionReason": "SUCCESS" }),
            ),
        ]);
        assert_eq!(out["flowStatus"], "SUCCESS");
        assert_eq!(
            out["outputs"],
            json!([
                { "content": { "document": "hello" }, "nodeName": "Out", "nodeOutputName": "document" },
                { "content": { "document": "{\"a\":1}" }, "nodeName": "Obj", "nodeOutputName": "document" }
            ])
        );
        assert_eq!(out["events"].as_array().unwrap().len(), 5);
        assert_eq!(
            out["events"][0]["flowTraceEvent"]["trace"]["nodeInputTrace"]["timestamp"],
            "1970-01-01T00:00:01.000Z"
        );
        assert_eq!(
            out["events"][1]["flowOutputEvent"]["nodeType"],
            "FlowOutputNode"
        );
        assert_eq!(out["requiresInput"], false);
        assert!(out.get("promptId").is_none());
        assert!(out.get("inputNodeName").is_none());
    }

    #[test]
    fn completion_without_reason_and_multi_turn() {
        let out = process_flow_events(&[
            ev(
                "flowMultiTurnInputRequestEvent",
                json!({ "nodeName": "Agent", "content": { "document": "?" } }),
            ),
            ev("flowCompletionEvent", json!({})),
            ev("unknownEvent", json!({ "x": 1 })),
        ]);
        assert_eq!(out["flowStatus"], "COMPLETED");
        assert_eq!(out["requiresInput"], true);
        assert_eq!(out["inputNodeName"], "Agent");
        assert_eq!(out["events"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn session_errors_are_recognized() {
        let e = Error::Service(ServiceError::new(
            "ValidationException",
            "Error retrieving session context for x",
        ));
        assert!(is_session_context_error(&e));
        assert!(!is_session_context_error(&Error::Service(
            ServiceError::new("ValidationException", "other")
        )));
        assert_eq!(
            Value::Object(session_error_result()),
            json!({ "flowStatus": "SESSION_ERROR", "outputs": [], "events": [], "requiresInput": false })
        );
    }

    #[test]
    fn ipc_normalization() {
        let input = json!({ "content": { "document": "d" }, "nodeName": "FlowInputNode", "nodeOutputName": "document" });
        let out = invoke_flow_input_from_ipc(
            &json!({ "flowIdentifier": "F", "flowAliasIdentifier": "A", "input": input }),
        );
        assert_eq!(out["inputs"], json!([input]));
        let out = invoke_flow_input_from_ipc(
            &json!({ "flowIdentifier": "F", "flowAliasIdentifier": "A" }),
        );
        assert_eq!(out["inputs"], json!([]));
        let out = invoke_flow_input_from_ipc(
            &json!({ "flowIdentifier": "F", "flowAliasIdentifier": "A", "inputs": [input], "enableTrace": true }),
        );
        assert_eq!(out["inputs"][0], input);
        assert_eq!(out["enableTrace"], true);
    }
}
