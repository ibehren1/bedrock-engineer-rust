//! The non-Converse services over a fake HTTP client: the real SDKs build, sign and send the
//! requests and parse the scripted responses, so these pin the wire request (URI, headers, body)
//! and the JSON handed back to the renderer / tools.

use aws_config::retry::RetryConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use aws_smithy_eventstream::frame::write_message_to;
use aws_smithy_http_client::test_util::infallible_client_fn;
use aws_smithy_types::body::SdkBody;
use aws_smithy_types::event_stream::{Header, HeaderValue, Message};
use base64::Engine;
use bedrock::sdk::ConfigFuture;
use bedrock::translate::{TranslateService, TranslateTextOptions};
use bedrock::{
    agent, flow, guardrail, image, image_recognition, inference_profile, structured_output,
    AwsSettings, ConverseService, ConverseSettings, RetryPolicy, SdkConfigClients, SdkConfigSource,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    uri: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Default)]
struct Fake {
    responses: Arc<Mutex<VecDeque<http::Response<SdkBody>>>>,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl Fake {
    fn new(responses: Vec<http::Response<SdkBody>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::default(),
        })
    }
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

impl SdkConfigSource for Fake {
    fn sdk_config<'a>(&'a self, aws: &'a AwsSettings) -> ConfigFuture<'a> {
        let responses = self.responses.clone();
        let requests = self.requests.clone();
        let http_client = infallible_client_fn(move |req: http::Request<SdkBody>| {
            let body = req
                .body()
                .bytes()
                .map(|b| serde_json::from_slice(b).unwrap_or(Value::Null))
                .unwrap_or(Value::Null);
            requests.lock().unwrap().push(Recorded {
                method: req.method().to_string(),
                uri: req.uri().to_string(),
                headers: req
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect(),
                body,
            });
            responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("no scripted response left")
        });
        let conf = SdkConfig::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(aws.region.clone()))
            .credentials_provider(SharedCredentialsProvider::new(Credentials::new(
                "AKID", "SECRET", None, None, "test",
            )))
            .retry_config(RetryConfig::disabled())
            .http_client(http_client)
            .build();
        Box::pin(async move { Ok(conf) })
    }
}

fn aws() -> AwsSettings {
    AwsSettings {
        region: "us-west-2".into(),
        access_key_id: "AKID".into(),
        secret_access_key: "SECRET".into(),
        ..Default::default()
    }
}

