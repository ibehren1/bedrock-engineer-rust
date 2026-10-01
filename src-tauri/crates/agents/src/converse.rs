//! The model call behind agent runs.
//!
//! [`ConverseBackend`] is the seam tests use to script model responses; the app uses
//! [`BedrockConverseBackend`], which is `BedrockService.converse` (non-streaming Converse with the
//! store's AWS / inference / thinking settings).

use async_trait::async_trait;
use bedrock::{CancellationToken, ConverseRequest, ConverseService, ConverseSettings};
use serde_json::Value;
use std::sync::Arc;

/// One non-streaming Converse call. Returns the `POST /converse` response body
/// (`output.message`, `stopReason`, `usage`, ...); errors are messages.
#[async_trait]
pub trait ConverseBackend: Send + Sync {
    async fn converse(&self, request: ConverseRequest) -> Result<Value, String>;
}

/// Settings for each call, read from the store at call time.
pub type SettingsSource = Arc<dyn Fn() -> ConverseSettings + Send + Sync>;

/// [`ConverseBackend`] over [`bedrock::ConverseService::converse`].
pub struct BedrockConverseBackend {
    service: Arc<ConverseService>,
    settings: SettingsSource,
}

impl BedrockConverseBackend {
    pub fn new(service: Arc<ConverseService>, settings: SettingsSource) -> Self {
        BedrockConverseBackend { service, settings }
    }
}

#[async_trait]
impl ConverseBackend for BedrockConverseBackend {
    async fn converse(&self, request: ConverseRequest) -> Result<Value, String> {
        let settings = (self.settings)();
        self.service
            .converse(&settings, &request, &CancellationToken::new())
            .await
            .map_err(|e| e.to_string())
    }
}
