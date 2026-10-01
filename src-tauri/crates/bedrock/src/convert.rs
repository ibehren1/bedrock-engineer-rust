//! Renderer JSON <-> Bedrock Runtime SDK type conversion.
//!
//! The renderer builds Converse requests with the JS SDK's plain-object shapes (`{ text }`,
//! `{ toolUse: { toolUseId, name, input } }`, `{ cachePoint: { type: 'default' } }`, ...). The
//! `*_from_json` functions turn those into SDK types. Unknown keys (e.g. the renderer's message
//! `id` / `metadata`) are ignored, just as the JS SDK serializer ignores them.
//!
//! Union members are selected in the JS SDK's `visit` order: the first present member wins
//! (`text` before `image` before ... `reasoningContent`).
//!
//! The `*_to_json` functions render SDK output types as the JS SDK deserializer produced them:
//! keys in alphabetical order, absent optionals omitted, blobs as `Uint8Array` JSON
//! (see [`crate::document`]).

use crate::document::{
    blob_from_json, blob_to_json, document_to_json, json_to_document, StringBlob,
};
use crate::error::{Error, Result};
use crate::settings::InferenceConfig;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, CachePointBlock, CachePointType, CacheTtl, CitationsConfig,
    CitationsContentBlock, ContentBlock, ConversationRole, DocumentBlock, DocumentContentBlock,
    DocumentFormat, DocumentSource, GuardrailConfiguration, GuardrailConverseContentBlock,
    GuardrailConverseContentQualifier, GuardrailConverseImageBlock, GuardrailConverseImageFormat,
    GuardrailConverseImageSource, GuardrailConverseTextBlock, GuardrailStreamConfiguration,
    GuardrailStreamProcessingMode, GuardrailTrace, ImageBlock, ImageFormat, ImageSource,
    InferenceConfiguration, Message, ReasoningContentBlock, ReasoningTextBlock, S3Location,
    SpecificToolChoice, SystemContentBlock, SystemTool, Tool, ToolChoice, ToolConfiguration,
    ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolResultStatus, ToolSpecification,
    ToolUseBlock, ToolUseType, VideoBlock, VideoFormat, VideoSource,
};
use aws_smithy_types::error::operation::BuildError;
use serde_json::{json, Map, Value};

fn built<T>(r: std::result::Result<T, BuildError>) -> Result<T> {
    r.map_err(|e| Error::InvalidRequest(e.to_string()))
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::InvalidRequest(msg.into())
}

fn as_object<'a>(v: &'a Value, what: &str) -> Result<&'a Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| invalid(format!("{what} must be an object")))
}

/// A member counts as present when it is set and not `null`.
fn member<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    m.get(key).filter(|v| !v.is_null())
}

fn opt_str(m: &Map<String, Value>, key: &str) -> Option<String> {
    member(m, key).and_then(Value::as_str).map(str::to_string)
}