fn json_response(status: u16, headers: &[(&str, &str)], body: Value) -> http::Response<SdkBody> {
    let mut b = http::Response::builder()
        .status(status)
        .header("content-type", "application/json");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(SdkBody::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn error_response(status: u16, error_type: &str, message: &str) -> http::Response<SdkBody> {
    json_response(
        status,
        &[
            ("x-amzn-errortype", error_type),
            ("x-amzn-requestid", "err-1"),
        ],
        json!({ "message": message }),
    )
}

fn frame(message_type: &str, type_header: &str, kind: &str, payload: Value) -> Vec<u8> {
    let msg = Message::new(serde_json::to_vec(&payload).unwrap())
        .add_header(Header::new(
            ":message-type",
            HeaderValue::String(message_type.to_string().into()),
        ))
        .add_header(Header::new(
            type_header.to_string(),
            HeaderValue::String(kind.to_string().into()),
        ))
        .add_header(Header::new(
            ":content-type",
            HeaderValue::String("application/json".into()),
        ));
    let mut buf = Vec::new();
    write_message_to(&msg, &mut buf).unwrap();
    buf
}

fn event(kind: &str, payload: Value) -> Vec<u8> {
    frame("event", ":event-type", kind, payload)
}

fn exception(kind: &str, message: &str) -> Vec<u8> {
    frame(
        "exception",
        ":exception-type",
        kind,
        json!({ "message": message }),
    )
}

fn stream_response(headers: &[(&str, &str)], frames: Vec<Vec<u8>>) -> http::Response<SdkBody> {
    let mut b = http::Response::builder()
        .status(200)
        .header("content-type", "application/vnd.amazon.eventstream");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(SdkBody::from(frames.concat())).unwrap()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ---------------------------------------------------------------------------------------------
// Image generation
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn generate_image_sends_model_body_and_maps_response() {
    let fake = Fake::new(vec![json_response(
        200,
        &[],
        json!({ "images": ["iVBOR"], "seeds": [5], "finish_reasons": [null] }),
    )]);
    let req: image::GenerateImageRequest = serde_json::from_value(json!({
        "modelId": "stability.stable-image-core-v1:1", "prompt": "mountains",
        "aspect_ratio": "1:1", "output_format": "png"
    }))
    .unwrap();
    let out = image::generate_image(fake.as_ref(), &aws(), &req)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&out).unwrap(),
        json!({ "images": ["iVBOR"], "seeds": [5], "finish_reasons": [null] })
    );
    let r = &fake.requests()[0];
    assert_eq!(r.method, "POST");
    assert!(
        r.uri
            .ends_with("/model/stability.stable-image-core-v1%3A1/invoke"),
        "{}",
        r.uri
    );
    assert_eq!(r.header("accept"), Some("application/json"));
    assert_eq!(
        r.body,
        json!({ "prompt": "mountains", "aspect_ratio": "1:1", "output_format": "png" })
    );
}

#[tokio::test]
async fn generate_image_error_messages() {
    let fake = Fake::new(vec![
        error_response(400, "ValidationException", "bad size"),
        error_response(403, "UnrecognizedClientException", "who?"),
        json_response(200, &[], json!({ "error": "unsafe prompt" })),
    ]);
    let req = image::GenerateImageRequest {
        model_id: "amazon.nova-canvas-v1:0".into(),
        prompt: "p".into(),
        ..Default::default()
    };
    let e = image::generate_image(fake.as_ref(), &aws(), &req)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Invalid request parameters: bad size");
    let e = image::generate_image(fake.as_ref(), &aws(), &req)
        .await
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "AWS authentication failed. Please check your credentials and permissions."
    );
    let e = image::generate_image(fake.as_ref(), &aws(), &req)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Nova error: unsafe prompt");

    let unsupported = image::GenerateImageRequest {
        model_id: "amazon.nova-reel-v1:0".into(),
        ..req
    };
    let e = image::generate_image(fake.as_ref(), &aws(), &unsupported)
        .await
        .unwrap_err();
    assert!(e
        .to_string()
        .starts_with("Model amazon.nova-reel-v1:0 is not supported. Supported models: stability.sd3-large-v1:0, "));
    assert_eq!(fake.requests().len(), 3);
}

// ---------------------------------------------------------------------------------------------
// Guardrails
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn apply_guardrail_passes_output_through_and_retries_throttling() {
    let body = json!({
        "usage": { "topicPolicyUnits": 1 },
        "action": "GUARDRAIL_INTERVENED",
        "actionReason": "blocked",
        "outputs": [{ "text": "Sorry." }],
        "assessments": [{ "topicPolicy": { "topics": [{ "name": "Investing", "type": "DENY", "action": "BLOCKED", "detected": true }] } }],
        "guardrailCoverage": { "textCharacters": { "guarded": 10, "total": 10 } }
    });
    let fake = Fake::new(vec![
        error_response(429, "ThrottlingException", "slow down"),
        json_response(200, &[("x-amzn-requestid", "rid-9")], body.clone()),
    ]);
    let request = json!({
        "guardrailIdentifier": "gr-1", "guardrailVersion": "DRAFT", "source": "INPUT",
        "content": [{ "text": { "text": "What stocks should I buy?" } }]
    });
    let out = guardrail::apply_guardrail_with_policy(
        fake.as_ref(),
        &aws(),
        &request,
        RetryPolicy {
            max_retries: 3,
            delay: Duration::from_millis(1),
        },
    )
    .await
    .unwrap();
    let mut expected = json!({
        "$metadata": { "httpStatusCode": 200, "requestId": "rid-9", "attempts": 1, "totalRetryDelay": 0 }
    });
    expected
        .as_object_mut()
        .unwrap()
        .extend(body.as_object().unwrap().clone());
    assert_eq!(out, expected);

    let reqs = fake.requests();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[1].uri.ends_with("/guardrail/gr-1/version/DRAFT/apply"));
    assert_eq!(
        reqs[1].body,
        json!({ "source": "INPUT", "content": [{ "text": { "text": "What stocks should I buy?" } }] })
    );
    assert_eq!(
        reqs[1].header("content-length"),
        Some(
            serde_json::to_vec(&reqs[1].body)
                .unwrap()
                .len()
                .to_string()
                .as_str()
        )
    );
}

