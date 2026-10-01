//! Bedrock tools (`src/preload/tools/handlers/bedrock`): generateImage, generateVideo,
//! checkVideoStatus, downloadVideo, recognizeImage, retrieve, invokeBedrockAgent, invokeFlow.
//!
//! The TS tools called the `bedrock:*` IPC handlers; here they call a [`BedrockBackend`],
//! whose production implementation [`SdkBedrockBackend`] wraps the `bedrock` crate services
//! (with the IPC handlers' parameter translation). Settings come from
//! [`crate::ToolSettings::converse`] (the store's `aws` etc.).
//!
//! Error messages from the backend are the service error's `message`. Under Electron the
//! IPC hop prefixed them with `Error invoking remote method '<channel>': Error: `; that
//! transport artifact is not reproduced.

mod check_video_status;
mod download_video;
mod generate_image;
mod generate_video;
mod invoke_bedrock_agent;
mod invoke_flow;
mod recognize_image;
mod retrieve;

pub use check_video_status::CheckVideoStatusTool;
pub use download_video::DownloadVideoTool;
pub use generate_image::GenerateImageTool;
pub use generate_video::GenerateVideoTool;
pub use invoke_bedrock_agent::{mime_type_for, InvokeBedrockAgentTool};
pub use invoke_flow::InvokeFlowTool;
pub use recognize_image::RecognizeImageTool;
pub use retrieve::RetrieveTool;

use crate::base::Tool;
use crate::error::ToolError;
use async_trait::async_trait;
use bedrock::agent::InvokeAgentResult;
use bedrock::image::{GenerateImageRequest, GeneratedImage};
use bedrock::image_recognition::RecognizeImageRequest;
use bedrock::video::{
    AsyncInvocationStatus, DownloadedVideo, GenerateMovieRequest, GeneratedMovie,
};
use bedrock::{ConverseService, ConverseSettings, SdkConfigClients, SdkConfigSource};
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Backend errors are `error.message`.
pub type BackendResult<T> = std::result::Result<T, String>;

/// The `bedrock:*` IPC handlers the Bedrock tools call.
#[async_trait]
pub trait BedrockBackend: Send + Sync {
    /// `bedrock:generateImage`.
    async fn generate_image(
        &self,
        settings: &ConverseSettings,
        req: &GenerateImageRequest,
    ) -> BackendResult<GeneratedImage>;

    /// `bedrock:recognizeImage` (one image per call).
    async fn recognize_image(
        &self,
        settings: &ConverseSettings,
        req: &RecognizeImageRequest,
    ) -> BackendResult<String>;

    /// `bedrock:retrieve` with its IPC params `{ knowledgeBaseId, query, retrievalConfiguration? }`.
    async fn retrieve(&self, settings: &ConverseSettings, params: &Value) -> BackendResult<Value>;

    /// `bedrock:invokeAgent` with `InvokeAgentCommandInput` JSON (file bytes base64).
    async fn invoke_agent(
        &self,
        settings: &ConverseSettings,
        params: &Value,
    ) -> BackendResult<InvokeAgentResult>;

    /// `bedrock:invokeFlow` with its IPC params `{ flowIdentifier, flowAliasIdentifier, inputs }`.
    async fn invoke_flow(
        &self,
        settings: &ConverseSettings,
        params: &Value,
    ) -> BackendResult<Value>;

    /// `bedrock:startVideoGeneration`.
    async fn start_video_generation(
        &self,
        settings: &ConverseSettings,
        req: &GenerateMovieRequest,
    ) -> BackendResult<GeneratedMovie>;

    /// `bedrock:checkVideoStatus` (`{ invocationArn }`).
    async fn check_video_status(
        &self,
        settings: &ConverseSettings,
        invocation_arn: &str,
    ) -> BackendResult<AsyncInvocationStatus>;

    /// `bedrock:downloadVideo` (`{ s3Uri, localPath }`).
    async fn download_video(
        &self,
        settings: &ConverseSettings,
        s3_uri: &str,
        local_path: &str,
    ) -> BackendResult<DownloadedVideo>;
}

/// [`BedrockBackend`] over the `bedrock` crate.
#[derive(Clone)]
pub struct SdkBedrockBackend {
    configs: Arc<dyn SdkConfigSource>,
    converse: Arc<ConverseService>,
}

impl Default for SdkBedrockBackend {
    fn default() -> Self {
        Self::new(Arc::new(bedrock::DefaultSdkConfig))
    }
}

impl SdkBedrockBackend {
    /// Every client comes from `configs`; the Converse path (Nova image recognition) too.
    pub fn new(configs: Arc<dyn SdkConfigSource>) -> Self {
        let converse = ConverseService::default()
            .with_client_factory(Arc::new(SdkConfigClients(configs.clone())));
        Self {
            configs,
            converse: Arc::new(converse),
        }
    }

