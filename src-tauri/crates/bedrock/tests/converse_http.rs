//! End-to-end tests of `ConverseService` over a fake HTTP client: the real SDK serializes the
//! request and decodes real event-stream frames, so these pin both the wire request JSON and the
//! emitted stream-event JSON.

use aws_config::retry::RetryConfig;
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_bedrockruntime::config::Region;
use aws_sdk_bedrockruntime::Client;
use aws_smithy_eventstream::frame::write_message_to;
use aws_smithy_http_client::test_util::infallible_client_fn;
use aws_smithy_types::body::SdkBody;
use aws_smithy_types::event_stream::{Header, HeaderValue, Message};
use bedrock::converse::ClientFuture;
use bedrock::{
    AwsSettings, BedrockSettings, CacheConfig, CancellationToken, ClientFactory, ConverseRequest,
    ConverseService, ConverseSettings, Error, InferenceConfig, ModelInfo, RetryPolicy,
    ThinkingMode, ThinkingType,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct Recorded {
    uri: String,
    body: Value,
}

#[derive(Clone)]
struct Fake {
    responses: Arc<Mutex<VecDeque<http::Response<SdkBody>>>>,
    requests: Arc<Mutex<Vec<Recorded>>>,
    regions: Arc<Mutex<Vec<String>>>,
}

impl Fake {
    fn new(responses: Vec<http::Response<SdkBody>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::default(),
            regions: Arc::default(),
        }
    }
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

impl ClientFactory for Fake {
    fn client<'a>(&'a self, aws: &'a AwsSettings) -> ClientFuture<'a> {
        self.regions.lock().unwrap().push(aws.region.clone());
        let responses = self.responses.clone();
        let requests = self.requests.clone();
        let http_client = infallible_client_fn(move |req: http::Request<SdkBody>| {
            let body = req
                .body()
                .bytes()
                .map(|b| serde_json::from_slice(b).unwrap_or(Value::Null))
                .unwrap_or(Value::Null);
            requests.lock().unwrap().push(Recorded {
                uri: req.uri().to_string(),
                body,
            });
            responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("no scripted response left")
        });
        let conf = aws_sdk_bedrockruntime::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(aws.region.clone()))
            .credentials_provider(Credentials::new("AKID", "SECRET", None, None, "test"))
            .retry_config(RetryConfig::disabled())
            .http_client(http_client)
            .build();
        Box::pin(async move { Ok(Client::from_conf(conf)) })
    }
}

struct Info;
impl ModelInfo for Info {
    fn supports_thinking(&self, model_id: &str) -> bool {
        model_id.contains("claude-sonnet-4")
    }
    fn supported_thinking_types(&self, _: &str) -> Vec<ThinkingType> {
        vec![ThinkingType::Enabled]
    }
    fn max_tokens_limit(&self, _: &str) -> Option<i64> {
        Some(64000)
    }
    fn regions(&self, _: &str) -> Option<Vec<String>> {
        Some(vec!["us-west-2".into(), "us-east-1".into()])
    }
    fn cache_config(&self, _: &str) -> Option<CacheConfig> {
        None
    }
}

fn event_frame(event_type: &'static str, payload: Value) -> Vec<u8> {
    let msg = Message::new(serde_json::to_vec(&payload).unwrap())
        .add_header(Header::new(
            ":message-type",
            HeaderValue::String("event".into()),
        ))
        .add_header(Header::new(
            ":event-type",
            HeaderValue::String(event_type.into()),
        ))
        .add_header(Header::new(
            ":content-type",
            HeaderValue::String("application/json".into()),
        ));
    let mut buf = Vec::new();
    write_message_to(&msg, &mut buf).unwrap();
    buf
}

fn exception_frame(exception_type: &'static str, message: &str) -> Vec<u8> {
    let msg = Message::new(serde_json::to_vec(&json!({ "message": message })).unwrap())
        .add_header(Header::new(
            ":message-type",
            HeaderValue::String("exception".into()),
        ))
        .add_header(Header::new(
            ":exception-type",
            HeaderValue::String(exception_type.into()),
        ))
        .add_header(Header::new(
            ":content-type",
            HeaderValue::String("application/json".into()),
        ));
    let mut buf = Vec::new();
    write_message_to(&msg, &mut buf).unwrap();
    buf
}

fn stream_response(frames: Vec<Vec<u8>>) -> http::Response<SdkBody> {
    http::Response::builder()
        .status(200)
        .header("content-type", "application/vnd.amazon.eventstream")
        .body(SdkBody::from(frames.concat()))
        .unwrap()
}

