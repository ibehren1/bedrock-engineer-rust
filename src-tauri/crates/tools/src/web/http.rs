//! Port of the `fetch-website` IPC handler (`src/main/handlers/util-handlers.ts`), which
//! both web tools use: an axios request with a browser User-Agent, through the configured
//! proxy.

use crate::context::ProxyConfig;
use serde_json::Value;
use std::collections::BTreeMap;

/// Whole request, connect included; a hostile server could otherwise hold it open forever.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// Largest response body read; the rest would only be cut for the model anyway (TOOLS-5).
pub const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;
/// Request headers the model may not set: the IMDSv2 session token pair.
const DROPPED_HEADERS: &[&str] = &[
    "x-aws-ec2-metadata-token",
    "x-aws-ec2-metadata-token-ttl-seconds",
];

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";

/// Request options (`RequestInit` subset forwarded by the tools).
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    pub method: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}

/// `{ status, headers, data }`. `data` follows axios' default response transform: the
/// body parsed as JSON when it parses, else the raw string.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub data: Value,
}

/// Perform the request. Non-2xx statuses are errors, as with axios'
/// `validateStatus`: `Request failed with status code N`.
///
/// Destinations are checked by [`super::net_guard`]; `block_local` is the user's **Block local
/// network access** setting.
pub async fn fetch_website(
    url: &str,
    options: &FetchOptions,
    proxy: Option<&ProxyConfig>,
    block_local: bool,
) -> Result<FetchResponse, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    if let Some(problem) = super::net_guard::url_problem(&parsed, block_local) {
        return Err(problem);
    }
    // A proxy that is on but unusable fails the request; nothing connects directly (AGENT-B5).
    if let Some(reason) = super::proxy::broken_reason(proxy) {
        return Err(reason);
    }
    let proxied = matches!(
        proxy.map(ProxyConfig::setting),
        Some(super::proxy::ProxySetting::On(_))
    );
    let builder = super::proxy::proxied(
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT),
        proxy,
    );
    let builder = super::net_guard::guard(builder, block_local, proxied);
    let client = builder.build().map_err(|e| e.to_string())?;
    let method = options
        .method
        .as_deref()
        .filter(|m| !m.is_empty())
        .unwrap_or("GET")
        .to_uppercase();
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut req = client.request(method, url);
    for (k, v) in &options.headers {
        if k.eq_ignore_ascii_case("user-agent")
            || DROPPED_HEADERS.iter().any(|d| k.eq_ignore_ascii_case(d))
        {
            continue;
        }
        req = req.header(k, v);
    }
    req = req.header(reqwest::header::USER_AGENT, USER_AGENT);
    if let Some(body) = options.body.as_ref().filter(|b| !b.is_empty()) {
        req = req.body(body.clone());
    }
    let mut resp = req.send().await.map_err(|e| {
        // Name the refusal rather than reqwest's generic wrapper.
        let mut msg = e.to_string();
        let mut source = std::error::Error::source(&e);
        while let Some(inner) = source {
            msg = inner.to_string();
            source = inner.source();
        }
        msg
    })?;
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                v.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let mut bytes: Vec<u8> = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        let room = MAX_RESPONSE_BYTES - bytes.len();
        if chunk.len() > room {
            bytes.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    if truncated {
        tracing::warn!(
            url,
            limit = MAX_RESPONSE_BYTES,
            "Response truncated at the size limit"
        );
    }
    if !(200..300).contains(&status) {
        return Err(format!("Request failed with status code {status}"));
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let data = if text.is_empty() {
        Value::String(text)
    } else {
        serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text))
    };
    Ok(FetchResponse {
        status,
        headers,
        data,
    })
}

#[cfg(test)]
pub(crate) mod test_server {
    //! Minimal one-shot HTTP/1.1 server for web tool tests.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve `responses` (status, content-type, body) in order, one per connection;
    /// returns the base URL and a handle yielding the raw requests received.
    pub async fn serve(
        responses: Vec<(u16, &'static str, String)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, ctype, body) in responses {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let n = sock.read(&mut tmp).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    let s = String::from_utf8_lossy(&buf);
                    if let Some(idx) = s.find("\r\n\r\n") {
                        let len = s[..idx]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= idx + 4 + len {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&buf).into_owned());
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                sock.write_all(resp.as_bytes()).await.unwrap();
                let _ = sock.shutdown().await;
            }
            requests
        });
        (format!("http://{addr}"), handle)
    }
}

#[cfg(test)]
mod tests {

    // TOOLS-4 / TOOLS-5: metadata always refused (also via redirect), local refused with the
    // switch, oversized bodies cut.
    #[tokio::test]
    async fn destinations_are_checked_and_bodies_capped() {
        let e = fetch_website(
            "http://169.254.169.254/latest/meta-data/",
            &FetchOptions::default(),
            None,
            false,
        )
        .await
        .unwrap_err();
        assert!(e.contains("may not reach"), "{e}");

        let (base, _h) = super::test_server::serve(vec![(200, "text/plain", "hi".into())]).await;
        let e = fetch_website(&base, &FetchOptions::default(), None, true)
            .await
            .unwrap_err();
        assert!(e.contains("may not reach"), "{e}");
        let r = fetch_website(&base, &FetchOptions::default(), None, false)
            .await
            .unwrap();
        assert_eq!(r.data, serde_json::json!("hi"));

        let big = "x".repeat(MAX_RESPONSE_BYTES + 1000);
        let (base, _h) = super::test_server::serve(vec![(200, "text/plain", big)]).await;
        let r = fetch_website(&base, &FetchOptions::default(), None, false)
            .await
            .unwrap();
        assert_eq!(r.data.as_str().unwrap().len(), MAX_RESPONSE_BYTES);
    }
    use super::*;

    #[tokio::test]
    async fn parses_json_and_sets_user_agent() {
        let (base, h) = test_server::serve(vec![
            (200, "application/json", r#"{"a":1}"#.to_string()),
            (200, "text/html", "<p>hi</p>".to_string()),
            (404, "text/plain", "nope".to_string()),
        ])
        .await;
        let r = fetch_website(&format!("{base}/x"), &FetchOptions::default(), None, false)
            .await
            .unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.data, serde_json::json!({"a": 1}));
        let r = fetch_website(&base, &FetchOptions::default(), None, false)
            .await
            .unwrap();
        assert_eq!(r.data, Value::String("<p>hi</p>".into()));
        let e = fetch_website(&base, &FetchOptions::default(), None, false)
            .await
            .unwrap_err();
        assert_eq!(e, "Request failed with status code 404");
        let reqs = h.await.unwrap();
        assert!(reqs[0].starts_with("GET /x "));
        assert!(reqs[0]
            .to_ascii_lowercase()
            .contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase())));
    }
}
