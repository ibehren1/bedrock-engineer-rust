//! `ConverseService` — port of `src/main/api/bedrock/services/converseService.ts` together with the
//! Express `/converse` and `/converse/stream` routes that fronted it.

use crate::client::runtime_client;
use crate::convert::{guardrail_from_json, guardrail_stream_from_json};
use crate::error::{Error, Result};
use crate::model_info::{ModelInfo, RegistryModelInfo};
use crate::request::{prepare_request, ConverseRequest, PreparedRequest, SdkRequestParts};
use crate::retry::{random_index, with_retries, FailoverConfig, RetryPolicy};
use crate::settings::{AwsSettings, ConverseSettings};
use crate::stream_event::{converse_output_to_json, stream_event_to_json};
use aws_sdk_bedrockruntime::operation::RequestId;
use aws_sdk_bedrockruntime::types::ConverseStreamOutput;
use aws_sdk_bedrockruntime::Client;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Boxed future returned by a [`ClientFactory`].
pub type ClientFuture<'a> = Pin<Box<dyn Future<Output = Result<Client>> + Send + 'a>>;

/// Builds a Bedrock Runtime client for (possibly region-overridden) settings. The default builds
/// a fresh client per request like the TS `createRuntimeClient`; the app may cache, and tests
/// inject a fake HTTP client.
pub trait ClientFactory: Send + Sync {
    fn client<'a>(&'a self, aws: &'a AwsSettings) -> ClientFuture<'a>;
}

/// [`ClientFactory`] using [`runtime_client`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultClientFactory;

impl ClientFactory for DefaultClientFactory {
    fn client<'a>(&'a self, aws: &'a AwsSettings) -> ClientFuture<'a> {
        Box::pin(runtime_client(aws))
    }
}

/// Source of stream events; implemented for the SDK receiver and for test fakes.
pub trait EventSource {
    fn next_event(&mut self) -> impl Future<Output = Result<Option<ConverseStreamOutput>>> + Send;
}

impl EventSource
    for aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver<
        ConverseStreamOutput,
        aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError,
    >
{
    async fn next_event(&mut self) -> Result<Option<ConverseStreamOutput>> {
        self.recv().await.map_err(Error::from)
    }
}

/// Forward every event from `source` to `on_event` as Express-identical JSON until the stream
/// ends, errors, the sink refuses an event, or `cancel` fires.
pub async fn pump_events<S, F>(
    mut source: S,
    cancel: &CancellationToken,
    mut on_event: F,
) -> Result<()>
where
    S: EventSource,
    F: FnMut(Value) -> std::result::Result<(), String>,
{
    loop {
        let next = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            next = source.next_event() => next?,
        };
        match next {
            Some(event) => {
                if let Some(json) = stream_event_to_json(&event) {
                    on_event(json).map_err(Error::Sink)?;
                } else {
                    tracing::debug!("Skipping unknown ConverseStream event");
                }
            }
            None => return Ok(()),
        }
    }
}

/// Converse / ConverseStream with the Electron app's request shaping, retry and failover.
pub struct ConverseService {
    info: Arc<dyn ModelInfo>,
    clients: Arc<dyn ClientFactory>,
    retry: RetryPolicy,
}

impl Default for ConverseService {
    /// Backed by the `models` registry ([`RegistryModelInfo`]).
    fn default() -> Self {
        Self::new(Arc::new(RegistryModelInfo))
    }
}

impl ConverseService {
    /// `info` supplies model metadata (see [`crate::model_info`]); [`ConverseService::default`]
    /// uses the registry.
    pub fn new(info: Arc<dyn ModelInfo>) -> Self {
        Self {
            info,
            clients: Arc::new(DefaultClientFactory),
            retry: RetryPolicy::default(),
        }
    }

    /// Replace the client factory (client caching in the app, fake HTTP in tests).
    pub fn with_client_factory(mut self, clients: Arc<dyn ClientFactory>) -> Self {
        self.clients = clients;
        self
    }

