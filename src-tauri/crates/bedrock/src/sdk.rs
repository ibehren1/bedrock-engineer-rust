//! Shared plumbing for the non-Converse services: where `SdkConfig`s come from, and the two
//! interceptors that keep request/response JSON identical to what the JS SDK produced.
//!
//! # Why interceptors
//!
//! The TS services pass renderer/tool JSON straight into SDK commands and hand the SDK output
//! straight back (`return res`). Those shapes are deep (Knowledge Base filters, agent traces,
//! guardrail assessments, flow traces, ...), and for the REST-JSON services involved the HTTP body
//! *is* that JSON. So instead of hand-mapping hundreds of SDK types in both directions:
//!
//! * `JsonBody` replaces the serialized request body with the caller's JSON (minus the members
//!   bound to the URI or headers, which go through the typed builder). It runs before signing, so
//!   the signature covers the replaced body.
//! * `RawBody` tees the response body as the SDK reads it. For plain JSON responses that is the
//!   output object; for event streams it is the raw frames, decoded with [`decode_event_frames`]
//!   into `(event type, payload JSON)` pairs.
//!
//! The typed SDK call still drives everything (endpoint resolution, signing, retries, error
//! parsing), so errors keep their SDK names and messages.
//!
//! Differences from JS SDK output that remain: blobs inside passthrough JSON stay base64 strings
//! (the JS SDK produced `Uint8Array`s) and timestamps are normalized only under known keys
//! ([`timestamps_to_iso`]).

