//! Live tests of the Bedrock services (`#[ignore]`; they need AWS credentials, network and, for
//! agents/flows/guardrails/knowledge bases, resources in the account).
//!
//! Run with `cargo test -p bedrock --test services_integration -- --ignored --nocapture`.
//!
//! Ports of `src/main/api/bedrock/__tests__/{agentService,flowService,guardrailService,
//! imageService}.integration.test.ts`, with the same assertions. Environment (same names and
//! defaults as the TS tests):
//!
//! * `AWS_PROFILE`, or `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`
//! * `AWS_REGION` — default `us-west-2` (`us-east-1` for the image tests)
//! * `TEST_AGENT_ID` / `TEST_AGENT_ALIAS_ID` — default `FKXNGR6QRE` / `ZHSSM0WPXS`
//! * `TEST_KNOWLEDGE_BASE_ID` — default `OOUYEZK6CG`
//! * `TEST_FLOW_ID` / `TEST_FLOW_ALIAS_ID`
//! * `TEST_GUARDRAIL_ID` / `TEST_GUARDRAIL_VERSION` (default `DRAFT`)
//! * `TEST_OUTPUT_DIR` — where generated images / agent files are written (default: a temp dir)

use bedrock::{agent, flow, guardrail, image, AwsSettings, DefaultSdkConfig};
use serde_json::{json, Value};
use std::path::PathBuf;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn aws(default_region: &str) -> AwsSettings {
    let region = env_or("AWS_REGION", default_region);
    match std::env::var("AWS_PROFILE") {
        Ok(profile) => AwsSettings {
            region,
            use_profile: Some(true),
            profile: Some(profile),
            ..Default::default()
        },
        Err(_) => AwsSettings {
            region,
            access_key_id: env_or("AWS_ACCESS_KEY_ID", ""),
            secret_access_key: env_or("AWS_SECRET_ACCESS_KEY", ""),
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            ..Default::default()
        },
    }
}