    /// Use an existing `ConverseService` (shares its retry policy / model info).
    pub fn with_converse(
        configs: Arc<dyn SdkConfigSource>,
        converse: Arc<ConverseService>,
    ) -> Self {
        Self { configs, converse }
    }
}

fn msg(e: bedrock::Error) -> String {
    e.message()
}

#[async_trait]
impl BedrockBackend for SdkBedrockBackend {
    async fn generate_image(
        &self,
        settings: &ConverseSettings,
        req: &GenerateImageRequest,
    ) -> BackendResult<GeneratedImage> {
        bedrock::image::generate_image(self.configs.as_ref(), &settings.aws, req)
            .await
            .map_err(msg)
    }

    async fn recognize_image(
        &self,
        settings: &ConverseSettings,
        req: &RecognizeImageRequest,
    ) -> BackendResult<String> {
        bedrock::image_recognition::recognize_image(
            self.configs.as_ref(),
            &self.converse,
            settings,
            req,
        )
        .await
        .map_err(msg)
    }

    async fn retrieve(&self, settings: &ConverseSettings, params: &Value) -> BackendResult<Value> {
        let input = bedrock::agent::retrieve_input_from_ipc(params);
        bedrock::agent::retrieve(self.configs.as_ref(), &settings.aws, &input)
            .await
            .map_err(msg)
    }

    async fn invoke_agent(
        &self,
        settings: &ConverseSettings,
        params: &Value,
    ) -> BackendResult<InvokeAgentResult> {
        bedrock::agent::invoke_agent(self.configs.as_ref(), &settings.aws, params)
            .await
            .map_err(msg)
    }

    async fn invoke_flow(
        &self,
        settings: &ConverseSettings,
        params: &Value,
    ) -> BackendResult<Value> {
        let input = bedrock::flow::invoke_flow_input_from_ipc(params);
        bedrock::flow::invoke_flow(self.configs.as_ref(), &settings.aws, &input)
            .await
            .map_err(msg)
    }

    async fn start_video_generation(
        &self,
        settings: &ConverseSettings,
        req: &GenerateMovieRequest,
    ) -> BackendResult<GeneratedMovie> {
        bedrock::video::start_video_generation(self.configs.as_ref(), &settings.aws, req)
            .await
            .map_err(msg)
    }

    async fn check_video_status(
        &self,
        settings: &ConverseSettings,
        invocation_arn: &str,
    ) -> BackendResult<AsyncInvocationStatus> {
        bedrock::video::get_job_status(self.configs.as_ref(), &settings.aws, invocation_arn)
            .await
            .map_err(msg)
    }

    async fn download_video(
        &self,
        settings: &ConverseSettings,
        s3_uri: &str,
        local_path: &str,
    ) -> BackendResult<DownloadedVideo> {
        bedrock::video::download_video(self.configs.as_ref(), &settings.aws, s3_uri, local_path)
            .await
            .map_err(msg)
    }
}

/// `createBedrockTools()`, in the TS order.
pub fn create_bedrock_tools(backend: Arc<dyn BedrockBackend>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(GenerateImageTool::new(backend.clone())),
        Arc::new(GenerateVideoTool::new(backend.clone())),
        Arc::new(CheckVideoStatusTool::new(backend.clone())),
        Arc::new(DownloadVideoTool::new(backend.clone())),
        Arc::new(RecognizeImageTool::new(backend.clone())),
        Arc::new(RetrieveTool::new(backend.clone())),
        Arc::new(InvokeBedrockAgentTool::new(backend.clone())),
        Arc::new(InvokeFlowTool::new(backend)),
    ]
}

// ---------------------------------------------------------------------------------------------
// JS-semantics helpers shared by the Bedrock tools
// ---------------------------------------------------------------------------------------------

/// `input.key !== undefined` (JSON `null` counts as present).
pub(crate) fn present(input: &Value, key: &str) -> bool {
    input.get(key).is_some()
}

/// `typeof v === 'string'`.
pub(crate) fn is_string(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::String(_)))
}

/// `this.truncateForLogging(str, max)` (UTF-16 units).
pub(crate) fn truncate_for_logging(s: &str, max: usize) -> String {
    if crate::util::js::len(s) <= max {
        s.to_string()
    } else {
        format!("{}...", crate::util::js::slice(s, 0, max))
    }
}

