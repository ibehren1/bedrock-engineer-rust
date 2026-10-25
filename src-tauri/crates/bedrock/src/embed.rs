//! Text embeddings through `InvokeModel`, for local knowledgebases: Amazon Titan Text
//! Embeddings V2 (one text per call) and Cohere Embed v3 / v4 (batches).

use crate::error::{Error, Result};
use crate::sdk::SdkConfigSource;
use crate::settings::AwsSettings;
use crate::usage::{self, UsageContext, UsageEvent, UsageService};
use serde_json::{json, Value};
use std::time::Duration;

/// Most texts Cohere accepts per call.
pub const COHERE_MAX_BATCH: usize = 96;

/// What the text is for (Cohere's `input_type`; ignored by Titan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Document,
    Query,
}

/// The model family, by id (a cross-region profile prefix such as `us.` is allowed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingFamily {
    Titan,
    Cohere,
}

pub fn embedding_family(model_id: &str) -> Option<EmbeddingFamily> {
    if model_id.contains("amazon.titan-embed") {
        Some(EmbeddingFamily::Titan)
    } else if model_id.contains("cohere.embed") {
        Some(EmbeddingFamily::Cohere)
    } else {
        None
    }
}

pub fn titan_request(text: &str, dimensions: usize) -> Value {
    json!({ "inputText": text, "dimensions": dimensions, "normalize": true })
}

pub fn cohere_request(
    model_id: &str,
    texts: &[String],
    input: InputType,
    dimensions: usize,
) -> Value {
    let mut body = json!({
        "texts": texts,
        "input_type": match input {
            InputType::Document => "search_document",
            InputType::Query => "search_query",
        },
        "truncate": "END",
        "embedding_types": ["float"],
    });
    // Only v4 has a configurable output size.
    if model_id.contains("embed-v4") {
        body["output_dimension"] = json!(dimensions);
    }
    body
}

fn floats(v: &Value) -> Option<Vec<f32>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect()
}

pub fn parse_titan_response(body: &[u8]) -> Result<Vec<f32>> {
    Ok(parse_titan_response_counted(body)?.0)
}

/// The vector and `inputTextTokenCount` (absent in old responses).
pub fn parse_titan_response_counted(body: &[u8]) -> Result<(Vec<f32>, Option<u64>)> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|e| Error::plain(format!("Invalid embedding response: {e}")))?;
    let vector = floats(&v["embedding"])
        .ok_or_else(|| Error::plain("Embedding response has no embedding"))?;
    Ok((vector, v["inputTextTokenCount"].as_u64()))
}

/// A rough token count for providers that don't report one (about 4 characters a token).
pub fn estimate_tokens(texts: &[String]) -> u64 {
    texts
        .iter()
        .map(|t| t.chars().count().div_ceil(4) as u64)
        .sum()
}

fn record_usage(
    client: &aws_sdk_bedrockruntime::Client,
    model_id: &str,
    input: InputType,
    tokens: u64,
    estimated: bool,
) {
    let region = client
        .config()
        .region()
        .map(|r| r.to_string())
        .unwrap_or_default();
    let feature = match input {
        InputType::Document => "kb-ingest",
        InputType::Query => "kb-search",
    };
    let mut event = UsageEvent::new(
        UsageService::Embed,
        model_id,
        &region,
        UsageContext::feature(feature),
    );
    event.input_tokens = tokens;
    event.estimated = estimated;
    usage::record(event);
}

/// `{"embeddings": {"float": [[..]]}}` (with `embedding_types`) or `{"embeddings": [[..]]}`.
pub fn parse_cohere_response(body: &[u8]) -> Result<Vec<Vec<f32>>> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|e| Error::plain(format!("Invalid embedding response: {e}")))?;
    let list = v["embeddings"]
        .get("float")
        .unwrap_or(&v["embeddings"])
        .as_array()
        .ok_or_else(|| Error::plain("Embedding response has no embeddings"))?;
    list.iter()
        .map(|e| floats(e).ok_or_else(|| Error::plain("Malformed embedding in response")))
        .collect()
}

/// Throttling / transient errors get a few retries with exponential backoff (1s, 2s, 4s, ...).
const MAX_ATTEMPTS: u32 = 6;

fn retryable(e: &Error) -> bool {
    matches!(
        e.service_name(),
        Some("ThrottlingException" | "ServiceUnavailableException" | "ModelNotReadyException")
    )
}