fn req_str(m: &Map<String, Value>, key: &str, what: &str) -> Result<String> {
    opt_str(m, key).ok_or_else(|| invalid(format!("{what}.{key} must be a string")))
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------
// Request direction: JSON -> SDK
// ---------------------------------------------------------------------------------------------

/// `{ role, content: [...] }` -> `Message`.
pub fn message_from_json(v: &Value) -> Result<Message> {
    let m = as_object(v, "message")?;
    let role = req_str(m, "role", "message")?;
    let content = match member(m, "content") {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(content_block_from_json)
            .collect::<Result<Vec<_>>>()?,
        Some(_) => return Err(invalid("message.content must be an array")),
        None => Vec::new(),
    };
    built(
        Message::builder()
            .role(ConversationRole::from(role.as_str()))
            .set_content(Some(content))
            .build(),
    )
}

/// One message `ContentBlock`.
pub fn content_block_from_json(v: &Value) -> Result<ContentBlock> {
    let m = as_object(v, "content block")?;
    if let Some(t) = member(m, "text") {
        return Ok(ContentBlock::Text(text_of(t)));
    }
    if let Some(img) = member(m, "image") {
        return Ok(ContentBlock::Image(image_from_json(img)?));
    }
    if let Some(doc) = member(m, "document") {
        return Ok(ContentBlock::Document(document_from_json(doc)?));
    }
    if let Some(video) = member(m, "video") {
        return Ok(ContentBlock::Video(video_from_json(video)?));
    }
    if let Some(tu) = member(m, "toolUse") {
        return Ok(ContentBlock::ToolUse(tool_use_from_json(tu)?));
    }
    if let Some(tr) = member(m, "toolResult") {
        return Ok(ContentBlock::ToolResult(tool_result_from_json(tr)?));
    }
    if let Some(g) = member(m, "guardContent") {
        return Ok(ContentBlock::GuardContent(guard_content_from_json(g)?));
    }
    if let Some(cp) = member(m, "cachePoint") {
        return Ok(ContentBlock::CachePoint(cache_point_from_json(cp)?));
    }
    if let Some(rc) = member(m, "reasoningContent") {
        return Ok(ContentBlock::ReasoningContent(reasoning_from_json(rc)?));
    }
    if let Some(cc) = member(m, "citationsContent") {
        return Ok(ContentBlock::CitationsContent(citations_content_from_json(
            cc,
        )?));
    }
    let keys: Vec<&str> = m.keys().map(String::as_str).collect();
    Err(invalid(format!(
        "unsupported content block (keys: {})",
        keys.join(", ")
    )))
}

/// `{ format, source: { bytes | s3Location } }`. String bytes are base64 (processImageContent).
pub fn image_from_json(v: &Value) -> Result<ImageBlock> {
    let m = as_object(v, "image")?;
    let format = req_str(m, "format", "image")?;
    let source = match member(m, "source") {
        Some(src) => Some(image_source_from_json(src)?),
        None => None,
    };
    built(
        ImageBlock::builder()
            .format(ImageFormat::from(format.as_str()))
            .set_source(source)
            .build(),
    )
}

fn image_source_from_json(v: &Value) -> Result<ImageSource> {
    let m = as_object(v, "image.source")?;
    if let Some(bytes) = member(m, "bytes") {
        let blob = blob_from_json(bytes, StringBlob::Base64)
            .ok_or_else(|| invalid("image.source.bytes is not base64 or a byte array"))?;
        return Ok(ImageSource::Bytes(blob));
    }
    if let Some(loc) = member(m, "s3Location") {
        return Ok(ImageSource::S3Location(s3_location_from_json(loc)?));
    }
    Err(invalid("image.source needs bytes or s3Location"))
}

fn s3_location_from_json(v: &Value) -> Result<S3Location> {
    let m = as_object(v, "s3Location")?;
    built(
        S3Location::builder()
            .uri(req_str(m, "uri", "s3Location")?)
            .set_bucket_owner(opt_str(m, "bucketOwner"))
            .build(),
    )
}

/// `{ format, name, source: { bytes | text | content | s3Location }, context?, citations? }`.
pub fn document_from_json(v: &Value) -> Result<DocumentBlock> {
    let m = as_object(v, "document")?;
    let source = match member(m, "source") {
        Some(src) => {
            let s = as_object(src, "document.source")?;
            Some(if let Some(bytes) = member(s, "bytes") {
                DocumentSource::Bytes(
                    blob_from_json(bytes, StringBlob::Utf8)
                        .ok_or_else(|| invalid("document.source.bytes is not a byte array"))?,
                )
            } else if let Some(text) = member(s, "text") {
                DocumentSource::Text(text_of(text))
            } else if let Some(Value::Array(items)) = member(s, "content") {
                DocumentSource::Content(
                    items
                        .iter()
                        .filter_map(|i| i.get("text").map(text_of))
                        .map(DocumentContentBlock::Text)
                        .collect(),
                )
            } else if let Some(loc) = member(s, "s3Location") {
                DocumentSource::S3Location(s3_location_from_json(loc)?)
            } else {
                return Err(invalid(
                    "document.source needs bytes, text, content or s3Location",
                ));
            })
        }
        None => None,
    };
    let citations = match member(m, "citations") {
        Some(c) => Some(built(
            CitationsConfig::builder()
                .enabled(c.get("enabled").and_then(Value::as_bool).unwrap_or(false))
                .build(),
        )?),
        None => None,
    };
    built(
        DocumentBlock::builder()
            .format(DocumentFormat::from(
                req_str(m, "format", "document")?.as_str(),
            ))
            .name(req_str(m, "name", "document")?)
            .set_source(source)
            .set_context(opt_str(m, "context"))
            .set_citations(citations)
            .build(),
    )
}

/// `{ format, source: { bytes | s3Location } }`.
pub fn video_from_json(v: &Value) -> Result<VideoBlock> {
    let m = as_object(v, "video")?;
    let source = match member(m, "source") {
        Some(src) => {
            let s = as_object(src, "video.source")?;
            Some(if let Some(bytes) = member(s, "bytes") {
                VideoSource::Bytes(
                    blob_from_json(bytes, StringBlob::Utf8)
                        .ok_or_else(|| invalid("video.source.bytes is not a byte array"))?,
                )
            } else if let Some(loc) = member(s, "s3Location") {
                VideoSource::S3Location(s3_location_from_json(loc)?)
            } else {
                return Err(invalid("video.source needs bytes or s3Location"));
            })
        }
        None => None,
    };
    built(
        VideoBlock::builder()
            .format(VideoFormat::from(req_str(m, "format", "video")?.as_str()))
            .set_source(source)
            .build(),
    )
}

/// `{ toolUseId, name, input, type? }`.
pub fn tool_use_from_json(v: &Value) -> Result<ToolUseBlock> {
    let m = as_object(v, "toolUse")?;
    let input = member(m, "input")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    built(
        ToolUseBlock::builder()
            .tool_use_id(req_str(m, "toolUseId", "toolUse")?)
            .name(req_str(m, "name", "toolUse")?)
            .input(json_to_document(&input))
            .set_type(opt_str(m, "type").map(|t| ToolUseType::from(t.as_str())))
            .build(),
    )
}

/// `{ toolUseId, content: [{ text | json | image | document | video }], status?, type? }`.
pub fn tool_result_from_json(v: &Value) -> Result<ToolResultBlock> {
    let m = as_object(v, "toolResult")?;
    let content = match member(m, "content") {
        Some(Value::Array(items)) => items
            .iter()
            .map(tool_result_content_from_json)
            .collect::<Result<Vec<_>>>()?,
        Some(_) => return Err(invalid("toolResult.content must be an array")),
        None => Vec::new(),
    };
    built(
        ToolResultBlock::builder()
            .tool_use_id(req_str(m, "toolUseId", "toolResult")?)
            .set_content(Some(content))
            .set_status(opt_str(m, "status").map(|s| ToolResultStatus::from(s.as_str())))
            .set_type(opt_str(m, "type"))
            .build(),
    )
}

fn tool_result_content_from_json(v: &Value) -> Result<ToolResultContentBlock> {
    let m = as_object(v, "toolResult.content[]")?;
    if let Some(j) = member(m, "json") {
        return Ok(ToolResultContentBlock::Json(json_to_document(j)));
    }
    if let Some(t) = member(m, "text") {
        return Ok(ToolResultContentBlock::Text(text_of(t)));
    }
    if let Some(img) = member(m, "image") {
        return Ok(ToolResultContentBlock::Image(image_from_json(img)?));
    }
    if let Some(doc) = member(m, "document") {
        return Ok(ToolResultContentBlock::Document(document_from_json(doc)?));
    }
    if let Some(video) = member(m, "video") {
        return Ok(ToolResultContentBlock::Video(video_from_json(video)?));
    }
    Err(invalid("unsupported toolResult content block"))
}

/// `{ text: { text, qualifiers? } }` or `{ image: { format, source: { bytes } } }`.
pub fn guard_content_from_json(v: &Value) -> Result<GuardrailConverseContentBlock> {
    let m = as_object(v, "guardContent")?;
    if let Some(t) = member(m, "text") {
        let t = as_object(t, "guardContent.text")?;
        let qualifiers = member(t, "qualifiers").and_then(Value::as_array).map(|qs| {
            qs.iter()
                .filter_map(Value::as_str)
                .map(GuardrailConverseContentQualifier::from)
                .collect()
        });
        return Ok(GuardrailConverseContentBlock::Text(built(
            GuardrailConverseTextBlock::builder()
                .text(req_str(t, "text", "guardContent.text")?)
                .set_qualifiers(qualifiers)
                .build(),
        )?));
    }
    if let Some(img) = member(m, "image") {
        let i = as_object(img, "guardContent.image")?;
        let source = match member(i, "source").and_then(|s| s.get("bytes")) {
            Some(bytes) => Some(GuardrailConverseImageSource::Bytes(
                blob_from_json(bytes, StringBlob::Base64)
                    .ok_or_else(|| invalid("guardContent.image.source.bytes is not bytes"))?,
            )),
            None => None,
        };
        return Ok(GuardrailConverseContentBlock::Image(built(
            GuardrailConverseImageBlock::builder()
                .format(GuardrailConverseImageFormat::from(
                    req_str(i, "format", "guardContent.image")?.as_str(),
                ))
                .set_source(source)
                .build(),
        )?));
    }
    Err(invalid("guardContent needs text or image"))
}

/// `{ type: 'default', ttl? }`.
pub fn cache_point_from_json(v: &Value) -> Result<CachePointBlock> {
    let m = as_object(v, "cachePoint")?;
    let kind = opt_str(m, "type").unwrap_or_else(|| "default".to_string());
    built(
        CachePointBlock::builder()
            .r#type(CachePointType::from(kind.as_str()))
            .set_ttl(opt_str(m, "ttl").map(|t| CacheTtl::from(t.as_str())))
            .build(),
    )
}

/// `{ reasoningText: { text, signature? } }` or `{ redactedContent: <bytes> }`.
pub fn reasoning_from_json(v: &Value) -> Result<ReasoningContentBlock> {
    let m = as_object(v, "reasoningContent")?;
    if let Some(rt) = member(m, "reasoningText") {
        let rt = as_object(rt, "reasoningContent.reasoningText")?;
        return Ok(ReasoningContentBlock::ReasoningText(built(
            ReasoningTextBlock::builder()
                .text(opt_str(rt, "text").unwrap_or_default())
                .set_signature(opt_str(rt, "signature"))
                .build(),
        )?));
    }
    if let Some(rc) = member(m, "redactedContent") {
        // Chat history stores this as serialized Uint8Array JSON; strings are treated as base64
        // since that is the only sensible textual encoding for opaque bytes.
        let blob = blob_from_json(rc, StringBlob::Base64)
            .ok_or_else(|| invalid("reasoningContent.redactedContent is not bytes"))?;
        return Ok(ReasoningContentBlock::RedactedContent(blob));
    }
    Err(invalid(
        "reasoningContent needs reasoningText or redactedContent",
    ))
}

/// Minimal `citationsContent` support: generated text only (citations metadata is not replayed).
fn citations_content_from_json(v: &Value) -> Result<CitationsContentBlock> {
    use aws_sdk_bedrockruntime::types::CitationGeneratedContent;
    let m = as_object(v, "citationsContent")?;
    let content = member(m, "content").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|i| i.get("text").map(text_of))
            .map(CitationGeneratedContent::Text)
            .collect()
    });
    Ok(CitationsContentBlock::builder()
        .set_content(content)
        .build())
}

