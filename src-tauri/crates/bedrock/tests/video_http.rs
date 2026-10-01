//! Nova Reel video generation (`bedrock::video`) over a fake HTTP client: the real Bedrock Runtime
//! and S3 SDKs build, sign and send the requests and parse the scripted responses.

use aws_config::retry::RetryConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use aws_smithy_http_client::test_util::infallible_client_fn;
use aws_smithy_types::body::SdkBody;
use bedrock::sdk::ConfigFuture;
use bedrock::video::{self, GenerateMovieRequest};
use bedrock::{AwsSettings, SdkConfigSource};
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

fn aws(region: &str) -> AwsSettings {
    AwsSettings {
        region: region.into(),
        access_key_id: "AKID".into(),
        secret_access_key: "SECRET".into(),
        ..Default::default()
    }
}

fn json_response(status: u16, body: Value) -> http::Response<SdkBody> {
    http::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(SdkBody::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn error_response(status: u16, error_type: &str, message: &str) -> http::Response<SdkBody> {
    http::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-amzn-errortype", error_type)
        .body(SdkBody::from(
            serde_json::to_vec(&json!({ "message": message })).unwrap(),
        ))
        .unwrap()
}

fn empty_ok() -> http::Response<SdkBody> {
    http::Response::builder()
        .status(200)
        .body(SdkBody::empty())
        .unwrap()
}

fn bytes_response(bytes: &[u8]) -> http::Response<SdkBody> {
    http::Response::builder()
        .status(200)
        .header("content-type", "video/mp4")
        .header("content-length", bytes.len().to_string())
        .body(SdkBody::from(bytes.to_vec()))
        .unwrap()
}

const ARN: &str = "arn:aws:bedrock:us-east-1:123456789012:async-invoke/abc123";

fn movie(prompt: &str, duration: f64) -> GenerateMovieRequest {
    GenerateMovieRequest {
        prompt: prompt.into(),
        duration_seconds: duration,
        s3_uri: "s3://my-bucket/videos".into(),
        ..Default::default()
    }
}

fn status_body(status: &str) -> Value {
    json!({
        "invocationArn": ARN,
        "modelArn": "arn:aws:bedrock:us-east-1::foundation-model/amazon.nova-reel-v1:1",
        "status": status,
        "submitTime": "2025-01-02T03:04:05.678Z",
        "endTime": "2025-01-02T03:10:00Z",
        "outputDataConfig": { "s3OutputDataConfig": { "s3Uri": "s3://my-bucket/videos/abc123" } }
    })
}

#[tokio::test]
async fn start_text_video_sends_start_async_invoke() {
    let fake = Fake::new(vec![json_response(200, json!({ "invocationArn": ARN }))]);
    let mut req = movie("a cat surfing", 6.0);
    req.seed = Some(42.0);
    let out = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &req)
        .await
        .unwrap();
    let v = serde_json::to_value(&out).unwrap();
    assert_eq!(v["invocationArn"], ARN);
    assert_eq!(v["status"]["invocationArn"], ARN);
    assert_eq!(v["status"]["modelId"], "amazon.nova-reel-v1:1");
    assert_eq!(v["status"]["status"], "InProgress");
    assert!(v["status"]["submitTime"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        v["status"]["outputDataConfig"],
        json!({ "s3OutputDataConfig": { "s3Uri": "s3://my-bucket/videos" } })
    );
    assert!(v.get("localPath").is_none() && v.get("error").is_none());

    let r = &fake.requests()[0];
    assert_eq!(r.method, "POST");
    assert!(r.uri.ends_with("/async-invoke"), "{}", r.uri);
    assert!(r.uri.contains("bedrock-runtime.us-east-1"), "{}", r.uri);
    assert_eq!(r.body["modelId"], "amazon.nova-reel-v1:1");
    assert_eq!(
        r.body["modelInput"],
        json!({
            "taskType": "TEXT_VIDEO",
            "textToVideoParams": { "text": "a cat surfing" },
            "videoGenerationConfig": {
                "durationSeconds": 6, "fps": 24, "dimension": "1280x720", "seed": 42
            }
        })
    );
    assert_eq!(
        r.body["outputDataConfig"],
        json!({ "s3OutputDataConfig": { "s3Uri": "s3://my-bucket/videos" } })
    );
}

#[tokio::test]
async fn v1_0_regions_send_text_video_and_unsupported_regions_fail_first() {
    let fake = Fake::new(vec![json_response(200, json!({ "invocationArn": ARN }))]);
    let out = video::start_video_generation(fake.as_ref(), &aws("eu-west-1"), &movie("p", 24.0))
        .await
        .unwrap();
    assert_eq!(out.status.model_id, "amazon.nova-reel-v1:0");
    let body = &fake.requests()[0].body;
    assert_eq!(body["modelId"], "amazon.nova-reel-v1:0");
    assert_eq!(body["modelInput"]["taskType"], "TEXT_VIDEO");
    assert_eq!(
        body["modelInput"]["videoGenerationConfig"]["durationSeconds"],
        24
    );

    let fake = Fake::new(vec![]);
    let e = video::start_video_generation(fake.as_ref(), &aws("us-west-2"), &movie("p", 6.0))
        .await
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Nova Reel is not available in region us-west-2. Supported regions: us-east-1, eu-west-1, ap-northeast-1"
    );
    // Validation runs before anything else.
    let e = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &movie("p", 5.0))
        .await
        .unwrap_err();
    assert!(e.to_string().starts_with("Duration must be 6 seconds"));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn start_errors_are_reworded() {
    let cases = [
        (
            400,
            "ValidationException",
            "bad seed",
            "Invalid request parameters: bad seed",
        ),
        (
            403,
            "UnrecognizedClientException",
            "x",
            "AWS authentication failed. Please check your credentials and permissions.",
        ),
        (
            403,
            "AccessDeniedException",
            "x",
            "Access denied. Please ensure you have permissions for Bedrock and S3.",
        ),
        (
            429,
            "ThrottlingException",
            "x",
            "Request was throttled. Please try again later.",
        ),
    ];
    for (status, kind, message, expected) in cases {
        let fake = Fake::new(vec![error_response(status, kind, message)]);
        let e = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &movie("p", 12.0))
            .await
            .unwrap_err();
        assert_eq!(e.to_string(), expected);
    }
    let fake = Fake::new(vec![error_response(500, "InternalServerException", "boom")]);
    let e = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &movie("p", 12.0))
        .await
        .unwrap_err();
    assert_eq!(e.service_name(), Some("InternalServerException"));
    assert_eq!(e.message(), "boom");
}

