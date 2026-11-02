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

/// Largest single SSE event accepted; a bigger one ends the connection (AGENT-M6).
pub const MAX_SSE_EVENT_BYTES: usize = 16 * 1024 * 1024;
/// Most bytes of an error response read into the error message (AGENT-M8).
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
/// Each POST of a JSON-RPC message (AGENT-M7).
const POST_TIMEOUT: Duration = Duration::from_secs(60);

/// Where the last event boundary (`\n\n`, `\r\r` or `\r\n\r\n`) in `chunk` ends.
fn last_event_end(chunk: &[u8]) -> Option<usize> {
    (1..chunk.len()).rev().find_map(|i| {
        let pair = &chunk[i - 1..=i];
        (pair == b"\n\n"
            || pair == b"\r\r"
            || (pair == b"\n\r" && chunk.get(i + 1) == Some(&b'\n')))
        .then(|| if pair == b"\n\r" { i + 2 } else { i + 1 })
    })
}

/// `stream` failing once more than [`MAX_SSE_EVENT_BYTES`] arrive without an event boundary, so
/// a server can't grow one event without limit.
fn cap_events<S, E>(
    stream: S,
) -> std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    Box::pin(stream.scan(0usize, |since_boundary, item| {
        let out = match item {
            Err(e) => Err(std::io::Error::other(e)),
            Ok(chunk) => {
                match last_event_end(&chunk) {
                    Some(end) => *since_boundary = chunk.len() - end,
                    None => *since_boundary += chunk.len(),
                }
                if *since_boundary > MAX_SSE_EVENT_BYTES {
                    Err(std::io::Error::other(format!(
                        "SSE event larger than {} MB",
                        MAX_SSE_EVENT_BYTES / (1024 * 1024)
                    )))
                } else {
                    Ok(chunk)
                }
            }
        };
        futures_util::future::ready(Some(out))
    }))
}

/// Up to [`MAX_ERROR_BODY_BYTES`] of `response`'s body, as text.
async fn error_body(mut response: reqwest::Response) -> String {
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        let room = MAX_ERROR_BODY_BYTES - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() >= MAX_ERROR_BODY_BYTES {
            break;
        }
    }
    String::from_utf8_lossy(&body).into_owned()
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
        // The headers count against the same timeout as the endpoint event (AGENT-M7).
        let response = tokio::time::timeout(
            timeout,
            client
                .get(url.clone())
                .headers(headers.clone())
                .header(ACCEPT, "text/event-stream")
                .send(),
        )
        .await
        .map_err(|_| {
            SseTransportError::Protocol("Timed out connecting to the SSE endpoint".into())
        })??;
        let status = response.status();
        if !status.is_success() {
            return Err(SseTransportError::Protocol(format!(
                "Non-200 status code ({})",
                status.as_u16()
            )));
        }

        let mut events = SseStream::from_bytes_stream(cap_events(response.bytes_stream()));
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
            let response = tokio::time::timeout(
                POST_TIMEOUT,
                client
                    .post(endpoint)
                    .headers(headers)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body)
                    .send(),
            )
            .await
            .map_err(|_| {
                SseTransportError::Protocol("Timed out posting to the SSE endpoint".into())
            })??;
            let status = response.status();
            if !status.is_success() {
                let text = error_body(response).await;
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

#[cfg(test)]
mod cap_tests {
    use super::*;

    #[test]
    fn event_boundaries_are_found() {
        assert_eq!(last_event_end(b"data: a\n\ndata: b"), Some(9));
        assert_eq!(last_event_end(b"data: a\r\n\r\nx"), Some(11));
        assert_eq!(last_event_end(b"data: a\n"), None);
    }

    // AGENT-M6: an event that never ends fails the stream instead of growing memory.
    #[tokio::test]
    async fn oversized_events_end_the_stream() {
        let chunk = bytes::Bytes::from(vec![b'x'; 1024 * 1024]);
        let chunks = (0..20).map(move |_| Ok::<_, std::io::Error>(chunk.clone()));
        let mut capped = cap_events(futures_util::stream::iter(chunks));
        let mut ok = 0;
        let mut failed = false;
        while let Some(item) = capped.next().await {
            match item {
                Ok(_) => ok += 1,
                Err(e) => {
                    assert!(e.to_string().contains("larger than"), "{e}");
                    failed = true;
                    break;
                }
            }
        }
        assert!(failed);
        assert_eq!(ok, MAX_SSE_EVENT_BYTES / (1024 * 1024));
    }
}