#[tokio::test]
async fn apply_guardrail_does_not_retry_validation_errors() {
    let fake = Fake::new(vec![error_response(
        400,
        "ValidationException",
        "no such guardrail",
    )]);
    let e = guardrail::apply_guardrail(
        fake.as_ref(),
        &aws(),
        &json!({ "guardrailIdentifier": "x", "guardrailVersion": "1", "source": "INPUT", "content": [] }),
    )
    .await
    .unwrap_err();
    assert_eq!(e.service_name(), Some("ValidationException"));
    assert_eq!(e.message(), "no such guardrail");
    assert_eq!(fake.requests().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Agents and knowledge bases
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn invoke_agent_aggregates_the_stream() {
    let trace = json!({
        "agentId": "AG", "agentAliasId": "AL", "sessionId": "S1", "agentVersion": "1",
        "eventTime": "2025-01-02T03:04:05.678901Z",
        "trace": { "orchestrationTrace": { "rationale": { "traceId": "t", "text": "thinking" } } }
    });
    let fake = Fake::new(vec![stream_response(
        &[
            ("x-amzn-bedrock-agent-content-type", "application/json"),
            ("x-amz-bedrock-agent-session-id", "S1"),
            ("x-amzn-requestid", "rid-a"),
        ],
        vec![
            event("trace", trace.clone()),
            event("chunk", json!({ "bytes": b64("Hello, ".as_bytes()) })),
            event("chunk", json!({ "bytes": b64("world".as_bytes()) })),
            event(
                "files",
                json!({ "files": [
                    { "name": "chart.png", "type": "image/png", "bytes": b64(&[137, 80]) },
                    { "name": "chart.png", "type": "image/png", "bytes": b64(&[1]) }
                ] }),
            ),
        ],
    )]);
    let out = agent::invoke_agent(
        fake.as_ref(),
        &aws(),
        &json!({
            "agentId": "AG", "agentAliasId": "AL", "sessionId": "S1", "inputText": "Plot it",
            "enableTrace": true,
            "sessionState": { "files": [{ "name": "d.csv", "useCase": "CODE_INTERPRETER",
                "source": { "sourceType": "BYTE_CONTENT", "byteContent": { "mediaType": "text/csv", "data": [97] } } }] }
        }),
    )
    .await
    .unwrap();

    let r = &fake.requests()[0];
    assert!(
        r.uri
            .ends_with("/agents/AG/agentAliases/AL/sessions/S1/text"),
        "{}",
        r.uri
    );
    assert_eq!(
        r.body,
        json!({
            "inputText": "Plot it", "enableTrace": true,
            "sessionState": { "files": [{ "name": "d.csv", "useCase": "CODE_INTERPRETER",
                "source": { "sourceType": "BYTE_CONTENT", "byteContent": { "mediaType": "text/csv", "data": "YQ==" } } }] }
        })
    );

    let json = serde_json::to_value(&out).unwrap();
    assert_eq!(json["$metadata"]["httpStatusCode"], 200);
    assert_eq!(json["$metadata"]["requestId"], "rid-a");
    assert_eq!(json["contentType"], "application/json");
    assert_eq!(json["sessionId"], "S1");
    assert_eq!(json["completion"]["message"], "Hello, world");
    assert_eq!(
        json["completion"]["files"],
        json!([{ "name": "chart.png", "content": { "0": 137, "1": 80 } }])
    );
    let mut expected_trace = trace;
    expected_trace["eventTime"] = json!("2025-01-02T03:04:05.678Z");
    assert_eq!(json["completion"]["traces"], json!([expected_trace]));
}

#[tokio::test]
async fn invoke_agent_generates_a_session_id_and_fails_on_stream_errors() {
    let fake = Fake::new(vec![stream_response(
        &[
            ("x-amzn-bedrock-agent-content-type", "application/json"),
            ("x-amz-bedrock-agent-session-id", "gen"),
        ],
        vec![
            event("chunk", json!({ "bytes": b64(b"partial") })),
            exception("throttlingException", "Rate exceeded"),
        ],
    )]);
    let e = agent::invoke_agent(
        fake.as_ref(),
        &aws(),
        &json!({ "agentId": "AG", "agentAliasId": "AL", "inputText": "hi" }),
    )
    .await
    .unwrap_err();
    assert_eq!(e.message(), "Rate exceeded");
    let r = &fake.requests()[0];
    assert!(r.uri.contains("/sessions/session_"), "{}", r.uri);
    assert_eq!(r.body, json!({ "inputText": "hi", "enableTrace": false }));
}

#[tokio::test]
async fn retrieve_passes_request_and_response_through() {
    let results = json!({
        "retrievalResults": [{
            "content": { "text": "Bedrock is...", "type": "TEXT" },
            "location": { "type": "S3", "s3Location": { "uri": "s3://kb/doc.pdf" } },
            "score": 0.87,
            "metadata": { "x-amz-bedrock-kb-source-uri": "s3://kb/doc.pdf", "page": 3 }
        }],
        "nextToken": "n"
    });
    let fake = Fake::new(vec![json_response(200, &[], results.clone())]);
    let input = agent::retrieve_input_from_ipc(&json!({
        "knowledgeBaseId": "KB1", "query": "what is bedrock",
        "retrievalConfiguration": { "vectorSearchConfiguration": {
            "numberOfResults": 3, "overrideSearchType": "HYBRID",
            "filter": { "andAll": [{ "equals": { "key": "lang", "value": "en" } }, { "greaterThan": { "key": "year", "value": 2020 } }] }
        } }
    }));
    let out = agent::retrieve(fake.as_ref(), &aws(), &input)
        .await
        .unwrap();
    assert_eq!(out["retrievalResults"], results["retrievalResults"]);
    assert_eq!(out["nextToken"], "n");
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);

    let r = &fake.requests()[0];
    assert!(r.uri.ends_with("/knowledgebases/KB1/retrieve"), "{}", r.uri);
    assert_eq!(
        r.body,
        json!({
            "retrievalQuery": { "text": "what is bedrock" },
            "retrievalConfiguration": { "vectorSearchConfiguration": {
                "numberOfResults": 3, "overrideSearchType": "HYBRID",
                "filter": { "andAll": [{ "equals": { "key": "lang", "value": "en" } }, { "greaterThan": { "key": "year", "value": 2020 } }] }
            } }
        })
    );
}

#[tokio::test]
async fn retrieve_and_generate_round_trip_and_not_found() {
    let response = json!({
        "sessionId": "rag-1",
        "output": { "text": "Answer [1]" },
        "citations": [{ "generatedResponsePart": { "textResponsePart": { "text": "Answer", "span": { "start": 0, "end": 5 } } },
            "retrievedReferences": [{ "content": { "text": "src" }, "location": { "type": "WEB", "webLocation": { "url": "https://x" } } }] }]
    });
    let fake = Fake::new(vec![
        json_response(200, &[], response.clone()),
        error_response(404, "ResourceNotFoundException", "KB not found"),
    ]);
    let input = json!({
        "input": { "text": "question" },
        "retrieveAndGenerateConfiguration": {
            "type": "KNOWLEDGE_BASE",
            "knowledgeBaseConfiguration": {
                "knowledgeBaseId": "KB1",
                "modelArn": "arn:aws:bedrock:us-west-2::foundation-model/anthropic.claude-3-haiku-20240307-v1:0",
                "generationConfiguration": { "promptTemplate": { "textPromptTemplate": "$search_results$" } }
            }
        }
    });
    let out = agent::retrieve_and_generate(fake.as_ref(), &aws(), &input)
        .await
        .unwrap();
    assert_eq!(out["sessionId"], "rag-1");
    assert_eq!(out["citations"], response["citations"]);
    assert_eq!(out["output"], response["output"]);
    let r = &fake.requests()[0];
    assert!(r.uri.ends_with("/retrieveAndGenerate"), "{}", r.uri);
    assert_eq!(r.body, input);

    let e = agent::retrieve_and_generate(fake.as_ref(), &aws(), &input)
        .await
        .unwrap_err();
    assert_eq!(e.service_name(), Some("ResourceNotFoundException"));
    assert_eq!(e.to_json()["message"], "KB not found");
}

// ---------------------------------------------------------------------------------------------
// Flows
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn invoke_flow_collects_outputs_and_events() {
    let fake = Fake::new(vec![stream_response(
        &[("x-amz-bedrock-flow-execution-id", "exec-1")],
        vec![
            event(
                "flowTraceEvent",
                json!({ "trace": { "nodeInputTrace": {
                "nodeName": "FlowInputNode", "timestamp": "2025-01-01T00:00:00Z", "fields": []
            } } }),
            ),
            event(
                "flowOutputEvent",
                json!({ "nodeName": "FlowOutputNode", "nodeType": "FlowOutputNode",
                "content": { "document": "Bedrock is a service" } }),
            ),
            event(
                "flowCompletionEvent",
                json!({ "completionReason": "SUCCESS" }),
            ),
        ],
    )]);
    let params = flow::invoke_flow_input_from_ipc(&json!({
        "flowIdentifier": "FLOW", "flowAliasIdentifier": "ALIAS",
        "input": { "content": { "document": "Search about Amazon Bedrock" }, "nodeName": "FlowInputNode", "nodeOutputName": "document" }
    }));
    let out = flow::invoke_flow(fake.as_ref(), &aws(), &params)
        .await
        .unwrap();
    assert_eq!(out["executionId"], "exec-1");
    assert_eq!(out["flowStatus"], "SUCCESS");
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    assert_eq!(
        out["outputs"],
        json!([{ "content": { "document": "Bedrock is a service" }, "nodeName": "FlowOutputNode", "nodeOutputName": "document" }])
    );
    assert_eq!(out["events"].as_array().unwrap().len(), 3);
    assert_eq!(
        out["events"][0]["flowTraceEvent"]["trace"]["nodeInputTrace"]["timestamp"],
        "2025-01-01T00:00:00.000Z"
    );
    assert_eq!(out["requiresInput"], false);

    let r = &fake.requests()[0];
    assert!(r.uri.ends_with("/flows/FLOW/aliases/ALIAS"), "{}", r.uri);
    assert_eq!(
        r.body,
        json!({ "inputs": [{ "content": { "document": "Search about Amazon Bedrock" }, "nodeName": "FlowInputNode", "nodeOutputName": "document" }], "enableTrace": false })
    );
}

#[tokio::test]
async fn invoke_flow_session_context_error_becomes_session_error() {
    let fake = Fake::new(vec![
        stream_response(
            &[],
            vec![exception(
                "validationException",
                "Error retrieving session context: expired",
            )],
        ),
        stream_response(&[], vec![exception("validationException", "Bad input")]),
        error_response(404, "ResourceNotFoundException", "no flow"),
    ]);
    let params = json!({ "flowIdentifier": "F", "flowAliasIdentifier": "A", "inputs": [] });
    let out = flow::invoke_flow(fake.as_ref(), &aws(), &params)
        .await
        .unwrap();
    assert_eq!(out["flowStatus"], "SESSION_ERROR");
    assert_eq!(out["executionId"], "");
    assert_eq!(out["outputs"], json!([]));
    assert_eq!(out["requiresInput"], false);

    let e = flow::invoke_flow(fake.as_ref(), &aws(), &params)
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Bad input");
    let e = flow::invoke_flow(fake.as_ref(), &aws(), &params)
        .await
        .unwrap_err();
    assert_eq!(e.service_name(), Some("ResourceNotFoundException"));
}

// ---------------------------------------------------------------------------------------------
// Translate
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn translate_text_request_result_and_cache() {
    let fake = Fake::new(vec![json_response(
        200,
        &[],
        json!({
            "TranslatedText": "こんにちは", "SourceLanguageCode": "en", "TargetLanguageCode": "ja",
            "AppliedTerminologies": [{ "Name": "brand" }],
            "AppliedSettings": { "Profanity": "MASK", "Formality": "FORMAL" }
        }),
    )]);
    let svc = TranslateService::new();
    let options = TranslateTextOptions {
        text: "Hello".into(),
        target_language: "ja".into(),
        ..Default::default()
    };
    let out = svc
        .translate_text(fake.as_ref(), &aws(), &options)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&out).unwrap(),
        json!({
            "translatedText": "こんにちは", "sourceLanguage": "en", "targetLanguage": "ja",
            "originalText": "Hello", "appliedTerminologies": ["brand"],
            "appliedSettings": { "profanity": "MASK", "formality": "FORMAL" }
        })
    );
    let r = &fake.requests()[0];
    assert_eq!(
        r.header("x-amz-target"),
        Some("AWSShineFrontendService_20170701.TranslateText")
    );
    assert_eq!(
        r.body,
        json!({
            "Text": "Hello", "SourceLanguageCode": "auto", "TargetLanguageCode": "ja",
            "Settings": { "Formality": "FORMAL", "Profanity": "MASK" }
        })
    );

    // Second call is served from the cache (no scripted response left).
    let again = svc
        .translate_text(fake.as_ref(), &aws(), &options)
        .await
        .unwrap();
    assert_eq!(again, out);
    assert_eq!(fake.requests().len(), 1);
    assert_eq!(
        svc.get_cached_translation("Hello", "auto", "ja"),
        Some(out.clone())
    );
    assert_eq!(svc.get_cached_translation("Hello", "en", "ja"), None);
    assert_eq!(svc.cache_stats().size, 1);
    svc.clear_cache();
    assert_eq!(svc.cache_stats().size, 0);
}

