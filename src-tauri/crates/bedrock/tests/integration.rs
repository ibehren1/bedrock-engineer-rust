//! Live Bedrock tests (`#[ignore]`; they need AWS credentials and network).
//!
//! Run with `cargo test -p bedrock --test integration -- --ignored --nocapture`.
//!
//! Environment:
//! * `AWS_PROFILE` — profile to use (otherwise `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` /
//!   `AWS_SESSION_TOKEN`).
//! * `AWS_REGION` — region for the smoke test (default `us-west-2`).
//! * `BEDROCK_TEST_MODEL_ID` — model for the smoke test
//!   (default `us.anthropic.claude-haiku-4-5-20251001-v1:0`).
//! * `BEDROCK_TEST_MODELS` — for the connectivity sweep: comma-separated `modelId@region` pairs.
//!   When unset, the sweep covers every registry model (`allModels`) in every region of
//!   `BEDROCK_TEST_REGIONS` (comma-separated; default: the 14 regions of the TS test).
//!
//! Port of `src/main/api/bedrock/__tests__/modelRegionConnectivity.integration.test.ts`.

use bedrock::{
    runtime_client, AwsSettings, CancellationToken, ConverseRequest, ConverseService,
    ConverseSettings, InferenceConfig,
};
use serde_json::json;

fn aws_from_env(region: &str) -> AwsSettings {
    match std::env::var("AWS_PROFILE") {
        Ok(profile) => AwsSettings {
            region: region.to_string(),
            use_profile: Some(true),
            profile: Some(profile),
            ..Default::default()
        },
        Err(_) => AwsSettings {
            region: region.to_string(),
            access_key_id: std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default(),
            secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default(),
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            ..Default::default()
        },
    }
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn converse_stream_smoke() {
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-2".into());
    let model_id = std::env::var("BEDROCK_TEST_MODEL_ID")
        .unwrap_or_else(|_| "us.anthropic.claude-haiku-4-5-20251001-v1:0".into());
    let settings = ConverseSettings {
        aws: aws_from_env(&region),
        inference_params: InferenceConfig {
            max_tokens: Some(64),
            temperature: Some(0.5),
            top_p: None,
            stop_sequences: None,
        },
        ..Default::default()
    };
    let req: ConverseRequest = serde_json::from_value(json!({
        "modelId": model_id,
        "system": [{ "text": "You are a helpful assistant." }],
        "messages": [{ "role": "user", "content": [{ "text": "Say hello." }] }]
    }))
    .unwrap();
    let mut events = Vec::new();
    ConverseService::default()
        .converse_stream(&settings, &req, &CancellationToken::new(), |e| {
            println!("{e}");
            events.push(e);
            Ok(())
        })
        .await
        .expect("stream failed");
    assert!(events.first().unwrap().get("messageStart").is_some());
    assert!(events.iter().any(|e| e.get("messageStop").is_some()));
    assert!(events.iter().any(|e| e.get("metadata").is_some()));
}

/// `TEST_REGIONS` of the TS test.
const TEST_REGIONS: [&str; 14] = [
    "us-east-1",
    "us-west-2",
    "us-east-2",
    "us-west-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-north-1",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-northeast-3",
    "ap-south-1",
    "ap-southeast-1",
    "ap-southeast-2",
];

/// Same inference config rules as the TS test: maxTokens 10; no sampling fields for
/// reasoning-effort / Kimi K3 models; no topP for Claude Sonnet 4.5 and Nova.
fn connectivity_inference(model_id: &str) -> aws_sdk_bedrockruntime::types::InferenceConfiguration {
    let skip_sampling = model_id.contains("openai.gpt-5")
        || model_id.contains("openai.gpt-6")
        || model_id.contains("xai.grok")
        || model_id.contains("kimi-k3");
    let mut b = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder().max_tokens(10);
    if !skip_sampling {
        b = b.temperature(1.0);
        let skip_top_p = model_id.contains("claude-sonnet-4-5")
            || model_id.contains("nova-")
            || model_id.contains("nova.");
        if !skip_top_p {
            b = b.top_p(0.9);
        }
    }
    b.build()
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn model_region_connectivity() {
    use aws_sdk_bedrockruntime::types::{
        ContentBlock, ConversationRole, Message, SystemContentBlock,
    };
    let pairs: Vec<(String, String)> = match std::env::var("BEDROCK_TEST_MODELS") {
        Ok(pairs) => pairs
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (m, r) = p.split_once('@').expect("expected modelId@region");
                (m.to_string(), r.to_string())
            })
            .collect(),
        Err(_) => {
            let regions =
                std::env::var("BEDROCK_TEST_REGIONS").unwrap_or_else(|_| TEST_REGIONS.join(","));
            regions
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .flat_map(|region| {
                    models::all_models()
                        .iter()
                        .filter(move |m| m.regions.iter().any(|r| r == region))
                        .map(move |m| (m.model_id.clone(), region.to_string()))
                })
                .collect()
        }
    };
    let mut failures = Vec::new();
    for (model_id, region) in &pairs {
        let (model_id, region) = (model_id.as_str(), region.as_str());
        let pair = format!("{model_id}@{region}");
        let client = runtime_client(&aws_from_env(region)).await.unwrap();
        let result = client
            .converse()
            .model_id(model_id)
            .messages(
                Message::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text("Hello".into()))
                    .build()
                    .unwrap(),
            )
            .system(SystemContentBlock::Text(
                "You are a helpful assistant.".into(),
            ))
            .inference_config(connectivity_inference(model_id))
            .send()
            .await;
        match result {
            Ok(out) => {
                assert!(out.output.is_some(), "{pair}: no output");
                println!("ok   {pair}");
            }
            Err(e) => {
                let e = bedrock::ServiceError::from_sdk(&e);
                println!("FAIL {pair}: {} {}", e.name, e.message);
                failures.push(format!("{pair}: {}", e.name));
            }
        }
    }
    assert!(failures.is_empty(), "failed: {failures:?}");
}