use crate::client::load_sdk_config;
use crate::converse::{ClientFactory, ClientFuture};
use crate::document::{blob_from_json, StringBlob};
use crate::error::{Error, Result};
use crate::settings::AwsSettings;
use aws_config::SdkConfig;
use aws_sdk_bedrockagentruntime::config::interceptors::{
    BeforeDeserializationInterceptorContextMut, BeforeTransmitInterceptorContextMut,
};
use aws_sdk_bedrockagentruntime::config::{ConfigBag, Intercept, RuntimeComponents};
use aws_smithy_types::body::SdkBody;
use aws_smithy_types::date_time::Format;
use aws_smithy_types::event_stream::HeaderValue;
use aws_smithy_types::DateTime;
use base64::Engine;
use bytes::Bytes;
use serde_json::{json, Map, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Boxed future returned by [`SdkConfigSource::sdk_config`].
pub type ConfigFuture<'a> = Pin<Box<dyn Future<Output = Result<SdkConfig>> + Send + 'a>>;

/// Produces the `SdkConfig` every service client is built from. The default is
/// [`load_sdk_config`] (the `create*Client` factories in `src/main/api/bedrock/client.ts`); tests
/// inject a fake HTTP client here.
pub trait SdkConfigSource: Send + Sync {
    fn sdk_config<'a>(&'a self, aws: &'a AwsSettings) -> ConfigFuture<'a>;
}

/// [`SdkConfigSource`] using [`load_sdk_config`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSdkConfig;

impl SdkConfigSource for DefaultSdkConfig {
    fn sdk_config<'a>(&'a self, aws: &'a AwsSettings) -> ConfigFuture<'a> {
        Box::pin(load_sdk_config(aws))
    }
}

/// Adapts an [`SdkConfigSource`] into the Converse [`ClientFactory`], so one source serves every
/// service (used by image recognition and structured output, which go through `ConverseService`).
#[derive(Clone)]
pub struct SdkConfigClients(pub Arc<dyn SdkConfigSource>);

impl ClientFactory for SdkConfigClients {
    fn client<'a>(&'a self, aws: &'a AwsSettings) -> ClientFuture<'a> {
        Box::pin(async move {
            let conf = self.0.sdk_config(aws).await?;
            Ok(aws_sdk_bedrockruntime::Client::new(&conf))
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Request body replacement
// ---------------------------------------------------------------------------------------------

/// Replaces the request body with fixed JSON before signing (see the module docs).
#[derive(Debug, Clone)]
pub(crate) struct JsonBody(Arc<Vec<u8>>);

impl JsonBody {
    pub(crate) fn new(body: &Value) -> Self {
        Self(Arc::new(serde_json::to_vec(body).unwrap_or_default()))
    }
}

impl Intercept for JsonBody {
    fn name(&self) -> &'static str {
        "JsonBody"
    }

    fn modify_before_signing(
        &self,
        context: &mut BeforeTransmitInterceptorContextMut<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> std::result::Result<(), BoxError> {
        let req = context.request_mut();
        *req.body_mut() = SdkBody::from(self.0.as_ref().clone());
        req.headers_mut()
            .insert("content-length", self.0.len().to_string());
        req.headers_mut().insert("content-type", "application/json");
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Response capture
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Captured {
    status: Option<u16>,
    request_id: Option<String>,
    bytes: Vec<u8>,
}

/// Tees the response body (see the module docs). Each attempt resets the capture, so only the
/// final response is kept.
#[derive(Debug, Clone, Default)]
pub(crate) struct RawBody(Arc<Mutex<Captured>>);

impl RawBody {
    /// The bytes read so far.
    pub(crate) fn bytes(&self) -> Vec<u8> {
        self.0.lock().map(|c| c.bytes.clone()).unwrap_or_default()
    }

    /// The captured body parsed as a JSON object (`{}` when empty or not an object).
    pub(crate) fn json_object(&self) -> Map<String, Value> {
        match serde_json::from_slice::<Value>(&self.bytes()) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }

    /// `$metadata` for the captured response.
    pub(crate) fn metadata(&self) -> Value {
        let c = self.0.lock();
        let (status, request_id) = match &c {
            Ok(c) => (c.status, c.request_id.clone()),
            Err(_) => (None, None),
        };
        metadata_json(status.unwrap_or(200), request_id.as_deref())
    }
}

impl Intercept for RawBody {
    fn name(&self) -> &'static str {
        "RawBody"
    }

    fn modify_before_deserialization(
        &self,
        context: &mut BeforeDeserializationInterceptorContextMut<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> std::result::Result<(), BoxError> {
        let resp = context.response_mut();
        if let Ok(mut c) = self.0.lock() {
            c.status = Some(resp.status().as_u16());
            c.request_id = resp
                .headers()
                .get("x-amzn-requestid")
                .or_else(|| resp.headers().get("x-amz-request-id"))
                .map(str::to_string);
            c.bytes.clear();
        }
        let inner = std::mem::replace(resp.body_mut(), SdkBody::taken());
        *resp.body_mut() = SdkBody::from_body_1_x(Tee {
            inner,
            sink: self.0.clone(),
        });
        Ok(())
    }
}

struct Tee {
    inner: SdkBody,
    sink: Arc<Mutex<Captured>>,
}

impl http_body::Body for Tee {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<http_body::Frame<Bytes>, BoxError>>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_frame(cx);
        if let Poll::Ready(Some(Ok(frame))) = &polled {
            if let Some(data) = frame.data_ref() {
                if let Ok(mut c) = this.sink.lock() {
                    c.bytes.extend_from_slice(data);
                }
            }
        }
        polled
    }

    fn is_end_stream(&self) -> bool {
        http_body::Body::is_end_stream(&self.inner)
    }

    fn size_hint(&self) -> http_body::SizeHint {
        http_body::Body::size_hint(&self.inner)
    }
}

/// Decode `application/vnd.amazon.eventstream` bytes into `(event type, payload)` pairs for the
/// `event` messages (exceptions are reported by the typed SDK receiver instead). Payloads that are
/// not JSON become `null`. Stops at the first incomplete or malformed frame.
pub fn decode_event_frames(bytes: &[u8]) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut buf: &[u8] = bytes;
    while !buf.is_empty() {
        let Ok(message) = aws_smithy_eventstream::frame::read_message_from(&mut buf) else {
            break;
        };
        let header = |name: &str| {
            message
                .headers()
                .iter()
                .find(|h| h.name().as_str() == name)
                .and_then(|h| match h.value() {
                    HeaderValue::String(s) => Some(s.as_str().to_string()),
                    _ => None,
                })
        };
        if header(":message-type").as_deref() != Some("event") {
            continue;
        }
        let Some(event_type) = header(":event-type") else {
            continue;
        };
        let payload = serde_json::from_slice(message.payload()).unwrap_or(Value::Null);
        out.push((event_type, payload));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------------------------

/// `$metadata` as the JS SDK attached it to every output: `httpStatusCode`, `requestId` (when
/// known), `attempts` and `totalRetryDelay` (`1` / `0`: SDK-internal retry counts are not exposed).
pub fn metadata_json(http_status_code: u16, request_id: Option<&str>) -> Value {
    let mut meta = Map::new();
    meta.insert("httpStatusCode".into(), json!(http_status_code));
    if let Some(id) = request_id {
        meta.insert("requestId".into(), json!(id));
    }
    meta.insert("attempts".into(), json!(1));
    meta.insert("totalRetryDelay".into(), json!(0));
    Value::Object(meta)
}

/// `Date.prototype.toISOString()`: `YYYY-MM-DDTHH:MM:SS.mmmZ`.
pub fn iso_millis(dt: &DateTime) -> String {
    let base = DateTime::from_secs(dt.secs())
        .fmt(Format::DateTime)
        .unwrap_or_default();
    let prefix = base.get(..19).unwrap_or(&base);
    format!("{prefix}.{:03}Z", dt.subsec_nanos() / 1_000_000)
}

/// The current time as [`iso_millis`] (`new Date()` serialized).
pub fn now_iso() -> String {
    iso_millis(&DateTime::from(std::time::SystemTime::now()))
}

/// Rewrites timestamps under any of `keys` (at any depth) to `Date.prototype.toISOString()`
/// form — what the JS SDK's `Date` values became once the result was `JSON.stringify`d. Accepts
/// the wire's RFC 3339 strings (with any offset / precision) and epoch-second numbers; other
/// values are left alone.
pub fn timestamps_to_iso(value: &mut Value, keys: &[&str]) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if keys.contains(&k.as_str()) {
                    let parsed = match v {
                        Value::Number(n) => n.as_f64().map(DateTime::from_secs_f64),
                        Value::String(s) => DateTime::from_str(s, Format::DateTimeWithOffset).ok(),
                        _ => None,
                    };
                    if let Some(dt) = parsed {
                        *v = Value::String(iso_millis(&dt));
                        continue;
                    }
                }
                timestamps_to_iso(v, keys);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| timestamps_to_iso(v, keys)),
        _ => {}
    }
}

/// Re-encode the blob at `v` (base64 string, `{"0":n,...}`, number array or Buffer JSON) as the
/// base64 string the wire expects. Strings are taken to be base64 already.
pub(crate) fn blob_to_wire(v: &mut Value, what: &str) -> Result<()> {
    if v.is_string() {
        return Ok(());
    }
    let blob = blob_from_json(v, StringBlob::Base64)
        .ok_or_else(|| Error::InvalidRequest(format!("{what}: expected bytes")))?;
    *v = Value::String(base64::engine::general_purpose::STANDARD.encode(blob.as_ref()));
    Ok(())
}

/// Apply [`blob_to_wire`] to `obj[list][*]...path` for every element of the array at `list`.
pub(crate) fn blobs_in_list(obj: &mut Value, list: &[&str], path: &[&str]) -> Result<()> {
    let Some(items) = pointer_mut(obj, list).and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for item in items {
        if let Some(v) = pointer_mut(item, path) {
            if !v.is_null() {
                blob_to_wire(v, &path.join("."))?;
            }
        }
    }
    Ok(())
}

fn pointer_mut<'a>(mut v: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    for key in path {
        v = v.get_mut(*key)?;
    }
    Some(v)
}

/// JS truthiness for an optional JSON value.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `value.as_object()` or an `InvalidRequest` naming `what`.
pub(crate) fn object_of<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| Error::InvalidRequest(format!("{what} must be an object")))
}

