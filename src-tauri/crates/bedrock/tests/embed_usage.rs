//! Titan embeddings over a fake HTTP client: a batch of texts is one usage event. In its own
//! test binary because it installs the process-wide usage sink.

use aws_config::retry::RetryConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use aws_smithy_http_client::test_util::infallible_client_fn;
use aws_smithy_types::body::SdkBody;
use bedrock::embed::{self, InputType};
use bedrock::usage::{UsageEvent, UsageSink};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct Capture(Mutex<Vec<UsageEvent>>);

impl UsageSink for Capture {
    fn record(&self, event: UsageEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// A client whose every call returns a 2-dimension embedding; the text `fail` gets a 400 and
/// the text `uncounted` a response without a token count.
fn client() -> aws_sdk_bedrockruntime::Client {
    let http_client = infallible_client_fn(|req: http::Request<SdkBody>| {
        let body: serde_json::Value =
            serde_json::from_slice(req.body().bytes().unwrap_or_default()).unwrap();
        let (status, reply) = match body["inputText"].as_str().unwrap() {
            "fail" => (400, json!({ "message": "bad input" })),
            "uncounted" => (200, json!({ "embedding": [1.0, 0.0] })),
            _ => (
                200,
                json!({ "embedding": [0.0, 1.0], "inputTextTokenCount": 7 }),
            ),
        };
        http::Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .header("x-amzn-errortype", "ValidationException")
            .body(SdkBody::from(serde_json::to_vec(&reply).unwrap()))
            .unwrap()
    });
    let conf = SdkConfig::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-west-2"))
        .credentials_provider(SharedCredentialsProvider::new(Credentials::new(
            "AKID", "SECRET", None, None, "test",
        )))
        .retry_config(RetryConfig::disabled())
        .http_client(http_client)
        .build();
    aws_sdk_bedrockruntime::Client::new(&conf)
}

#[tokio::test]
async fn titan_batch_is_one_usage_event() {
    let sink = Arc::new(Capture(Mutex::new(Vec::new())));
    bedrock::usage::set_sink(sink.clone());
    let model = "amazon.titan-embed-text-v2:0";
    let c = client();

    let texts: Vec<String> = ["a", "b", "uncounted"].map(String::from).to_vec();
    let out = embed::embed_titan(&c, model, &texts, InputType::Document, 2, 4)
        .await
        .unwrap();
    assert_eq!(out, vec![vec![0.0, 1.0], vec![0.0, 1.0], vec![1.0, 0.0]]);
    {
        let events = sink.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.calls, 3);
        // 7 + 7 reported, plus an estimate for "uncounted" (9 chars, ~3 tokens).
        assert_eq!(e.input_tokens, 17);
        assert!(e.estimated);
        assert_eq!(e.context.feature, "kb-ingest");
    }

    // A failed call fails the batch, but the calls that succeeded are still billed.
    sink.0.lock().unwrap().clear();
    let texts: Vec<String> = ["a", "fail"].map(String::from).to_vec();
    assert!(embed::embed_titan(&c, model, &texts, InputType::Query, 2, 1)
        .await
        .is_err());
    let events = sink.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].calls, 1);
    assert_eq!(events[0].input_tokens, 7);
    assert!(!events[0].estimated);
    assert_eq!(events[0].context.feature, "kb-search");
}