#[tokio::test]
async fn translate_validation_and_errors() {
    let fake = Fake::new(vec![error_response(
        400,
        "UnsupportedLanguagePairException",
        "en to xx unsupported",
    )]);
    let svc = TranslateService::new();
    let opts = |text: &str| TranslateTextOptions {
        text: text.into(),
        source_language: Some("en".into()),
        target_language: "xx".into(),
        cache_key: None,
    };
    let e = svc
        .translate_text(fake.as_ref(), &aws(), &opts("   "))
        .await
        .unwrap_err();
    assert_eq!(
        e.to_json(),
        json!({ "name": "Error", "message": "Text cannot be empty" })
    );
    let e = svc
        .translate_text(fake.as_ref(), &aws(), &opts(&"a".repeat(10_001)))
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Text too long (max 10,000 characters)");
    let e = svc
        .translate_text(fake.as_ref(), &aws(), &opts("Hello"))
        .await
        .unwrap_err();
    assert_eq!(
        e.to_json(),
        json!({
            "name": "UnsupportedLanguagePairException", "message": "en to xx unsupported",
            "code": "UnsupportedLanguagePairException", "originalText": "Hello", "requestId": "err-1"
        })
    );
}

#[tokio::test]
async fn translate_batch_keeps_successes_in_order() {
    let ok = |t: &str| {
        json_response(
            200,
            &[],
            json!({ "TranslatedText": t, "SourceLanguageCode": "en", "TargetLanguageCode": "ja" }),
        )
    };
    // Futures are polled in order, so requests go out in input order.
    let fake = Fake::new(vec![
        ok("一"),
        error_response(400, "ValidationException", "bad"),
        ok("三"),
    ]);
    let svc = TranslateService::new();
    let texts: Vec<TranslateTextOptions> = ["one", "two", "three"]
        .iter()
        .map(|t| TranslateTextOptions {
            text: (*t).into(),
            source_language: Some("en".into()),
            target_language: "ja".into(),
            cache_key: None,
        })
        .collect();
    let out = svc.translate_batch(fake.as_ref(), &aws(), &texts).await;
    let got: Vec<(&str, &str)> = out
        .iter()
        .map(|r| (r.original_text.as_str(), r.translated_text.as_str()))
        .collect();
    assert_eq!(got, vec![("one", "一"), ("three", "三")]);
}