    /// Replace the throttling retry policy (default: 30 retries, 5 s apart).
    pub fn with_retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// The request that would be sent, in JSON form (the TS `commandParams`).
    pub fn prepare(&self, settings: &ConverseSettings, req: &ConverseRequest) -> PreparedRequest {
        prepare_request(req, settings, self.info.as_ref())
    }

    fn failover(&self, settings: &ConverseSettings, model_id: &str) -> FailoverConfig {
        let bedrock = settings.bedrock_settings.clone().unwrap_or_default();
        FailoverConfig {
            enabled: bedrock.enable_region_failover,
            current_region: settings.aws.region.clone(),
            configured_regions: bedrock.available_failover_regions,
            model_regions: self.info.regions(model_id),
        }
    }

    fn aws_for(settings: &ConverseSettings, region: Option<String>) -> AwsSettings {
        match region {
            Some(r) => settings.aws.with_region(r),
            None => settings.aws.clone(),
        }
    }

    /// Non-streaming Converse. Returns the `POST /converse` response body JSON.
    pub async fn converse(
        &self,
        settings: &ConverseSettings,
        req: &ConverseRequest,
        cancel: &CancellationToken,
    ) -> Result<Value> {
        let parts = Arc::new(self.prepare(settings, req).to_sdk()?);
        let failover = self.failover(settings, &req.model_id);
        with_retries(self.retry, &failover, cancel, random_index, |region| {
            let aws = Self::aws_for(settings, region);
            let parts = parts.clone();
            let clients = self.clients.clone();
            async move {
                let client = clients.client(&aws).await?;
                send_converse(&client, &parts).await
            }
        })
        .await
    }

    /// Streaming Converse. Each event is passed to `on_event` as the exact JSON object the Express
    /// `/converse/stream` route wrote per line (see [`crate::stream_event`]). Returns `Ok(())` when
    /// the stream ends normally.
    ///
    /// Errors before the first event (including after exhausting throttling retries) and errors
    /// raised inside the stream are both returned as `Err`. `Err(Error::Cancelled)` if `cancel`
    /// fires; `Err(Error::Sink)` if `on_event` returns an error (the stream is dropped, closing the
    /// HTTP connection).
    pub async fn converse_stream<F>(
        &self,
        settings: &ConverseSettings,
        req: &ConverseRequest,
        cancel: &CancellationToken,
        on_event: F,
    ) -> Result<()>
    where
        F: FnMut(Value) -> std::result::Result<(), String>,
    {
        let parts = Arc::new(self.prepare(settings, req).to_sdk()?);
        tracing::debug!(
            model_id = %req.model_id,
            region = %settings.aws.region,
            message_count = req.messages.len(),
            "Sending stream converse request"
        );
        let failover = self.failover(settings, &req.model_id);
        let receiver = with_retries(self.retry, &failover, cancel, random_index, |region| {
            let aws = Self::aws_for(settings, region);
            let parts = parts.clone();
            let clients = self.clients.clone();
            async move {
                let client = clients.client(&aws).await?;
                let builder = client
                    .converse_stream()
                    .model_id(parts.model_id.clone())
                    .set_messages(Some(parts.messages.clone()))
                    .set_system(parts.system.clone())
                    .set_tool_config(parts.tool_config.clone())
                    .inference_config(parts.inference_config.clone())
                    .set_additional_model_request_fields(
                        parts.additional_model_request_fields.clone(),
                    )
                    .set_guardrail_config(
                        parts
                            .guardrail_config
                            .as_ref()
                            .map(guardrail_stream_from_json)
                            .transpose()?,
                    );
                let out = builder.send().await.map_err(Error::from)?;
                Ok(out.stream)
            }
        })
        .await?;
        pump_events(receiver, cancel, on_event).await
    }
}