async fn invoke(
    client: &aws_sdk_bedrockruntime::Client,
    model_id: &str,
    body: &Value,
) -> Result<Vec<u8>> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        let res = client
            .invoke_model()
            .model_id(model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(aws_smithy_types::Blob::new(
                serde_json::to_vec(body).unwrap_or_default(),
            ))
            .send()
            .await
            .map_err(Error::from);
        match res {
            Ok(out) => return Ok(out.body.into_inner()),
            Err(e) if attempt < MAX_ATTEMPTS && retryable(&e) => {
                let delay = Duration::from_millis(1000 << (attempt - 1));
                tracing::debug!(model_id, attempt, error = %e, "Embedding throttled, retrying");
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// A Bedrock runtime client for embedding calls.
pub async fn embedding_client(
    configs: &dyn SdkConfigSource,
    aws: &AwsSettings,
) -> Result<aws_sdk_bedrockruntime::Client> {
    let conf = configs.sdk_config(aws).await?;
    Ok(aws_sdk_bedrockruntime::Client::new(&conf))
}

/// Embed one text with Titan Text Embeddings V2 (normalized). `input` only labels the usage.
pub async fn embed_titan(
    client: &aws_sdk_bedrockruntime::Client,
    model_id: &str,
    text: &str,
    input: InputType,
    dimensions: usize,
) -> Result<Vec<f32>> {
    let body = invoke(client, model_id, &titan_request(text, dimensions)).await?;
    let (vector, tokens) = parse_titan_response_counted(&body)?;
    match tokens {
        Some(n) => record_usage(client, model_id, input, n, false),
        None => record_usage(
            client,
            model_id,
            input,
            estimate_tokens(&[text.to_string()]),
            true,
        ),
    }
    Ok(vector)
}

/// Embed up to [`COHERE_MAX_BATCH`] texts with Cohere Embed.
pub async fn embed_cohere(
    client: &aws_sdk_bedrockruntime::Client,
    model_id: &str,
    texts: &[String],
    input: InputType,
    dimensions: usize,
) -> Result<Vec<Vec<f32>>> {
    let body = invoke(
        client,
        model_id,
        &cohere_request(model_id, texts, input, dimensions),
    )
    .await?;
    let out = parse_cohere_response(&body)?;
    // Cohere's response body carries no token count.
    record_usage(client, model_id, input, estimate_tokens(texts), true);
    if out.len() != texts.len() {
        return Err(Error::plain(format!(
            "Expected {} embeddings, got {}",
            texts.len(),
            out.len()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families() {
        assert_eq!(
            embedding_family("amazon.titan-embed-text-v2:0"),
            Some(EmbeddingFamily::Titan)
        );
        assert_eq!(
            embedding_family("us.cohere.embed-v4:0"),
            Some(EmbeddingFamily::Cohere)
        );
        assert_eq!(embedding_family("anthropic.claude"), None);
    }

    #[test]
    fn request_bodies() {
        assert_eq!(
            titan_request("hi", 512),
            json!({"inputText": "hi", "dimensions": 512, "normalize": true})
        );
        let b = cohere_request(
            "cohere.embed-multilingual-v3",
            &["a".into()],
            InputType::Query,
            1024,
        );
        assert_eq!(b["input_type"], "search_query");
        assert!(b.get("output_dimension").is_none());
        let b = cohere_request("cohere.embed-v4:0", &["a".into()], InputType::Document, 256);
        assert_eq!(b["input_type"], "search_document");
        assert_eq!(b["output_dimension"], 256);
    }

    #[test]
    fn responses() {
        assert_eq!(
            parse_titan_response(br#"{"embedding":[0.5,1],"inputTextTokenCount":1}"#).unwrap(),
            vec![0.5, 1.0]
        );
        assert!(parse_titan_response(b"{}").is_err());
        assert_eq!(
            parse_titan_response_counted(br#"{"embedding":[1],"inputTextTokenCount":7}"#)
                .unwrap()
                .1,
            Some(7)
        );
        assert_eq!(estimate_tokens(&["abcde".into(), "ab".into()]), 3);
        assert_eq!(
            parse_cohere_response(br#"{"embeddings":{"float":[[1],[2]]}}"#).unwrap(),
            vec![vec![1.0], vec![2.0]]
        );
        assert_eq!(
            parse_cohere_response(br#"{"embeddings":[[3]]}"#).unwrap(),
            vec![vec![3.0]]
        );
        assert!(parse_cohere_response(br#"{"embeddings":{"float":[["x"]]}}"#).is_err());
    }
}