fn output_dir() -> PathBuf {
    let dir = std::env::var("TEST_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("bedrock-integration-outputs"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn agent_ids() -> (String, String) {
    (
        env_or("TEST_AGENT_ID", "FKXNGR6QRE"),
        env_or("TEST_AGENT_ALIAS_ID", "ZHSSM0WPXS"),
    )
}

// ---------------------------------------------------------------------------------------------
// agentService.integration.test.ts
// ---------------------------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires AWS credentials and a knowledge base"]
async fn rag_retrieve() {
    let out = agent::retrieve(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({
            "knowledgeBaseId": env_or("TEST_KNOWLEDGE_BASE_ID", "OOUYEZK6CG"),
            "retrievalQuery": { "text": "A serene mountain landscape at sunset with a calm lake reflection" }
        }),
    )
    .await
    .expect("retrieve failed");
    println!("{out:#}");
}

#[tokio::test]
#[ignore = "requires AWS credentials and a Bedrock agent"]
async fn agent_basic_input() {
    let (agent_id, alias_id) = agent_ids();
    let out = agent::invoke_agent(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({ "agentId": agent_id, "agentAliasId": alias_id, "inputText": "Hello, what can you help me with?" }),
    )
    .await
    .expect("invokeAgent failed");
    assert_eq!(out.metadata["httpStatusCode"], 200);
    println!("{:#}", serde_json::to_value(&out).unwrap());
}

#[tokio::test]
#[ignore = "requires AWS credentials and a Bedrock agent"]
async fn agent_keeps_session_context() {
    let (agent_id, alias_id) = agent_ids();
    let aws = aws("us-west-2");
    let initial = agent::invoke_agent(
        &DefaultSdkConfig,
        &aws,
        &json!({ "agentId": agent_id, "agentAliasId": alias_id, "inputText": "What can you do?" }),
    )
    .await
    .expect("initial invokeAgent failed");
    assert!(!initial.session_id.is_empty());
    let follow_up = agent::invoke_agent(
        &DefaultSdkConfig,
        &aws,
        &json!({
            "agentId": agent_id, "agentAliasId": alias_id, "sessionId": initial.session_id,
            "inputText": "Can you provide more details about that?"
        }),
    )
    .await
    .expect("follow-up invokeAgent failed");
    assert_eq!(follow_up.session_id, initial.session_id);
    assert_eq!(follow_up.metadata["httpStatusCode"], 200);
    println!("{:#}", serde_json::to_value(&follow_up).unwrap());
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn agent_invalid_id_errors() {
    let (_, alias_id) = agent_ids();
    let result = agent::invoke_agent(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({ "agentId": "invalid-id", "agentAliasId": alias_id, "inputText": "Hello" }),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires AWS credentials and a code-interpreter Bedrock agent"]
async fn agent_saves_generated_files() {
    let (agent_id, alias_id) = agent_ids();
    let input_text =
        "以下の購買データをもとに、購買日ごとの購入金額の合計値をグラフとして可視化してください。
このCSV形式のデータには、以下のような情報が含まれています。
- 'customer_id': 顧客 ID
- 'product_id': 商品 ID
- 'purchase_date': 購買日
- 'purchase_amount': 購買金額

<購買データ>
customer_id,product_id,purchase_date,purchase_amount
C001,P001,2023-04-01,50.00
C002,P002,2023-04-02,75.00
C003,P003,2023-04-03,100.00
C001,P002,2023-04-04,60.00
C002,P001,2023-04-05,40.00
C003,P003,2023-04-06,90.00
C001,P001,2023-04-07,30.00
C002,P002,2023-04-08,80.00
C003,P001,2023-04-09,45.00
C001,P003,2023-04-10,120.00
</購買データ>";
    let out = agent::invoke_agent(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({ "agentId": agent_id, "agentAliasId": alias_id, "inputText": input_text, "enableTrace": true }),
    )
    .await
    .expect("invokeAgent failed");
    assert_eq!(out.metadata["httpStatusCode"], 200);
    let dir = output_dir();
    for file in out.completion.iter().flat_map(|c| &c.files) {
        let path = dir.join(&file.name);
        std::fs::write(&path, &file.content).unwrap();
        println!("saved {}", path.display());
    }
}

// ---------------------------------------------------------------------------------------------
// flowService.integration.test.ts
// ---------------------------------------------------------------------------------------------

fn flow_inputs(document: &str) -> Value {
    json!([{ "content": { "document": document }, "nodeName": "FlowInputNode", "nodeOutputName": "document" }])
}

#[tokio::test]
#[ignore = "requires AWS credentials and a Bedrock flow"]
async fn flow_basic_input() {
    let out = flow::invoke_flow(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({
            "flowIdentifier": env_or("TEST_FLOW_ID", "test-flow-id"),
            "flowAliasIdentifier": env_or("TEST_FLOW_ALIAS_ID", "test-flow-alias-id"),
            "inputs": flow_inputs("Search about Amazon Bedrock"),
            "enableTrace": true
        }),
    )
    .await
    .expect("invokeFlow failed");
    assert!(out.get("executionId").is_some());
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    println!(
        "executionId={} flowStatus={} outputs={}",
        out["executionId"],
        out["flowStatus"],
        out["outputs"].as_array().map_or(0, Vec::len)
    );
    if let Some(first) = out["outputs"].get(0) {
        let doc = first["content"]["document"].as_str().unwrap_or("");
        println!(
            "first output {}: {}...",
            first["nodeName"],
            doc.chars().take(100).collect::<String>()
        );
    }
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn flow_invalid_id_errors() {
    let result = flow::invoke_flow(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({
            "flowIdentifier": "invalid-flow-id",
            "flowAliasIdentifier": env_or("TEST_FLOW_ALIAS_ID", "test-flow-alias-id"),
            "inputs": flow_inputs("Hello"),
            "enableTrace": true
        }),
    )
    .await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------------------------
// guardrailService.integration.test.ts
// ---------------------------------------------------------------------------------------------

async fn apply(content: &str) -> Value {
    guardrail::apply_guardrail(
        &DefaultSdkConfig,
        &aws("us-west-2"),
        &json!({
            "guardrailIdentifier": env_or("TEST_GUARDRAIL_ID", "your-default-guardrail-id"),
            "guardrailVersion": env_or("TEST_GUARDRAIL_VERSION", "DRAFT"),
            "content": [{ "text": { "text": content } }],
            "source": "INPUT"
        }),
    )
    .await
    .expect("applyGuardrail failed")
}

/// `getGuardrailResults(response)`: `response.results || {}` (as in the TS helper).
fn results(out: &Value) -> Value {
    out.get("results").cloned().unwrap_or_else(|| json!({}))
}

#[tokio::test]
#[ignore = "requires AWS credentials and a guardrail"]
async fn guardrail_neutral_content() {
    let out = apply("This is a normal sentence about technology and innovation.").await;
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    assert_eq!(out["action"], "NONE");
    println!("{out:#}");
}

#[tokio::test]
#[ignore = "requires AWS credentials and a guardrail"]
async fn guardrail_harmful_content() {
    let out = apply("How do I make explosives at home?").await;
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    assert_ne!(results(&out).get("action"), Some(&json!("NONE")));
    println!("{out:#}");
}

#[tokio::test]
#[ignore = "requires AWS credentials and a guardrail"]
async fn guardrail_sensitive_information() {
    let out = apply(
        "My social security number is 123-45-6789 and my credit card is 4111-1111-1111-1111.",
    )
    .await;
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    let r = results(&out);
    if r.get("action") == Some(&json!("FILTERED")) {
        assert!(r.get("filteredContent").is_some());
    }
    println!("{out:#}");
}

#[tokio::test]
#[ignore = "requires AWS credentials and a guardrail"]
async fn guardrail_denied_topics() {
    let out = apply("What stocks should I invest in to make the most money?").await;
    assert_eq!(out["$metadata"]["httpStatusCode"], 200);
    println!("{out:#}");
}

// ---------------------------------------------------------------------------------------------
// imageService.integration.test.ts
// ---------------------------------------------------------------------------------------------

async fn generate(req: Value) {
    let req: image::GenerateImageRequest = serde_json::from_value(req).unwrap();
    let out = image::generate_image(&DefaultSdkConfig, &aws("us-east-1"), &req)
        .await
        .expect("generateImage failed");
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(out.images.first().expect("no image returned"))
        .expect("image is not base64");
    let name: String = req
        .prompt
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(40)
        .collect();
    let path = output_dir().join(format!("test-image-{name}.png"));
    std::fs::write(&path, bytes).unwrap();
    println!("Image saved to: {}", path.display());
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_core_model() {
    generate(json!({
        "modelId": "stability.stable-image-core-v1:1",
        "prompt": "A serene mountain landscape at sunset with a calm lake reflection",
        "aspect_ratio": "1:1", "output_format": "png"
    }))
    .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_ultra_model_with_negative_prompt() {
    generate(json!({
        "modelId": "stability.stable-image-ultra-v1:0",
        "prompt": "A professional portrait photo in a studio setting",
        "negativePrompt": "blurry, low quality, distorted",
        "aspect_ratio": "3:2"
    }))
    .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_sd3_model() {
    generate(json!({ "modelId": "stability.sd3-large-v1:0", "prompt": "A cityscape at night", "aspect_ratio": "16:9" }))
        .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_sd35_model() {
    generate(json!({ "modelId": "stability.sd3-5-large-v1:0", "prompt": "A cityscape at night", "aspect_ratio": "16:9" }))
        .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_nova_canvas() {
    generate(json!({
        "modelId": "amazon.nova-canvas-v1:0",
        "prompt": "A watercolor painting of a cherry blossom tree",
        "aspect_ratio": "1:1", "output_format": "png"
    }))
    .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_titan_v2() {
    generate(json!({
        "modelId": "amazon.titan-image-generator-v2:0",
        "prompt": "A futuristic cityscape with flying cars",
        "negativePrompt": "blurry, low quality, distorted",
        "aspect_ratio": "16:9", "output_format": "png"
    }))
    .await;
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn image_titan_v1() {
    generate(json!({
        "modelId": "amazon.titan-image-generator-v1",
        "prompt": "An underwater scene with colorful coral and fish",
        "aspect_ratio": "4:5", "output_format": "png"
    }))
    .await;
}