fn error_response(status: u16, error_type: &str, message: &str) -> http::Response<SdkBody> {
    http::Response::builder()
        .status(status)
        .header("x-amzn-errortype", error_type)
        .header("x-amzn-requestid", "req-1")
        .header("content-type", "application/json")
        .body(SdkBody::from(
            serde_json::to_vec(&json!({ "message": message })).unwrap(),
        ))
        .unwrap()
}

fn settings() -> ConverseSettings {
    ConverseSettings {
        aws: AwsSettings {
            region: "us-west-2".into(),
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            ..Default::default()
        },
        inference_params: InferenceConfig {
            max_tokens: Some(4096),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        thinking_mode: Some(ThinkingMode {
            kind: Some("enabled".into()),
            budget_tokens: Some(1024),
        }),
        ..Default::default()
    }
}

fn service(fake: &Fake) -> ConverseService {
    ConverseService::new(Arc::new(Info))
        .with_client_factory(Arc::new(fake.clone()))
        .with_retry_policy(RetryPolicy {
            max_retries: 3,
            delay: Duration::from_millis(1),
        })
}

fn chat_request() -> ConverseRequest {
    serde_json::from_value(json!({
        "modelId": "us.anthropic.claude-sonnet-4-20250514-v1:0",
        "system": [{ "text": "You are helpful." }, { "cachePoint": { "type": "default" } }],
        "messages": [
            { "id": "m0", "role": "user", "content": [
                { "text": "look" },
                { "image": { "format": "png", "source": { "bytes": { "0": 1, "1": 2, "2": 3 } } } }
            ] },
            { "id": "m1", "role": "assistant", "metadata": { "modelId": "x" }, "content": [
                { "reasoningContent": { "reasoningText": { "text": "thinking", "signature": "sig" } } },
                { "toolUse": { "toolUseId": "t1", "name": "readFiles", "input": { "paths": ["a"] } } }
            ] },
            { "role": "user", "content": [
                { "toolResult": { "toolUseId": "t1", "content": [{ "json": { "ok": true } }], "status": "success" } },
                { "cachePoint": { "type": "default" } }
            ] }
        ],
        "toolConfig": { "tools": [
            { "toolSpec": { "name": "readFiles", "description": "Read", "inputSchema": { "json": { "type": "object" } } } },
            { "cachePoint": { "type": "default" } }
        ] }
    }))
    .unwrap()
}

fn standard_stream() -> Vec<Vec<u8>> {
    vec![
        event_frame("messageStart", json!({ "role": "assistant" })),
        event_frame(
            "contentBlockDelta",
            json!({ "contentBlockIndex": 0, "delta": { "reasoningContent": { "text": "Let me" } } }),
        ),
        event_frame(
            "contentBlockDelta",
            json!({ "contentBlockIndex": 0, "delta": { "reasoningContent": { "signature": "EqQB" } } }),
        ),
        event_frame("contentBlockStop", json!({ "contentBlockIndex": 0 })),
        event_frame(
            "contentBlockDelta",
            json!({ "contentBlockIndex": 1, "delta": { "text": "Hi" } }),
        ),
        event_frame("contentBlockStop", json!({ "contentBlockIndex": 1 })),
        event_frame(
            "contentBlockStart",
            json!({ "contentBlockIndex": 2, "start": { "toolUse": { "toolUseId": "tu2", "name": "listFiles" } } }),
        ),
        event_frame(
            "contentBlockDelta",
            json!({ "contentBlockIndex": 2, "delta": { "toolUse": { "input": "{\"path\":" } } }),
        ),
        event_frame("contentBlockStop", json!({ "contentBlockIndex": 2 })),
        event_frame("messageStop", json!({ "stopReason": "tool_use" })),
        event_frame(
            "metadata",
            json!({
                "usage": { "inputTokens": 10, "outputTokens": 20, "totalTokens": 30, "cacheReadInputTokens": 4, "cacheWriteInputTokens": 6 },
                "metrics": { "latencyMs": 812 }
            }),
        ),
    ]
}

#[tokio::test]
async fn stream_emits_express_identical_lines_and_sends_expected_request() {
    let fake = Fake::new(vec![stream_response(standard_stream())]);
    let mut lines = Vec::new();
    service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |v| {
                lines.push(serde_json::to_string(&v).unwrap());
                Ok(())
            },
        )
        .await
        .unwrap();

    assert_eq!(
        lines,
        vec![
            r#"{"messageStart":{"role":"assistant"}}"#,
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"text":"Let me"}}}}"#,
            r#"{"contentBlockDelta":{"contentBlockIndex":0,"delta":{"reasoningContent":{"signature":"EqQB"}}}}"#,
            r#"{"contentBlockStop":{"contentBlockIndex":0}}"#,
            r#"{"contentBlockDelta":{"contentBlockIndex":1,"delta":{"text":"Hi"}}}"#,
            r#"{"contentBlockStop":{"contentBlockIndex":1}}"#,
            r#"{"contentBlockStart":{"contentBlockIndex":2,"start":{"toolUse":{"name":"listFiles","toolUseId":"tu2"}}}}"#,
            r#"{"contentBlockDelta":{"contentBlockIndex":2,"delta":{"toolUse":{"input":"{\"path\":"}}}}"#,
            r#"{"contentBlockStop":{"contentBlockIndex":2}}"#,
            r#"{"messageStop":{"stopReason":"tool_use"}}"#,
            r#"{"metadata":{"metrics":{"latencyMs":812},"usage":{"cacheReadInputTokens":4,"cacheWriteInputTokens":6,"inputTokens":10,"outputTokens":20,"totalTokens":30}}}"#,
        ]
    );

    let reqs = fake.requests();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0]
            .uri
            .starts_with("https://bedrock-runtime.us-west-2.amazonaws.com/model/")
            && reqs[0].uri.ends_with("/converse-stream"),
        "{}",
        reqs[0].uri
    );
    let body = &reqs[0].body;
    assert_eq!(
        body["system"],
        json!([{ "text": "You are helpful." }, { "cachePoint": { "type": "default" } }])
    );
    assert_eq!(
        body["messages"][0]["content"][1],
        json!({ "image": { "format": "png", "source": { "bytes": "AQID" } } })
    );
    assert_eq!(
        body["messages"][1],
        json!({ "role": "assistant", "content": [
            { "reasoningContent": { "reasoningText": { "text": "thinking", "signature": "sig" } } },
            { "toolUse": { "toolUseId": "t1", "name": "readFiles", "input": { "paths": ["a"] } } }
        ] })
    );
    assert_eq!(
        body["messages"][2]["content"],
        json!([
            { "toolResult": { "toolUseId": "t1", "content": [{ "json": { "ok": true } }], "status": "success" } },
            { "cachePoint": { "type": "default" } }
        ])
    );
    assert_eq!(
        body["toolConfig"],
        json!({ "tools": [
            { "toolSpec": { "name": "readFiles", "description": "Read", "inputSchema": { "json": { "type": "object" } } } },
            { "cachePoint": { "type": "default" } }
        ] })
    );
    // thinking on: temperature forced to 1, topP dropped, budget passed through
    assert_eq!(
        body["inferenceConfig"],
        json!({ "maxTokens": 4096, "temperature": 1.0 })
    );
    assert_eq!(
        body["additionalModelRequestFields"],
        json!({ "thinking": { "type": "enabled", "budget_tokens": 1024 } })
    );
    assert!(body.get("guardrailConfig").is_none());
}

