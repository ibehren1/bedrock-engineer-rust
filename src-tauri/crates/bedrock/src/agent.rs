//! Bedrock Agents and Knowledge Bases — port of `src/main/api/bedrock/services/agentService.ts`.
//!
//! * [`invoke_agent`] — `bedrock:invokeAgent` (the `invokeBedrockAgent` tool). The response
//!   stream is aggregated exactly like `readStreamResponse`: chunk text concatenated, files
//!   de-duplicated by name, trace parts collected.
//! * [`retrieve`] — `bedrock:retrieve` (the `retrieve` tool); [`retrieve_input_from_ipc`] does
//!   the handler's `{ knowledgeBaseId, query, retrievalConfiguration? }` translation.
//! * [`retrieve_and_generate`] — the `retrieve_and_generate` command (`POST /retrieveAndGenerate`).
//!
//! Inputs are the SDK command input JSON; outputs are the SDK output JSON (`$metadata` + the
//! service response), see [`crate::sdk`] for how that is kept faithful.

use crate::document::bytes_to_index_object;
use crate::error::{Error, Result};
use crate::retry::random_index;
use crate::sdk::{
    blobs_in_list, decode_event_frames, object_of, required_str, timestamps_to_iso, truthy,
    JsonBody, RawBody, SdkConfigSource,
};
use crate::settings::AwsSettings;
use base64::Engine;
use serde::{Serialize, Serializer};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// Timestamp members inside agent trace parts (`TracePart.eventTime`, `Metadata.startTime` /
/// `endTime`); the JS SDK turned them into `Date`s.
pub const TRACE_TIMESTAMP_KEYS: [&str; 3] = ["eventTime", "startTime", "endTime"];

/// A file returned by the agent (code interpreter output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentFile {
    pub name: String,
    /// Raw bytes; serialized as `JSON.stringify(Uint8Array)` does (`{"0":n,...}`).
    #[serde(serialize_with = "index_object")]
    pub content: Vec<u8>,
}

fn index_object<S: Serializer>(bytes: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
    bytes_to_index_object(bytes).serialize(s)
}

/// `Completion`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Completion {
    pub message: String,
    pub files: Vec<AgentFile>,
    /// `TracePart` objects as sent by the service (timestamps as ISO strings).
    pub traces: Vec<Value>,
}

/// `InvokeAgentResult`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvokeAgentResult {
    #[serde(rename = "$metadata")]
    pub metadata: Value,
    pub content_type: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<Completion>,
}

/// `generateSessionId()`: `session_<ms since epoch>_<7 base-36 chars>`.
pub fn generate_session_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let suffix: String = (0..7).map(|_| DIGITS[random_index(36)] as char).collect();
    format!("session_{ms}_{suffix}")
}

/// Members of `InvokeAgentCommandInput` bound to the URI or headers rather than the body.
const INVOKE_AGENT_HTTP_MEMBERS: [&str; 4] = ["agentId", "agentAliasId", "sessionId", "sourceArn"];