// ---------------------------------------------------------------------------------------------
// Inference profiles
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn list_application_inference_profiles_maps_and_swallows_errors() {
    let fake = Fake::new(vec![
        json_response(
            200,
            &[],
            json!({ "inferenceProfileSummaries": [
                {
                    "inferenceProfileName": "my-profile",
                    "inferenceProfileArn": "arn:aws:bedrock:us-west-2:123:application-inference-profile/abc",
                    "description": "team profile",
                    "createdAt": "2025-01-01T00:00:00Z",
                    "updatedAt": "2025-02-01T12:30:00.5Z",
                    "models": [{ "modelArn": "arn:aws:bedrock:us-west-2::foundation-model/amazon.nova-pro-v1:0" }],
                    "inferenceProfileId": "abc",
                    "status": "ACTIVE",
                    "type": "APPLICATION"
                }
            ] }),
        ),
        error_response(403, "AccessDeniedException", "denied"),
    ]);
    let profiles =
        inference_profile::list_application_inference_profiles(fake.as_ref(), &aws()).await;
    assert_eq!(
        serde_json::to_value(&profiles).unwrap(),
        json!([{
            "inferenceProfileArn": "arn:aws:bedrock:us-west-2:123:application-inference-profile/abc",
            "inferenceProfileName": "my-profile",
            "description": "team profile",
            "status": "ACTIVE",
            "createdAt": "2025-01-01T00:00:00.000Z",
            "updatedAt": "2025-02-01T12:30:00.500Z",
            "modelSource": { "copyFrom": "arn:aws:bedrock:us-west-2::foundation-model/amazon.nova-pro-v1:0" },
            "type": "APPLICATION"
        }])
    );
    let r = &fake.requests()[0];
    assert_eq!(r.method, "GET");
    assert!(r.uri.contains("/inference-profiles?"), "{}", r.uri);
    assert!(r.uri.contains("type=APPLICATION"), "{}", r.uri);

    assert!(
        inference_profile::list_application_inference_profiles(fake.as_ref(), &aws())
            .await
            .is_empty()
    );
}