#[tokio::test]
async fn multi_shot_manual_uploads_images_and_falls_back_to_automated() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.jpeg");
    std::fs::write(&a, b"PNG").unwrap();
    std::fs::write(&b, b"JPEG").unwrap();
    let mut req = movie("base", 12.0);
    req.seed = Some(9.0);
    req.input_images = Some(vec![
        a.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    ]);
    req.prompts = Some(vec!["shot one".into(), "shot two".into()]);

    let fake = Fake::new(vec![
        empty_ok(),
        empty_ok(),
        error_response(400, "ValidationException", "Missing textToVideoParams"),
        json_response(200, json!({ "invocationArn": ARN })),
    ]);
    let out = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &req)
        .await
        .unwrap();
    assert_eq!(out.invocation_arn, ARN);

    let reqs = fake.requests();
    assert_eq!(reqs.len(), 4);
    for (r, file, ctype) in [
        (&reqs[0], "a.png", "image/png"),
        (&reqs[1], "b.jpeg", "image/jpeg"),
    ] {
        assert_eq!(r.method, "PUT");
        assert!(r.uri.contains("my-bucket"), "{}", r.uri);
        assert!(r.uri.contains("/temp-images/"), "{}", r.uri);
        assert!(r.uri.contains(&format!("/{file}?")), "{}", r.uri);
        assert_eq!(r.header("content-type"), Some(ctype));
    }
    let manual = &reqs[2].body["modelInput"];
    assert_eq!(manual["taskType"], "MULTI_SHOT_MANUAL");
    assert_eq!(
        manual["videoGenerationConfig"],
        json!({ "fps": 24, "dimension": "1280x720", "seed": 9 })
    );
    let shots = manual["multiShotManualParams"]["shots"].as_array().unwrap();
    assert_eq!(shots.len(), 2);
    assert_eq!(shots[0]["text"], "shot one");
    assert_eq!(shots[0]["image"]["format"], "png");
    assert_eq!(shots[1]["image"]["format"], "jpeg");
    let uri = shots[1]["image"]["source"]["s3Location"]["uri"]
        .as_str()
        .unwrap();
    assert!(uri.starts_with("s3://my-bucket/temp-images/") && uri.ends_with("/b.jpeg"));

    assert_eq!(
        reqs[3].body["modelInput"],
        json!({
            "taskType": "MULTI_SHOT_AUTOMATED",
            "multiShotAutomatedParams": { "text": "shot one. shot two" },
            "videoGenerationConfig": {
                "fps": 24, "dimension": "1280x720", "seed": 9, "durationSeconds": 12
            }
        })
    );

    // Fallback failure.
    let fake = Fake::new(vec![
        empty_ok(),
        empty_ok(),
        error_response(400, "ValidationException", "Missing textToVideoParams"),
        error_response(400, "ValidationException", "still bad"),
    ]);
    let e = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &req)
        .await
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Both MULTI_SHOT_MANUAL and fallback failed. Original error: Missing textToVideoParams. Fallback error: ValidationException: still bad"
    );

    // Upload failures are wrapped.
    let fake = Fake::new(vec![error_response(403, "AccessDenied", "nope")]);
    let e = video::start_video_generation(fake.as_ref(), &aws("us-east-1"), &req)
        .await
        .unwrap_err();
    assert!(
        e.to_string().starts_with("Failed to upload image to S3: "),
        "{e}"
    );
}