/// One system prompt block: `{ text }`, `{ guardContent }` or `{ cachePoint }`.
pub fn system_block_from_json(v: &Value) -> Result<SystemContentBlock> {
    let m = as_object(v, "system block")?;
    if let Some(t) = member(m, "text") {
        return Ok(SystemContentBlock::Text(text_of(t)));
    }
    if let Some(g) = member(m, "guardContent") {
        return Ok(SystemContentBlock::GuardContent(guard_content_from_json(
            g,
        )?));
    }
    if let Some(cp) = member(m, "cachePoint") {
        return Ok(SystemContentBlock::CachePoint(cache_point_from_json(cp)?));
    }
    Err(invalid("unsupported system block"))
}

/// `{ tools: [...], toolChoice? }`.
pub fn tool_config_from_json(v: &Value) -> Result<ToolConfiguration> {
    let m = as_object(v, "toolConfig")?;
    let tools = match member(m, "tools") {
        Some(Value::Array(items)) => items
            .iter()
            .map(tool_from_json)
            .collect::<Result<Vec<_>>>()?,
        Some(_) => return Err(invalid("toolConfig.tools must be an array")),
        None => Vec::new(),
    };
    let choice = match member(m, "toolChoice") {
        Some(c) => tool_choice_from_json(c)?,
        None => None,
    };
    built(
        ToolConfiguration::builder()
            .set_tools(Some(tools))
            .set_tool_choice(choice)
            .build(),
    )
}