// ---------------------------------------------------------------------------------------------
// Converse-backed services
// ---------------------------------------------------------------------------------------------

fn converse_settings() -> ConverseSettings {
    ConverseSettings {
        aws: aws(),
        ..Default::default()
    }
}

fn converse_service(fake: &Arc<Fake>) -> ConverseService {
    ConverseService::default()
        .with_client_factory(Arc::new(SdkConfigClients(fake.clone())))
        .with_retry_policy(RetryPolicy {
            max_retries: 0,
            delay: Duration::from_millis(1),
        })
}

fn converse_reply(content: Value) -> http::Response<SdkBody> {
    json_response(
        200,
        &[],
        json!({
            "output": { "message": { "role": "assistant", "content": content } },
            "stopReason": "tool_use",
            "usage": { "inputTokens": 1, "outputTokens": 2, "totalTokens": 3 },
            "metrics": { "latencyMs": 5 }
        }),
    )
}

#[tokio::test]
async fn structured_output_forces_tool_and_returns_input() {
    let fake = Fake::new(vec![
        converse_reply(json!([
            { "toolUse": { "toolUseId": "t1", "name": "recommend_website_changes",
                "input": { "recommendations": [{ "title": "Nav", "value": "Add a menu" }] } } }
        ])),
        converse_reply(json!([{ "text": "no tool" }])),
        error_response(400, "ValidationException", "toolChoice not supported"),
    ]);
    let svc = converse_service(&fake);
    let req = structured_output::WebsiteRecommendationsRequest {
        website_code: "<html></html>".into(),
        language: "English".into(),
        model_id: "us.anthropic.claude-sonnet-4-20250514-v1:0".into(),
    };
    let out = structured_output::get_website_recommendations(&svc, &converse_settings(), &req)
        .await
        .unwrap();
    assert_eq!(
        out,
        json!({ "recommendations": [{ "title": "Nav", "value": "Add a menu" }] })
    );
    let r = &fake.requests()[0];
    assert!(r.uri.contains("/converse"), "{}", r.uri);
    assert_eq!(
        r.body["toolConfig"]["toolChoice"],
        json!({ "tool": { "name": "recommend_website_changes" } })
    );
    assert_eq!(
        r.body["toolConfig"]["tools"][0]["toolSpec"]["inputSchema"]["json"]["required"],
        json!(["recommendations"])
    );
    assert_eq!(r.body["messages"][0]["content"][0]["text"], "<html></html>");
    assert_eq!(r.body["inferenceConfig"]["maxTokens"], 2048);

    let e = structured_output::get_website_recommendations(&svc, &converse_settings(), &req)
        .await
        .unwrap_err();
    assert_eq!(e.to_json()["code"], "MISSING_OUTPUT");
    assert_eq!(e.to_json()["name"], "StructuredOutputError");

    let e = structured_output::get_website_recommendations(&svc, &converse_settings(), &req)
        .await
        .unwrap_err();
    assert_eq!(e.to_json()["code"], "VALIDATION_ERROR");
    assert_eq!(e.to_json()["message"], "toolChoice not supported");
    assert_eq!(
        e.to_json()["details"]["originalError"],
        "toolChoice not supported"
    );
}