#[tokio::test]
async fn job_status_maps_get_async_invoke() {
    let fake = Fake::new(vec![json_response(200, status_body("Completed"))]);
    let s = video::get_job_status(fake.as_ref(), &aws("us-east-1"), ARN)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({
            "invocationArn": ARN,
            "modelId": "amazon.nova-reel-v1:1",
            "status": "Completed",
            "submitTime": "2025-01-02T03:04:05.678Z",
            "endTime": "2025-01-02T03:10:00.000Z",
            "outputDataConfig": { "s3OutputDataConfig": { "s3Uri": "s3://my-bucket/videos/abc123" } }
        })
    );
    let r = &fake.requests()[0];
    assert_eq!(r.method, "GET");
    assert!(
        r.uri.ends_with(&format!(
            "/async-invoke/{}",
            ARN.replace(':', "%3A").replace('/', "%2F")
        )),
        "{}",
        r.uri
    );

    let fake = Fake::new(vec![json_response(
        200,
        json!({
            "invocationArn": ARN, "modelArn": "m", "status": "Failed",
            "submitTime": "2025-01-02T03:04:05Z", "failureMessage": "content filtered"
        }),
    )]);
    let s = video::get_job_status(fake.as_ref(), &aws("ap-northeast-1"), ARN)
        .await
        .unwrap();
    assert_eq!(s.model_id, "amazon.nova-reel-v1:0");
    assert_eq!(s.failure_message.as_deref(), Some("content filtered"));
    assert!(s.output_data_config.is_none() && s.end_time.is_none());

    let fake = Fake::new(vec![error_response(400, "ValidationException", "bad arn")]);
    let e = video::get_job_status(fake.as_ref(), &aws("us-east-1"), "arn:x")
        .await
        .unwrap_err();
    assert_eq!(e.service_name(), Some("ValidationException"));
}

#[tokio::test]
async fn download_streams_object_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("nested/out.mp4");
    let out_s = out.to_string_lossy().into_owned();
    let fake = Fake::new(vec![bytes_response(b"MP4DATA")]);
    let d = video::download_video(
        fake.as_ref(),
        &aws("us-east-1"),
        "s3://my-bucket/videos/abc123/output.mp4",
        &out_s,
    )
    .await
    .unwrap();
    assert_eq!(d.downloaded_path, out_s);
    assert_eq!(d.file_size, 7);
    assert_eq!(std::fs::read(&out).unwrap(), b"MP4DATA");
    let r = &fake.requests()[0];
    assert_eq!(r.method, "GET");
    assert!(r.uri.contains("my-bucket"), "{}", r.uri);
    assert!(r.uri.contains("videos/abc123/output.mp4"), "{}", r.uri);

    let e = video::download_video(
        fake.as_ref(),
        &aws("us-east-1"),
        "undefined/output.mp4",
        &out_s,
    )
    .await
    .unwrap_err();
    assert_eq!(e.to_string(), "Invalid S3 URI format: undefined/output.mp4");
}

#[tokio::test]
async fn generate_video_polls_then_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("final");
    let fake = Fake::new(vec![
        json_response(200, json!({ "invocationArn": ARN })),
        json_response(200, status_body("InProgress")),
        json_response(200, status_body("Completed")),
        bytes_response(b"VID"),
    ]);
    let mut req = movie("p", 6.0);
    req.output_path = Some(out.to_string_lossy().into_owned());
    let m = video::generate_video_with(
        fake.as_ref(),
        &aws("us-east-1"),
        &req,
        Duration::from_secs(60),
        Duration::from_millis(1),
    )
    .await
    .unwrap();
    let mp4 = format!("{}.mp4", out.to_string_lossy());
    assert_eq!(m.local_path.as_deref(), Some(mp4.as_str()));
    assert_eq!(
        m.output_location.as_deref(),
        Some("s3://my-bucket/videos/abc123")
    );
    assert_eq!(m.status.status, "Completed");
    assert!(m.error.is_none());
    assert_eq!(std::fs::read(&mp4).unwrap(), b"VID");
    // `<prefix>/output.mp4` is fetched, not the prefix itself.
    assert!(
        fake.requests()[3].uri.contains("videos/abc123/output.mp4"),
        "{}",
        fake.requests()[3].uri
    );

    let fake = Fake::new(vec![
        json_response(200, json!({ "invocationArn": ARN })),
        json_response(
            200,
            json!({ "invocationArn": ARN, "modelArn": "m", "status": "Failed",
                    "submitTime": "2025-01-02T03:04:05Z" }),
        ),
    ]);
    let e = video::generate_video_with(
        fake.as_ref(),
        &aws("us-east-1"),
        &movie("p", 6.0),
        Duration::from_secs(60),
        Duration::from_millis(1),
    )
    .await
    .unwrap_err();
    assert_eq!(e.to_string(), "Video generation failed: Unknown error");

    let fake = Fake::new(vec![]);
    let e = video::wait_for_completion(
        fake.as_ref(),
        &aws("us-east-1"),
        ARN,
        Duration::ZERO,
        Duration::from_millis(1),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(e.to_string(), "Video generation timed out after 0 seconds");
}