fn tool_from_json(v: &Value) -> Result<Tool> {
    let m = as_object(v, "tool")?;
    if let Some(spec) = member(m, "toolSpec") {
        let s = as_object(spec, "toolSpec")?;
        let schema = member(s, "inputSchema")
            .and_then(|is| is.get("json"))
            .map(|j| ToolInputSchema::Json(json_to_document(j)));
        return Ok(Tool::ToolSpec(built(
            ToolSpecification::builder()
                .name(req_str(s, "name", "toolSpec")?)
                .set_description(opt_str(s, "description"))
                .set_input_schema(schema)
                .set_strict(member(s, "strict").and_then(Value::as_bool))
                .build(),
        )?));
    }
    if let Some(cp) = member(m, "cachePoint") {
        return Ok(Tool::CachePoint(cache_point_from_json(cp)?));
    }
    if let Some(st) = member(m, "systemTool") {
        let st = as_object(st, "systemTool")?;
        return Ok(Tool::SystemTool(built(
            SystemTool::builder()
                .name(req_str(st, "name", "systemTool")?)
                .build(),
        )?));
    }
    Err(invalid("unsupported tool entry"))
}

fn tool_choice_from_json(v: &Value) -> Result<Option<ToolChoice>> {
    let m = as_object(v, "toolChoice")?;
    if member(m, "auto").is_some() {
        return Ok(Some(ToolChoice::Auto(AutoToolChoice::builder().build())));
    }
    if member(m, "any").is_some() {
        return Ok(Some(ToolChoice::Any(AnyToolChoice::builder().build())));
    }
    if let Some(t) = member(m, "tool") {
        let t = as_object(t, "toolChoice.tool")?;
        return Ok(Some(ToolChoice::Tool(built(
            SpecificToolChoice::builder()
                .name(req_str(t, "name", "toolChoice.tool")?)
                .build(),
        )?)));
    }
    Ok(None)
}

/// Inference config -> SDK. Out-of-range `maxTokens` is rejected locally.
pub fn inference_config_to_sdk(c: &InferenceConfig) -> Result<InferenceConfiguration> {
    let max_tokens = match c.max_tokens {
        Some(n) => {
            Some(i32::try_from(n).map_err(|_| invalid(format!("maxTokens {n} is out of range")))?)
        }
        None => None,
    };
    Ok(InferenceConfiguration::builder()
        .set_max_tokens(max_tokens)
        .set_temperature(c.temperature.map(|t| t as f32))
        .set_top_p(c.top_p.map(|t| t as f32))
        .set_stop_sequences(c.stop_sequences.clone())
        .build())
}

/// `{ guardrailIdentifier, guardrailVersion, trace? }` for `Converse`.
pub fn guardrail_from_json(v: &Value) -> Result<GuardrailConfiguration> {
    let m = as_object(v, "guardrailConfig")?;
    Ok(GuardrailConfiguration::builder()
        .guardrail_identifier(req_str(m, "guardrailIdentifier", "guardrailConfig")?)
        .guardrail_version(req_str(m, "guardrailVersion", "guardrailConfig")?)
        .set_trace(opt_str(m, "trace").map(|t| GuardrailTrace::from(t.as_str())))
        .build())
}