async fn send_converse(client: &Client, parts: &SdkRequestParts) -> Result<Value> {
    let out = client
        .converse()
        .model_id(parts.model_id.clone())
        .set_messages(Some(parts.messages.clone()))
        .set_system(parts.system.clone())
        .set_tool_config(parts.tool_config.clone())
        .inference_config(parts.inference_config.clone())
        .set_additional_model_request_fields(parts.additional_model_request_fields.clone())
        .set_guardrail_config(
            parts
                .guardrail_config
                .as_ref()
                .map(guardrail_from_json)
                .transpose()?,
        )
        .send()
        .await
        .map_err(Error::from)?;
    Ok(converse_output_to_json(&out, out.request_id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{ConversationRole, MessageStartEvent};
    use std::collections::VecDeque;
    use std::time::Duration;

    /// Yields scripted events, then either ends or hangs forever.
    struct Scripted {
        events: VecDeque<ConverseStreamOutput>,
        hang: bool,
    }

    impl EventSource for Scripted {
        async fn next_event(&mut self) -> Result<Option<ConverseStreamOutput>> {
            match self.events.pop_front() {
                Some(e) => Ok(Some(e)),
                None if self.hang => std::future::pending().await,
                None => Ok(None),
            }
        }
    }

    fn start() -> ConverseStreamOutput {
        ConverseStreamOutput::MessageStart(
            MessageStartEvent::builder()
                .role(ConversationRole::Assistant)
                .build()
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn pump_forwards_until_end() {
        let mut got = Vec::new();
        pump_events(
            Scripted {
                events: vec![start(), start()].into(),
                hang: false,
            },
            &CancellationToken::new(),
            |v| {
                got.push(v);
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0],
            serde_json::json!({ "messageStart": { "role": "assistant" } })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pump_cancel_mid_stream() {
        let token = CancellationToken::new();
        let t2 = token.clone();
        let handle = tokio::spawn(async move {
            let mut n = 0;
            let r = pump_events(
                Scripted {
                    events: vec![start()].into(),
                    hang: true,
                },
                &t2,
                |_| {
                    n += 1;
                    Ok(())
                },
            )
            .await;
            (r, n)
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        token.cancel();
        let (r, n) = handle.await.unwrap();
        assert!(matches!(r, Err(Error::Cancelled)));
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn pump_sink_error() {
        let r = pump_events(
            Scripted {
                events: vec![start()].into(),
                hang: false,
            },
            &CancellationToken::new(),
            |_| Err("gone".to_string()),
        )
        .await;
        assert!(matches!(r, Err(Error::Sink(ref m)) if m == "gone"));
    }

    #[test]
    fn failover_config_from_settings() {
        struct Regions;
        impl ModelInfo for Regions {
            fn supports_thinking(&self, _: &str) -> bool {
                false
            }
            fn supported_thinking_types(&self, _: &str) -> Vec<crate::ThinkingType> {
                vec![]
            }
            fn max_tokens_limit(&self, _: &str) -> Option<i64> {
                None
            }
            fn regions(&self, _: &str) -> Option<Vec<String>> {
                Some(vec!["us-east-1".into()])
            }
            fn cache_config(&self, _: &str) -> Option<crate::CacheConfig> {
                None
            }
        }
        let svc = ConverseService::new(Arc::new(Regions));
        let mut s = ConverseSettings::default();
        s.aws.region = "us-west-2".into();
        s.bedrock_settings = Some(crate::BedrockSettings {
            enable_region_failover: true,
            available_failover_regions: vec!["us-east-1".into()],
        });
        let f = svc.failover(&s, "m");
        assert!(f.enabled);
        assert_eq!(f.current_region, "us-west-2");
        assert_eq!(f.model_regions, Some(vec!["us-east-1".to_string()]));
        assert_eq!(f.configured_regions, vec!["us-east-1".to_string()]);
        // no bedrockSettings -> failover off
        assert!(!svc.failover(&ConverseSettings::default(), "m").enabled);
    }
}