/// `throw \`${prefix}: ${JSON.stringify({ success: false, name, error, message })}\``: a thrown
/// string, which `wrapError` turns into an `ExecutionError` with `originalError`.
pub(crate) fn thrown_json(prefix: &str, name: &str, error: &str, message: &str) -> ToolError {
    let body = json!({ "success": false, "name": name, "error": error, "message": message });
    thrown_string(format!("{prefix}: {body}"), name)
}

/// `wrapError(someString, toolName)`.
pub(crate) fn thrown_string(s: String, tool_name: &str) -> ToolError {
    let mut extra = Map::new();
    extra.insert("originalError".into(), json!(s));
    ToolError::execution(s, tool_name, None, Some(extra))
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::sync::Mutex;

    pub type RecognizeFn = Arc<dyn Fn(&str) -> BackendResult<String> + Send + Sync>;

    /// Records every call and answers from the configured closures' results.
    #[derive(Default)]
    pub struct FakeBedrock {
        pub calls: Mutex<Vec<(String, Value)>>,
        pub image: Mutex<Option<BackendResult<GeneratedImage>>>,
        pub recognize: Mutex<Option<RecognizeFn>>,
        pub retrieve: Mutex<Option<BackendResult<Value>>>,
        pub agent: Mutex<Option<BackendResult<InvokeAgentResult>>>,
        pub flow: Mutex<Option<BackendResult<Value>>>,
        pub start_video: Mutex<Option<BackendResult<GeneratedMovie>>>,
        /// Answered in order; the last one repeats.
        pub video_status: Mutex<Vec<BackendResult<AsyncInvocationStatus>>>,
        pub download: Mutex<Option<BackendResult<DownloadedVideo>>>,
    }

    impl FakeBedrock {
        pub fn record(&self, name: &str, v: Value) {
            self.calls.lock().unwrap().push((name.to_string(), v));
        }
        pub fn calls(&self) -> Vec<(String, Value)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl BedrockBackend for FakeBedrock {
        async fn generate_image(
            &self,
            settings: &ConverseSettings,
            req: &GenerateImageRequest,
        ) -> BackendResult<GeneratedImage> {
            self.record(
                "generateImage",
                json!({ "region": settings.aws.region, "req": req }),
            );
            self.image.lock().unwrap().clone().expect("image result")
        }
        async fn recognize_image(
            &self,
            _settings: &ConverseSettings,
            req: &RecognizeImageRequest,
        ) -> BackendResult<String> {
            self.record("recognizeImage", serde_json::to_value(req).unwrap());
            let f = self.recognize.lock().unwrap().clone().expect("recognize");
            f(&req.image_path)
        }
        async fn retrieve(&self, _s: &ConverseSettings, params: &Value) -> BackendResult<Value> {
            self.record("retrieve", params.clone());
            self.retrieve
                .lock()
                .unwrap()
                .clone()
                .expect("retrieve result")
        }
        async fn invoke_agent(
            &self,
            _s: &ConverseSettings,
            params: &Value,
        ) -> BackendResult<InvokeAgentResult> {
            self.record("invokeAgent", params.clone());
            self.agent.lock().unwrap().clone().expect("agent result")
        }
        async fn invoke_flow(&self, _s: &ConverseSettings, params: &Value) -> BackendResult<Value> {
            self.record("invokeFlow", params.clone());
            self.flow.lock().unwrap().clone().expect("flow result")
        }
        async fn start_video_generation(
            &self,
            _s: &ConverseSettings,
            req: &GenerateMovieRequest,
        ) -> BackendResult<GeneratedMovie> {
            self.record("startVideoGeneration", serde_json::to_value(req).unwrap());
            self.start_video
                .lock()
                .unwrap()
                .clone()
                .expect("start video result")
        }
        async fn check_video_status(
            &self,
            _s: &ConverseSettings,
            invocation_arn: &str,
        ) -> BackendResult<AsyncInvocationStatus> {
            self.record(
                "checkVideoStatus",
                json!({ "invocationArn": invocation_arn }),
            );
            let mut all = self.video_status.lock().unwrap();
            if all.len() > 1 {
                all.remove(0)
            } else {
                all.first().cloned().expect("video status result")
            }
        }
        async fn download_video(
            &self,
            _s: &ConverseSettings,
            s3_uri: &str,
            local_path: &str,
        ) -> BackendResult<DownloadedVideo> {
            self.record(
                "downloadVideo",
                json!({ "s3Uri": s3_uri, "localPath": local_path }),
            );
            self.download
                .lock()
                .unwrap()
                .clone()
                .expect("download result")
        }
    }

    /// The error response JSON inside a `shouldReturnErrorAsString` error.
    pub fn response(err: &ToolError) -> Value {
        assert_eq!(err.name, "Error");
        serde_json::from_str(&err.message).expect("error response JSON")
    }
}
