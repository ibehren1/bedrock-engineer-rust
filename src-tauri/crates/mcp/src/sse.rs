//! Legacy HTTP+SSE client transport (MCP protocol 2024-11-05), the Rust counterpart of the TS
//! SDK's `SSEClientTransport`. `rmcp` only ships Streamable HTTP, and the TS client falls back to
//! SSE when Streamable HTTP fails, so this is implemented here.
//!
//! Protocol: `GET <url>` opens an event stream whose first `endpoint` event carries the (possibly
//! relative) URL to `POST` JSON-RPC messages to; server messages arrive as `message` events.

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE};
use reqwest::Url;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::service::{RoleClient, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use sse_stream::SseStream;
use std::future::Future;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum SseTransportError {
    #[error("SSE error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("SSE error: {0}")]
    Protocol(String),
    #[error("SSE error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A connected legacy SSE transport.
pub struct SseClientTransport {
    client: reqwest::Client,
    endpoint: Url,
    headers: HeaderMap,
    rx: mpsc::Receiver<ServerJsonRpcMessage>,
    reader: JoinHandle<()>,
}

impl SseClientTransport {
    /// Open the event stream and wait (up to `timeout`) for the `endpoint` event.
    pub async fn connect(
        client: reqwest::Client,
        url: Url,
        headers: HeaderMap,
        timeout: Duration,
    ) -> Result<Self, SseTransportError> {
        let response = client
            .get(url.clone())
            .headers(headers.clone())
            .header(ACCEPT, "text/event-stream")
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(SseTransportError::Protocol(format!(
                "Non-200 status code ({})",
                status.as_u16()
            )));
        }

        let mut events = SseStream::from_bytes_stream(response.bytes_stream());
        let (tx, rx) = mpsc::channel(64);
        let (endpoint_tx, endpoint_rx) = oneshot::channel::<Result<Url, String>>();

        let base = url.clone();
        let reader = tokio::spawn(async move {
            let mut endpoint_tx = Some(endpoint_tx);
            while let Some(event) = events.next().await {
                let event = match event {
                    Ok(e) => e,
                    Err(e) => {
                        if let Some(t) = endpoint_tx.take() {
                            let _ = t.send(Err(e.to_string()));
                        }
                        break;
                    }
                };
                let data = event.data.unwrap_or_default();
                match event.event.as_deref() {
                    Some("endpoint") => {
                        if let Some(t) = endpoint_tx.take() {
                            let _ = t.send(base.join(data.trim()).map_err(|e| e.to_string()));
                        }
                    }
                    None | Some("message") => {
                        match serde_json::from_str::<ServerJsonRpcMessage>(&data) {
                            Ok(msg) => {
                                if tx.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(category = "mcp", error = %e, "Ignoring malformed SSE message")
                            }
                        }
                    }
                    Some(_) => {}
                }
            }
            if let Some(t) = endpoint_tx.take() {
                let _ = t.send(Err("SSE stream closed before the endpoint event".into()));
            }
        });

        let endpoint = match tokio::time::timeout(timeout, endpoint_rx).await {
            Ok(Ok(Ok(endpoint))) => endpoint,
            Ok(Ok(Err(e))) => {
                reader.abort();
                return Err(SseTransportError::Protocol(e));
            }
            Ok(Err(_)) => {
                reader.abort();
                return Err(SseTransportError::Protocol("SSE reader stopped".into()));
            }
            Err(_) => {
                reader.abort();
                return Err(SseTransportError::Protocol(
                    "Timed out waiting for the SSE endpoint event (timeout)".into(),
                ));
            }
        };
        if endpoint.origin() != url.origin() {
            reader.abort();
            return Err(SseTransportError::Protocol(format!(
                "Endpoint origin does not match connection origin: {}",
                endpoint.origin().ascii_serialization()
            )));
        }

        Ok(Self {
            client,
            endpoint,
            headers,
            rx,
            reader,
        })
    }
}

impl Transport<RoleClient> for SseClientTransport {
    type Error = SseTransportError;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let headers = self.headers.clone();
        async move {
            let body = serde_json::to_vec::<ClientJsonRpcMessage>(&item)?;
            let response = client
                .post(endpoint)
                .headers(headers)
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await?;
            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(SseTransportError::Protocol(format!(
                    "Error POSTing to endpoint (HTTP {}): {text}",
                    status.as_u16()
                )));
            }
            Ok(())
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.rx.recv()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.reader.abort();
        self.rx.close();
        std::future::ready(Ok(()))
    }
}

impl Drop for SseClientTransport {
    fn drop(&mut self) {
        self.reader.abort();
    }
}