/// The InvokeAgent request body: everything but the URI/header members, `enableTrace` defaulted
/// to `false`, file bytes in `sessionState.files` as base64.
pub fn invoke_agent_body(input: &Map<String, Value>) -> Result<Value> {
    let mut body: Map<String, Value> = input
        .iter()
        .filter(|(k, v)| !INVOKE_AGENT_HTTP_MEMBERS.contains(&k.as_str()) && !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if !body.contains_key("enableTrace") {
        body.insert("enableTrace".into(), json!(false));
    }
    let mut body = Value::Object(body);
    if let Some(state) = body.get_mut("sessionState") {
        blobs_in_list(state, &["files"], &["source", "byteContent", "data"])?;
    }
    Ok(body)
}

/// `readStreamResponse` over decoded `(event type, payload)` pairs.
pub fn aggregate_agent_events(events: &[(String, Value)]) -> Completion {
    let mut out = Completion::default();
    let mut seen = HashSet::new();
    let b64 = base64::engine::general_purpose::STANDARD;
    for (kind, payload) in events {
        match kind.as_str() {
            "trace" if truthy(payload.get("trace")) => {
                let mut part = payload.clone();
                timestamps_to_iso(&mut part, &TRACE_TIMESTAMP_KEYS);
                out.traces.push(part);
            }
            "chunk" => {
                if let Some(bytes) = payload
                    .get("bytes")
                    .and_then(Value::as_str)
                    .and_then(|s| b64.decode(s).ok())
                {
                    // `new TextDecoder().decode(bytes)` per chunk.
                    out.message.push_str(&String::from_utf8_lossy(&bytes));
                }
            }
            "files" => {
                let files = payload.get("files").and_then(Value::as_array);
                for file in files.into_iter().flatten() {
                    let name = file.get("name").and_then(Value::as_str).unwrap_or("");
                    // The same file can appear several times; keep the first.
                    if !seen.insert(name.to_string()) {
                        continue;
                    }
                    let bytes = file
                        .get("bytes")
                        .and_then(Value::as_str)
                        .and_then(|s| b64.decode(s).ok());
                    if let (false, Some(content)) = (name.is_empty(), bytes) {
                        out.files.push(AgentFile {
                            name: name.to_string(),
                            content,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `invokeAgent(params)`. `params` is `InvokeAgentInput` JSON: `agentId`, `agentAliasId`,
/// `inputText`, optional `sessionId` (generated when absent), `enableTrace` (default `false`) and
/// any other `InvokeAgentCommandInput` member (`sessionState`, `endSession`, `memoryId`, ...).
pub async fn invoke_agent(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    params: &Value,
) -> Result<InvokeAgentResult> {
    let m = object_of(params, "InvokeAgent input")?;
    let agent_id = required_str(m, "agentId")?;
    let agent_alias_id = required_str(m, "agentAliasId")?;
    let session_id = m
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(generate_session_id);
    let source_arn = m
        .get("sourceArn")
        .and_then(Value::as_str)
        .map(str::to_string);
    let body = JsonBody::new(&invoke_agent_body(m)?);

    let result = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrockagentruntime::Client::new(&conf);
        let raw = RawBody::default();
        let mut out = client
            .invoke_agent()
            .agent_id(agent_id)
            .agent_alias_id(agent_alias_id)
            .session_id(session_id)
            .set_source_arn(source_arn)
            .customize()
            .interceptor(body)
            .interceptor(raw.clone())
            .send()
            .await
            .map_err(Error::from)?;
        // Drive the stream to the end; errors inside it fail the call like the TS for-await.
        while out.completion.recv().await.map_err(Error::from)?.is_some() {}
        let events = decode_event_frames(&raw.bytes());
        Ok(InvokeAgentResult {
            metadata: raw.metadata(),
            content_type: out.content_type,
            session_id: out.session_id,
            completion: Some(aggregate_agent_events(&events)),
        })
    }
    .await;
    if let Err(e) = &result {
        tracing::error!(error = %e, "Error invoking agent");
    }
    result
}

/// The `bedrock:retrieve` handler's translation: `{ knowledgeBaseId, query,
/// retrievalConfiguration? }` → `RetrieveCommandInput`.
pub fn retrieve_input_from_ipc(params: &Value) -> Value {
    let mut out = Map::new();
    out.insert(
        "knowledgeBaseId".into(),
        params
            .get("knowledgeBaseId")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "retrievalQuery".into(),
        json!({ "text": params.get("query").cloned().unwrap_or(Value::Null) }),
    );
    if let Some(cfg) = params
        .get("retrievalConfiguration")
        .filter(|v| truthy(Some(v)))
    {
        out.insert("retrievalConfiguration".into(), cfg.clone());
    }
    Value::Object(out)
}

fn body_without(input: &Map<String, Value>, http_members: &[&str]) -> Map<String, Value> {
    input
        .iter()
        .filter(|(k, v)| !http_members.contains(&k.as_str()) && !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn with_metadata(raw: &RawBody) -> Value {
    let mut out = Map::new();
    out.insert("$metadata".into(), raw.metadata());
    out.extend(raw.json_object());
    Value::Object(out)
}

/// `retrieve(props)`: `RetrieveCommandInput` JSON → `RetrieveCommandOutput` JSON
/// (`{ $metadata, retrievalResults, guardrailAction?, nextToken? }`).
pub async fn retrieve(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    input: &Value,
) -> Result<Value> {
    let m = object_of(input, "Retrieve input")?;
    let knowledge_base_id = required_str(m, "knowledgeBaseId")?;
    let body = JsonBody::new(&Value::Object(body_without(m, &["knowledgeBaseId"])));
    let conf = configs.sdk_config(aws).await?;
    let client = aws_sdk_bedrockagentruntime::Client::new(&conf);
    let raw = RawBody::default();
    client
        .retrieve()
        .knowledge_base_id(knowledge_base_id)
        .customize()
        .interceptor(body)
        .interceptor(raw.clone())
        .send()
        .await
        .map_err(Error::from)?;
    Ok(with_metadata(&raw))
}

/// `retrieveAndGenerate(props)`: `RetrieveAndGenerateCommandInput` JSON →
/// `RetrieveAndGenerateCommandOutput` JSON (`{ $metadata, sessionId, output, citations,
/// guardrailAction? }`). Bytes in `retrieveAndGenerateConfiguration.externalSourcesConfiguration
/// .sources[].byteContent.data` may be base64 or any byte-array JSON form.
///
/// The Express route answered `ResourceNotFoundException` with 404 and everything else with 500;
/// both are an `Err` here (the error name tells them apart).
pub async fn retrieve_and_generate(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
    input: &Value,
) -> Result<Value> {
    let m = object_of(input, "RetrieveAndGenerate input")?;
    let mut body = Value::Object(body_without(m, &[]));
    if let Some(ext) =
        body.pointer_mut("/retrieveAndGenerateConfiguration/externalSourcesConfiguration")
    {
        blobs_in_list(ext, &["sources"], &["byteContent", "data"])?;
    }
    let body = JsonBody::new(&body);
    let result = async {
        let conf = configs.sdk_config(aws).await?;
        let client = aws_sdk_bedrockagentruntime::Client::new(&conf);
        let raw = RawBody::default();
        client
            .retrieve_and_generate()
            .customize()
            .interceptor(body)
            .interceptor(raw.clone())
            .send()
            .await
            .map_err(Error::from)?;
        Ok(with_metadata(&raw))
    }
    .await;
    if let Err(e) = &result {
        tracing::error!(error = %e, "RetrieveAndGenerate error");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_shape() {
        let id = generate_session_id();
        let parts: Vec<&str> = id.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "session");
        assert!(parts[1].parse::<u128>().is_ok());
        assert_eq!(parts[2].len(), 7);
        assert!(parts[2]
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()));
    }

    #[test]
    fn body_drops_http_members_and_defaults_trace() {
        let input = json!({
            "agentId": "A", "agentAliasId": "B", "sessionId": "S", "sourceArn": "arn",
            "inputText": "hi", "memoryId": null,
            "sessionState": { "files": [{ "name": "a.csv", "useCase": "CODE_INTERPRETER",
                "source": { "sourceType": "BYTE_CONTENT",
                    "byteContent": { "mediaType": "text/csv", "data": [97, 44, 98] } } }] }
        });
        let body = invoke_agent_body(input.as_object().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({
                "inputText": "hi",
                "sessionState": { "files": [{ "name": "a.csv", "useCase": "CODE_INTERPRETER",
                    "source": { "sourceType": "BYTE_CONTENT",
                        "byteContent": { "mediaType": "text/csv", "data": "YSxi" } } }] },
                "enableTrace": false
            })
        );
        let body = invoke_agent_body(
            json!({ "inputText": "x", "enableTrace": true })
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["enableTrace"], true);
    }

    #[test]
    fn aggregation_matches_read_stream_response() {
        let b64 = |s: &[u8]| base64::engine::general_purpose::STANDARD.encode(s);
        let events = vec![
            (
                "chunk".to_string(),
                json!({ "bytes": b64("Hel".as_bytes()) }),
            ),
            (
                "trace".to_string(),
                json!({ "agentId": "A", "eventTime": 0, "trace": { "orchestrationTrace": {} } }),
            ),
            ("trace".to_string(), json!({ "agentId": "A" })),
            (
                "chunk".to_string(),
                json!({ "bytes": b64("lo".as_bytes()) }),
            ),
            (
                "files".to_string(),
                json!({ "files": [
                { "name": "chart.png", "type": "image/png", "bytes": b64(&[1, 2]) },
                { "name": "chart.png", "type": "image/png", "bytes": b64(&[9]) },
                { "name": "", "bytes": b64(&[3]) },
                { "name": "nobytes.txt" }
            ] }),
            ),
            ("returnControl".to_string(), json!({})),
        ];
        let c = aggregate_agent_events(&events);
        assert_eq!(c.message, "Hello");
        assert_eq!(
            c.files,
            vec![AgentFile {
                name: "chart.png".into(),
                content: vec![1, 2]
            }]
        );
        assert_eq!(c.traces.len(), 1);
        assert_eq!(c.traces[0]["eventTime"], "1970-01-01T00:00:00.000Z");
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["files"][0]["content"], json!({ "0": 1, "1": 2 }));
    }

    #[test]
    fn retrieve_ipc_translation() {
        assert_eq!(
            retrieve_input_from_ipc(&json!({ "knowledgeBaseId": "KB", "query": "q" })),
            json!({ "knowledgeBaseId": "KB", "retrievalQuery": { "text": "q" } })
        );
        let cfg = json!({ "vectorSearchConfiguration": { "numberOfResults": 3 } });
        assert_eq!(
            retrieve_input_from_ipc(
                &json!({ "knowledgeBaseId": "KB", "query": "q", "retrievalConfiguration": cfg })
            )["retrievalConfiguration"],
            cfg
        );
    }
}