/// A non-empty string member (URI/header labels), else `InvalidRequest`.
pub(crate) fn required_str(m: &Map<String, Value>, key: &str) -> Result<String> {
    match m.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => Ok(s.to_string()),
        _ => Err(Error::InvalidRequest(format!("{key} is required"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_millis_matches_to_iso_string() {
        assert_eq!(
            iso_millis(&DateTime::from_secs_f64(1_700_000_000.5)),
            "2023-11-14T22:13:20.500Z"
        );
        assert_eq!(
            iso_millis(&DateTime::from_secs(0)),
            "1970-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn timestamps_are_rewritten_at_any_depth() {
        let mut v = json!({
            "eventTime": 1_700_000_000,
            "trace": { "metadata": { "startTime": 1.25, "note": 3 } },
            "list": [{ "endTime": "1970-01-01T09:00:00.123456+09:00" }],
            "already": { "eventTime": "2020-01-01T00:00:00Z" },
            "junk": { "eventTime": "yesterday" }
        });
        timestamps_to_iso(&mut v, &["eventTime", "startTime", "endTime"]);
        assert_eq!(v["eventTime"], "2023-11-14T22:13:20.000Z");
        assert_eq!(
            v["trace"]["metadata"]["startTime"],
            "1970-01-01T00:00:01.250Z"
        );
        assert_eq!(v["trace"]["metadata"]["note"], 3);
        assert_eq!(v["list"][0]["endTime"], "1970-01-01T00:00:00.123Z");
        assert_eq!(v["already"]["eventTime"], "2020-01-01T00:00:00.000Z");
        assert_eq!(v["junk"]["eventTime"], "yesterday");
    }

    #[test]
    fn blobs_are_normalized_to_base64() {
        let mut v = json!({ "files": [
            { "source": { "byteContent": { "data": { "0": 104, "1": 105 } } } },
            { "source": { "byteContent": { "data": "aGk=" } } },
            { "source": { "byteContent": { "data": { "type": "Buffer", "data": [104, 105] } } } },
            { "source": { "s3Location": { "uri": "s3://b/k" } } }
        ] });
        blobs_in_list(&mut v, &["files"], &["source", "byteContent", "data"]).unwrap();
        for i in 0..3 {
            assert_eq!(v["files"][i]["source"]["byteContent"]["data"], "aGk=");
        }
        assert!(v["files"][3]["source"].get("byteContent").is_none());

        let mut bad = json!({ "files": [{ "source": { "byteContent": { "data": { "x": 1 } } } }] });
        assert!(blobs_in_list(&mut bad, &["files"], &["source", "byteContent", "data"]).is_err());
    }

    #[test]
    fn metadata_shape() {
        assert_eq!(
            metadata_json(200, Some("r")),
            json!({ "httpStatusCode": 200, "requestId": "r", "attempts": 1, "totalRetryDelay": 0 })
        );
    }

    #[test]
    fn truthiness() {
        assert!(!truthy(None));
        assert!(!truthy(Some(&json!(""))));
        assert!(!truthy(Some(&json!(0))));
        assert!(truthy(Some(&json!({}))));
        assert!(truthy(Some(&json!([]))));
    }
}
