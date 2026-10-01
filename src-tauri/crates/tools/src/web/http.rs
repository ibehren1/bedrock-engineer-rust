//! Port of the `fetch-website` IPC handler (`src/main/handlers/util-handlers.ts`), which
//! both web tools use: an axios request with a browser User-Agent, through the configured
//! proxy.

use crate::context::ProxyConfig;
use serde_json::Value;
use std::collections::BTreeMap;

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
pub async fn fetch_website(
    url: &str,
    options: &FetchOptions,
    proxy: Option<&ProxyConfig>,
) -> Result<FetchResponse, String> {
    let mut builder = reqwest::Client::builder();
    if let Some(proxy_url) = proxy.and_then(ProxyConfig::proxy_url) {
        let p = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;
        builder = builder.proxy(p);
    }
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
        if k.eq_ignore_ascii_case("user-agent") {
            continue;
        }
        req = req.header(k, v);
    }
    req = req.header(reqwest::header::USER_AGENT, USER_AGENT);
    if let Some(body) = options.body.as_ref().filter(|b| !b.is_empty()) {
        req = req.body(body.clone());
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
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
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
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
    use super::*;

    #[tokio::test]
    async fn parses_json_and_sets_user_agent() {
        let (base, h) = test_server::serve(vec![
            (200, "application/json", r#"{"a":1}"#.to_string()),
            (200, "text/html", "<p>hi</p>".to_string()),
            (404, "text/plain", "nope".to_string()),
        ])
        .await;
        let r = fetch_website(&format!("{base}/x"), &FetchOptions::default(), None)
            .await
            .unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.data, serde_json::json!({"a": 1}));
        let r = fetch_website(&base, &FetchOptions::default(), None)
            .await
            .unwrap();
        assert_eq!(r.data, Value::String("<p>hi</p>".into()));
        let e = fetch_website(&base, &FetchOptions::default(), None)
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