/// `{ guardrailIdentifier, guardrailVersion, trace?, streamProcessingMode? }` for `ConverseStream`.
pub fn guardrail_stream_from_json(v: &Value) -> Result<GuardrailStreamConfiguration> {
    let m = as_object(v, "guardrailConfig")?;
    Ok(GuardrailStreamConfiguration::builder()
        .guardrail_identifier(req_str(m, "guardrailIdentifier", "guardrailConfig")?)
        .guardrail_version(req_str(m, "guardrailVersion", "guardrailConfig")?)
        .set_trace(opt_str(m, "trace").map(|t| GuardrailTrace::from(t.as_str())))
        .set_stream_processing_mode(
            opt_str(m, "streamProcessingMode")
                .map(|t| GuardrailStreamProcessingMode::from(t.as_str())),
        )
        .build())
}

// ---------------------------------------------------------------------------------------------
// Response direction: SDK -> JSON
// ---------------------------------------------------------------------------------------------

/// Build an object from `(key, Option<Value>)` pairs, dropping `None`s (JS `take` semantics).
pub(crate) fn object(pairs: Vec<(&str, Option<Value>)>) -> Value {
    let mut out = Map::new();
    for (k, v) in pairs {
        if let Some(v) = v {
            out.insert(k.to_string(), v);
        }
    }
    Value::Object(out)
}

/// `Message` -> `{ content: [...], role }`.
pub fn message_to_json(msg: &Message) -> Value {
    json!({
        "content": msg.content.iter().filter_map(content_block_to_json).collect::<Vec<_>>(),
        "role": msg.role.as_str(),
    })
}

/// SDK `ContentBlock` -> JSON; `None` for variants this port does not render.
pub fn content_block_to_json(block: &ContentBlock) -> Option<Value> {
    Some(match block {
        ContentBlock::Text(t) => json!({ "text": t }),
        ContentBlock::Image(i) => json!({ "image": image_to_json(i) }),
        ContentBlock::Document(d) => json!({ "document": document_to_json_value(d) }),
        ContentBlock::Video(v) => json!({ "video": video_to_json(v) }),
        ContentBlock::ToolUse(t) => json!({ "toolUse": tool_use_to_json(t) }),
        ContentBlock::ToolResult(t) => json!({ "toolResult": tool_result_to_json(t) }),
        ContentBlock::GuardContent(g) => json!({ "guardContent": guard_content_to_json(g)? }),
        ContentBlock::CachePoint(c) => json!({ "cachePoint": cache_point_to_json(c) }),
        ContentBlock::ReasoningContent(r) => json!({ "reasoningContent": reasoning_to_json(r)? }),
        ContentBlock::CitationsContent(c) => {
            json!({ "citationsContent": citations_content_to_json(c) })
        }
        _ => return None,
    })
}

fn s3_location_to_json(l: &S3Location) -> Value {
    object(vec![
        ("bucketOwner", l.bucket_owner.clone().map(Value::from)),
        ("uri", Some(Value::from(l.uri.clone()))),
    ])
}

fn image_to_json(i: &ImageBlock) -> Value {
    let source = i.source.as_ref().and_then(|s| match s {
        ImageSource::Bytes(b) => Some(json!({ "bytes": blob_to_json(b) })),
        ImageSource::S3Location(l) => Some(json!({ "s3Location": s3_location_to_json(l) })),
        _ => None,
    });
    object(vec![
        (
            "error",
            i.error
                .as_ref()
                .map(|e| object(vec![("message", e.message.clone().map(Value::from))])),
        ),
        ("format", Some(Value::from(i.format.as_str()))),
        ("source", source),
    ])
}

fn document_to_json_value(d: &DocumentBlock) -> Value {
    let source = d.source.as_ref().and_then(|s| match s {
        DocumentSource::Bytes(b) => Some(json!({ "bytes": blob_to_json(b) })),
        DocumentSource::Text(t) => Some(json!({ "text": t })),
        DocumentSource::Content(items) => Some(json!({
            "content": items.iter().filter_map(|c| match c {
                DocumentContentBlock::Text(t) => Some(json!({ "text": t })),
                _ => None,
            }).collect::<Vec<_>>()
        })),
        DocumentSource::S3Location(l) => Some(json!({ "s3Location": s3_location_to_json(l) })),
        _ => None,
    });
    object(vec![
        (
            "citations",
            d.citations
                .as_ref()
                .map(|c| json!({ "enabled": c.enabled })),
        ),
        ("context", d.context.clone().map(Value::from)),
        ("format", Some(Value::from(d.format.as_str()))),
        ("name", Some(Value::from(d.name.clone()))),
        ("source", source),
    ])
}

fn video_to_json(v: &VideoBlock) -> Value {
    let source = v.source.as_ref().and_then(|s| match s {
        VideoSource::Bytes(b) => Some(json!({ "bytes": blob_to_json(b) })),
        VideoSource::S3Location(l) => Some(json!({ "s3Location": s3_location_to_json(l) })),
        _ => None,
    });
    object(vec![
        ("format", Some(Value::from(v.format.as_str()))),
        ("source", source),
    ])
}