#[tokio::test]
async fn redacted_reasoning_is_uint8array_json() {
    let fake = Fake::new(vec![stream_response(vec![event_frame(
        "contentBlockDelta",
        // wire format: base64 blob
        json!({ "contentBlockIndex": 0, "delta": { "reasoningContent": { "redactedContent": "AQI=" } } }),
    )])]);
    let mut events = Vec::new();
    service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |v| {
                events.push(v);
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(
        events,
        vec![
            json!({ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "reasoningContent": { "redactedContent": { "0": 1, "1": 2 } } } } })
        ]
    );
}

#[tokio::test]
async fn throttling_is_retried_with_region_failover() {
    let fake = Fake::new(vec![
        error_response(429, "ThrottlingException", "Too many requests"),
        stream_response(vec![event_frame(
            "messageStart",
            json!({ "role": "assistant" }),
        )]),
    ]);
    let mut s = settings();
    s.bedrock_settings = Some(BedrockSettings {
        enable_region_failover: true,
        available_failover_regions: vec![],
    });
    let mut n = 0;
    service(&fake)
        .converse_stream(&s, &chat_request(), &CancellationToken::new(), |_| {
            n += 1;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        *fake.regions.lock().unwrap(),
        vec!["us-west-2", "us-east-1"]
    );
    let reqs = fake.requests();
    assert!(reqs[1]
        .uri
        .contains("bedrock-runtime.us-east-1.amazonaws.com"));
}

#[tokio::test]
async fn throttling_without_failover_retries_same_region() {
    let fake = Fake::new(vec![
        error_response(429, "ThrottlingException", "Too many requests"),
        error_response(503, "ServiceUnavailableException", "down"),
        stream_response(vec![]),
    ]);
    service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .await
        .unwrap();
    assert_eq!(*fake.regions.lock().unwrap(), vec!["us-west-2"; 3]);
}

#[tokio::test]
async fn validation_error_is_not_retried_and_keeps_express_body() {
    let fake = Fake::new(vec![error_response(
        400,
        "ValidationException",
        "bad input",
    )]);
    let err = service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .await
        .unwrap_err();
    assert_eq!(err.service_name(), Some("ValidationException"));
    let body = err.to_json();
    assert_eq!(body["name"], "ValidationException");
    assert_eq!(body["message"], "bad input");
    assert_eq!(body["$fault"], "client");
    assert_eq!(body["$metadata"]["httpStatusCode"], 400);
    assert_eq!(body["$metadata"]["requestId"], "req-1");
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn in_stream_exception_surfaces_after_delivered_events() {
    let fake = Fake::new(vec![stream_response(vec![
        event_frame("messageStart", json!({ "role": "assistant" })),
        exception_frame("modelStreamErrorException", "model blew up"),
    ])]);
    let mut events = Vec::new();
    let err = service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |v| {
                events.push(v);
                Ok(())
            },
        )
        .await
        .unwrap_err();
    assert_eq!(events.len(), 1);
    assert_eq!(err.service_name(), Some("ModelStreamErrorException"));
    assert!(err.to_string().contains("model blew up"), "{err}");
}

#[tokio::test]
async fn sink_error_stops_stream() {
    let fake = Fake::new(vec![stream_response(standard_stream())]);
    let mut seen = 0;
    let err = service(&fake)
        .converse_stream(
            &settings(),
            &chat_request(),
            &CancellationToken::new(),
            |_| {
                seen += 1;
                if seen == 2 {
                    Err("channel closed".into())
                } else {
                    Ok(())
                }
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Sink(ref m) if m == "channel closed"));
    assert_eq!(seen, 2);
}

#[tokio::test]
async fn cancelled_before_send() {
    let fake = Fake::new(vec![stream_response(standard_stream())]);
    let token = CancellationToken::new();
    token.cancel();
    let err = service(&fake)
        .converse_stream(&settings(), &chat_request(), &token, |_| Ok(()))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
    assert_eq!(err.to_json()["name"], "AbortError");
}

#[tokio::test]
async fn invalid_request_fails_locally() {
    let fake = Fake::new(vec![]);
    let mut req = chat_request();
    req.messages = vec![json!({ "role": "user", "content": [{ "unknownBlock": {} }] })];
    let err = service(&fake)
        .converse_stream(&settings(), &req, &CancellationToken::new(), |_| Ok(()))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidRequest(_)));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn non_streaming_converse_body_and_guardrail_from_settings() {
    let response_body = json!({
        "output": { "message": { "role": "assistant", "content": [
            { "text": "Hello" },
            { "toolUse": { "toolUseId": "t9", "name": "think", "input": { "thought": "x" } } }
        ] } },
        "stopReason": "tool_use",
        "usage": { "inputTokens": 3, "outputTokens": 4, "totalTokens": 7 },
        "metrics": { "latencyMs": 99 }
    });
    let fake = Fake::new(vec![http::Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .header("x-amzn-requestid", "rid-9")
        .body(SdkBody::from(serde_json::to_vec(&response_body).unwrap()))
        .unwrap()]);
    let mut s = settings();
    s.guardrail_settings = Some(bedrock::GuardrailSettings {
        enabled: true,
        guardrail_identifier: "gid".into(),
        guardrail_version: "DRAFT".into(),
        trace: Some("enabled".into()),
    });
    let mut req = chat_request();
    req.disable_thinking = Some(true);
    let out = service(&fake)
        .converse(&s, &req, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        out,
        json!({
            "$metadata": { "httpStatusCode": 200, "requestId": "rid-9", "attempts": 1, "totalRetryDelay": 0 },
            "metrics": { "latencyMs": 99 },
            "output": { "message": { "content": [
                { "text": "Hello" },
                { "toolUse": { "input": { "thought": "x" }, "name": "think", "toolUseId": "t9" } }
            ], "role": "assistant" } },
            "stopReason": "tool_use",
            "usage": { "inputTokens": 3, "outputTokens": 4, "totalTokens": 7 }
        })
    );
    let reqs = fake.requests();
    assert!(reqs[0].uri.ends_with("/converse"), "{}", reqs[0].uri);
    assert_eq!(
        reqs[0].body["guardrailConfig"],
        json!({ "guardrailIdentifier": "gid", "guardrailVersion": "DRAFT", "trace": "enabled" })
    );
    // disableThinking: no thinking field, Claude topP dropped because temperature is set
    assert!(reqs[0].body.get("additionalModelRequestFields").is_none());
    assert_eq!(
        reqs[0].body["inferenceConfig"],
        json!({ "maxTokens": 4096, "temperature": 0.5 })
    );
}