#[tokio::test]
async fn recognize_image_claude_and_nova_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("icon.png");
    std::fs::write(&path, [137u8, 80, 78, 71]).unwrap();
    let path = path.to_str().unwrap().to_string();

    let fake = Fake::new(vec![
        json_response(
            200,
            &[],
            json!({ "content": [{ "type": "text", "text": "A small icon." }] }),
        ),
        converse_reply(json!([{ "text": "An icon" }, { "text": "with colors" }])),
    ]);
    let svc = converse_service(&fake);
    let claude = image_recognition::RecognizeImageRequest {
        image_path: path.clone(),
        prompt: None,
        model_id: Some("anthropic.claude-3-haiku-20240307-v1:0".into()),
    };
    let text =
        image_recognition::recognize_image(fake.as_ref(), &svc, &converse_settings(), &claude)
            .await
            .unwrap();
    assert_eq!(text, "A small icon.");
    let r = &fake.requests()[0];
    assert!(r.uri.ends_with("/invoke"), "{}", r.uri);
    let image = &r.body["messages"][0]["content"][0];
    assert_eq!(image["source"]["media_type"], "image/png");
    assert_eq!(image["source"]["data"], b64(&[137, 80, 78, 71]));
    assert_eq!(
        r.body["messages"][0]["content"][1]["text"],
        image_recognition::DEFAULT_RECOGNITION_PROMPT
    );

    let nova = image_recognition::RecognizeImageRequest {
        image_path: path,
        prompt: Some("Describe".into()),
        model_id: None,
    };
    let text = image_recognition::recognize_image(fake.as_ref(), &svc, &converse_settings(), &nova)
        .await
        .unwrap();
    assert_eq!(text, "An icon\nwith colors");
    let r = &fake.requests()[1];
    assert!(
        r.uri.contains("/model/amazon.nova-lite-v1%3A0/converse"),
        "{}",
        r.uri
    );
    let block = &r.body["messages"][0]["content"][0]["image"];
    assert_eq!(block["format"], "png");
    assert_eq!(block["source"]["bytes"], b64(&[137, 80, 78, 71]));
    assert_eq!(r.body["messages"][0]["content"][1]["text"], "Describe");
}