fn tool_use_to_json(t: &ToolUseBlock) -> Value {
    object(vec![
        ("input", Some(document_to_json(&t.input))),
        ("name", Some(Value::from(t.name.clone()))),
        ("toolUseId", Some(Value::from(t.tool_use_id.clone()))),
        ("type", t.r#type.as_ref().map(|x| Value::from(x.as_str()))),
    ])
}

fn tool_result_to_json(t: &ToolResultBlock) -> Value {
    let content: Vec<Value> = t
        .content
        .iter()
        .filter_map(|c| match c {
            ToolResultContentBlock::Json(d) => Some(json!({ "json": document_to_json(d) })),
            ToolResultContentBlock::Text(s) => Some(json!({ "text": s })),
            ToolResultContentBlock::Image(i) => Some(json!({ "image": image_to_json(i) })),
            ToolResultContentBlock::Document(d) => {
                Some(json!({ "document": document_to_json_value(d) }))
            }
            ToolResultContentBlock::Video(v) => Some(json!({ "video": video_to_json(v) })),
            _ => None,
        })
        .collect();
    object(vec![
        ("content", Some(Value::Array(content))),
        ("status", t.status.as_ref().map(|s| Value::from(s.as_str()))),
        ("toolUseId", Some(Value::from(t.tool_use_id.clone()))),
        ("type", t.r#type.clone().map(Value::from)),
    ])
}

fn guard_content_to_json(g: &GuardrailConverseContentBlock) -> Option<Value> {
    Some(match g {
        GuardrailConverseContentBlock::Text(t) => json!({
            "text": object(vec![
                ("qualifiers", t.qualifiers.as_ref().map(|qs| {
                    Value::Array(qs.iter().map(|q| Value::from(q.as_str())).collect())
                })),
                ("text", Some(Value::from(t.text.clone()))),
            ])
        }),
        GuardrailConverseContentBlock::Image(i) => json!({
            "image": object(vec![
                ("format", Some(Value::from(i.format.as_str()))),
                ("source", i.source.as_ref().and_then(|s| match s {
                    GuardrailConverseImageSource::Bytes(b) => Some(json!({ "bytes": blob_to_json(b) })),
                    _ => None,
                })),
            ])
        }),
        _ => return None,
    })
}

fn cache_point_to_json(c: &CachePointBlock) -> Value {
    object(vec![
        ("ttl", c.ttl.as_ref().map(|t| Value::from(t.as_str()))),
        ("type", Some(Value::from(c.r#type.as_str()))),
    ])
}

fn reasoning_to_json(r: &ReasoningContentBlock) -> Option<Value> {
    Some(match r {
        ReasoningContentBlock::ReasoningText(t) => json!({
            "reasoningText": object(vec![
                ("signature", t.signature.clone().map(Value::from)),
                ("text", Some(Value::from(t.text.clone()))),
            ])
        }),
        ReasoningContentBlock::RedactedContent(b) => json!({ "redactedContent": blob_to_json(b) }),
        _ => return None,
    })
}

fn citations_content_to_json(c: &CitationsContentBlock) -> Value {
    use aws_sdk_bedrockruntime::types::{CitationGeneratedContent, CitationSourceContent};
    let content = c.content.as_ref().map(|items| {
        Value::Array(
            items
                .iter()
                .filter_map(|i| match i {
                    CitationGeneratedContent::Text(t) => Some(json!({ "text": t })),
                    _ => None,
                })
                .collect(),
        )
    });
    let citations = c.citations.as_ref().map(|cs| {
        Value::Array(
            cs.iter()
                .map(|ci| {
                    object(vec![
                        ("source", ci.source.clone().map(Value::from)),
                        (
                            "sourceContent",
                            ci.source_content.as_ref().map(|sc| {
                                Value::Array(
                                    sc.iter()
                                        .filter_map(|s| match s {
                                            CitationSourceContent::Text(t) => {
                                                Some(json!({ "text": t }))
                                            }
                                            _ => None,
                                        })
                                        .collect(),
                                )
                            }),
                        ),
                        ("title", ci.title.clone().map(Value::from)),
                    ])
                })
                .collect(),
        )
    });
    object(vec![("citations", citations), ("content", content)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_smithy_types::{Blob, Document};

    #[test]
    fn text_message_with_renderer_extras() {
        // IdentifiableMessage carries id/metadata; they must be ignored.
        let msg = message_from_json(&json!({
            "id": "m1",
            "metadata": { "modelId": "x" },
            "role": "user",
            "content": [{ "text": "hello" }]
        }))
        .unwrap();
        assert_eq!(msg.role, ConversationRole::User);
        assert_eq!(msg.content, vec![ContentBlock::Text("hello".into())]);
        assert_eq!(
            message_to_json(&msg),
            json!({ "content": [{ "text": "hello" }], "role": "user" })
        );
    }

    #[test]
    fn tool_use_block_round_trip() {
        let v = json!({ "toolUse": { "toolUseId": "t1", "name": "readFiles", "input": { "paths": ["a", "b"], "n": 2 } } });
        let block = content_block_from_json(&v).unwrap();
        let ContentBlock::ToolUse(tu) = &block else {
            panic!()
        };
        assert_eq!(tu.tool_use_id, "t1");
        assert_eq!(tu.name, "readFiles");
        assert!(matches!(tu.input, Document::Object(_)));
        assert_eq!(
            content_block_to_json(&block).unwrap(),
            json!({ "toolUse": { "input": { "n": 2, "paths": ["a", "b"] }, "name": "readFiles", "toolUseId": "t1" } })
        );
    }

    #[test]
    fn tool_use_missing_input_defaults_to_empty_object() {
        let block =
            content_block_from_json(&json!({ "toolUse": { "toolUseId": "t", "name": "n" } }))
                .unwrap();
        let ContentBlock::ToolUse(tu) = block else {
            panic!()
        };
        assert_eq!(tu.input, Document::Object(Default::default()));
    }

    #[test]
    fn tool_result_with_json_text_image_and_status() {
        let v = json!({
            "toolResult": {
                "toolUseId": "t1",
                "status": "error",
                "content": [
                    { "json": { "success": false, "error": "boom" } },
                    { "text": "plain" },
                    { "image": { "format": "png", "source": { "bytes": "AQID" } } }
                ]
            }
        });
        let block = content_block_from_json(&v).unwrap();
        let ContentBlock::ToolResult(tr) = &block else {
            panic!()
        };
        assert_eq!(tr.status, Some(ToolResultStatus::Error));
        assert_eq!(tr.content.len(), 3);
        match &tr.content[2] {
            ToolResultContentBlock::Image(i) => {
                assert_eq!(i.format, ImageFormat::Png);
                assert_eq!(
                    i.source,
                    Some(ImageSource::Bytes(Blob::new(vec![1u8, 2, 3])))
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            content_block_to_json(&block).unwrap(),
            json!({ "toolResult": {
                "content": [
                    { "json": { "error": "boom", "success": false } },
                    { "text": "plain" },
                    { "image": { "format": "png", "source": { "bytes": { "0": 1, "1": 2, "2": 3 } } } }
                ],
                "status": "error",
                "toolUseId": "t1"
            } })
        );
    }

    #[test]
    fn image_bytes_from_serialized_uint8array() {
        let block = content_block_from_json(&json!({
            "image": { "format": "jpeg", "source": { "bytes": { "0": 255, "1": 216 } } }
        }))
        .unwrap();
        let ContentBlock::Image(img) = block else {
            panic!()
        };
        assert_eq!(img.format, ImageFormat::Jpeg);
        assert_eq!(
            img.source,
            Some(ImageSource::Bytes(Blob::new(vec![255u8, 216])))
        );
    }

    #[test]
    fn image_with_bad_bytes_is_rejected() {
        let err = content_block_from_json(&json!({
            "image": { "format": "png", "source": { "bytes": "%%%" } }
        }))
        .unwrap_err();
        assert!(matches!(err, Error::InvalidRequest(_)));
    }

    #[test]
    fn document_and_video_blocks() {
        let block = content_block_from_json(&json!({
            "document": { "format": "pdf", "name": "spec", "source": { "bytes": [37, 80] } }
        }))
        .unwrap();
        let ContentBlock::Document(d) = &block else {
            panic!()
        };
        assert_eq!(d.format, DocumentFormat::Pdf);
        assert_eq!(
            d.source,
            Some(DocumentSource::Bytes(Blob::new(vec![37u8, 80])))
        );
        assert_eq!(
            content_block_to_json(&block).unwrap(),
            json!({ "document": { "format": "pdf", "name": "spec", "source": { "bytes": { "0": 37, "1": 80 } } } })
        );

        let block = content_block_from_json(&json!({
            "video": { "format": "mp4", "source": { "s3Location": { "uri": "s3://b/k" } } }
        }))
        .unwrap();
        assert_eq!(
            content_block_to_json(&block).unwrap(),
            json!({ "video": { "format": "mp4", "source": { "s3Location": { "uri": "s3://b/k" } } } })
        );
    }

    #[test]
    fn reasoning_text_with_signature_round_trip() {
        let v = json!({ "reasoningContent": { "reasoningText": { "text": "let me think", "signature": "sig==" } } });
        let block = content_block_from_json(&v).unwrap();
        let ContentBlock::ReasoningContent(ReasoningContentBlock::ReasoningText(rt)) = &block
        else {
            panic!()
        };
        assert_eq!(rt.text, "let me think");
        assert_eq!(rt.signature.as_deref(), Some("sig=="));
        assert_eq!(
            content_block_to_json(&block).unwrap(),
            json!({ "reasoningContent": { "reasoningText": { "signature": "sig==", "text": "let me think" } } })
        );
    }

    #[test]
    fn redacted_reasoning_from_history_shape() {
        // The renderer stores redactedContent exactly as the stream emitted it (Uint8Array JSON).
        let v = json!({ "reasoningContent": { "redactedContent": { "0": 9, "1": 8 } } });
        let block = content_block_from_json(&v).unwrap();
        assert_eq!(
            block,
            ContentBlock::ReasoningContent(ReasoningContentBlock::RedactedContent(Blob::new(
                vec![9u8, 8]
            )))
        );
        assert_eq!(content_block_to_json(&block).unwrap(), v);
    }

    #[test]
    fn cache_point_blocks_everywhere() {
        let cp = json!({ "cachePoint": { "type": "default" } });
        assert!(matches!(
            content_block_from_json(&cp).unwrap(),
            ContentBlock::CachePoint(ref c) if c.r#type == CachePointType::Default && c.ttl.is_none()
        ));
        assert!(matches!(
            system_block_from_json(&cp).unwrap(),
            SystemContentBlock::CachePoint(_)
        ));
        let with_ttl =
            content_block_from_json(&json!({ "cachePoint": { "type": "default", "ttl": "1h" } }))
                .unwrap();
        assert_eq!(
            content_block_to_json(&with_ttl).unwrap(),
            json!({ "cachePoint": { "ttl": "1h", "type": "default" } })
        );
        let tc = tool_config_from_json(&json!({
            "tools": [
                { "toolSpec": { "name": "a", "description": "d", "inputSchema": { "json": { "type": "object" } } } },
                { "cachePoint": { "type": "default" } }
            ]
        }))
        .unwrap();
        assert_eq!(tc.tools.len(), 2);
        assert!(matches!(tc.tools[1], Tool::CachePoint(_)));
        let Tool::ToolSpec(spec) = &tc.tools[0] else {
            panic!()
        };
        assert_eq!(spec.name, "a");
        assert_eq!(spec.description.as_deref(), Some("d"));
        assert!(matches!(
            spec.input_schema,
            Some(ToolInputSchema::Json(Document::Object(_)))
        ));
    }

    #[test]
    fn tool_choice_variants() {
        let auto =
            tool_config_from_json(&json!({ "tools": [], "toolChoice": { "auto": {} } })).unwrap();
        assert!(matches!(auto.tool_choice, Some(ToolChoice::Auto(_))));
        let any =
            tool_config_from_json(&json!({ "tools": [], "toolChoice": { "any": {} } })).unwrap();
        assert!(matches!(any.tool_choice, Some(ToolChoice::Any(_))));
        let tool = tool_config_from_json(
            &json!({ "tools": [], "toolChoice": { "tool": { "name": "x" } } }),
        )
        .unwrap();
        assert!(matches!(tool.tool_choice, Some(ToolChoice::Tool(ref t)) if t.name == "x"));
        let none = tool_config_from_json(&json!({ "tools": [] })).unwrap();
        assert!(none.tool_choice.is_none());
    }

    #[test]
    fn text_wins_over_other_members_like_js_visit() {
        let block = content_block_from_json(&json!({
            "text": " ",
            "toolUse": { "toolUseId": "t", "name": "n", "input": {} }
        }))
        .unwrap();
        assert_eq!(block, ContentBlock::Text(" ".into()));
    }

    #[test]
    fn null_members_are_absent() {
        let block = content_block_from_json(&json!({
            "text": null,
            "cachePoint": { "type": "default" }
        }))
        .unwrap();
        assert!(matches!(block, ContentBlock::CachePoint(_)));
    }

    #[test]
    fn unsupported_block_reports_keys() {
        let err = content_block_from_json(&json!({ "mystery": {} })).unwrap_err();
        assert!(err.to_string().contains("mystery"));
    }

    #[test]
    fn guard_content_text_and_image() {
        let g = content_block_from_json(&json!({
            "guardContent": { "text": { "text": "check", "qualifiers": ["guard_content"] } }
        }))
        .unwrap();
        assert_eq!(
            content_block_to_json(&g).unwrap(),
            json!({ "guardContent": { "text": { "qualifiers": ["guard_content"], "text": "check" } } })
        );
        let gi = content_block_from_json(&json!({
            "guardContent": { "image": { "format": "png", "source": { "bytes": "AQ==" } } }
        }))
        .unwrap();
        assert_eq!(
            content_block_to_json(&gi).unwrap(),
            json!({ "guardContent": { "image": { "format": "png", "source": { "bytes": { "0": 1 } } } } })
        );
    }

    #[test]
    fn inference_and_guardrail_conversion() {
        let ic = inference_config_to_sdk(&InferenceConfig {
            max_tokens: Some(4096),
            temperature: Some(0.5),
            top_p: None,
            stop_sequences: None,
        })
        .unwrap();
        assert_eq!(ic.max_tokens, Some(4096));
        assert_eq!(ic.temperature, Some(0.5));
        assert_eq!(ic.top_p, None);
        assert!(inference_config_to_sdk(&InferenceConfig {
            max_tokens: Some(i64::MAX),
            ..Default::default()
        })
        .is_err());

        let g = guardrail_from_json(&json!({
            "guardrailIdentifier": "gid", "guardrailVersion": "DRAFT", "trace": "enabled"
        }))
        .unwrap();
        assert_eq!(g.guardrail_identifier, "gid");
        assert_eq!(g.trace, GuardrailTrace::Enabled);
        let gs = guardrail_stream_from_json(&json!({
            "guardrailIdentifier": "gid", "guardrailVersion": "1", "streamProcessingMode": "async"
        }))
        .unwrap();
        assert_eq!(
            gs.stream_processing_mode,
            GuardrailStreamProcessingMode::Async
        );
    }

    #[test]
    fn message_content_must_be_array() {
        assert!(message_from_json(&json!({ "role": "user", "content": "hi" })).is_err());
        let empty = message_from_json(&json!({ "role": "assistant" })).unwrap();
        assert!(empty.content.is_empty());
    }
}
