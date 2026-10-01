//! Connection tests against `tests/fake_mcp_server.py` (stdio, Streamable HTTP, legacy SSE).
//! Skipped (with a note) when `python3` is not available.

use common::agent::{ConnectionType, McpServerConfig};
use indexmap::IndexMap;
use mcp::{McpClient, McpManager};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn script() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_mcp_server.py")
        .to_string_lossy()
        .into_owned()
}

fn python() -> Option<String> {
    for candidate in ["python3", "python"] {
        let ok = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(candidate.to_string());
        }
    }
    eprintln!("python3 not found; skipping MCP fake-server test");
    None
}

fn stdio_server(py: &str) -> McpServerConfig {
    McpServerConfig {
        name: "fake".into(),
        description: "fake server".into(),
        connection_type: Some(ConnectionType::Command),
        command: Some(py.into()),
        args: Some(vec![script()]),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_lists_and_calls_tools() {
    let Some(py) = python() else { return };
    let mut env = IndexMap::new();
    env.insert("FAKE_MCP_REQUIRE_ENV".to_string(), "yes".to_string());
    let client = McpClient::from_command(&py, &[script()], Some(&env))
        .await
        .expect("connect");

    let names: Vec<&str> = client
        .tools()
        .iter()
        .map(|t| t["toolSpec"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["echo", "image", "resource"]);
    assert_eq!(
        client.tools()[0],
        json!({
            "toolSpec": {
                "name": "echo",
                "description": "Echo the input text",
                "inputSchema": { "json": {
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                } }
            }
        })
    );
    assert!(client.tools()[1]["toolSpec"].get("description").is_none());

    let out = client
        .call_tool("echo", &json!({ "text": "hi" }))
        .await
        .unwrap();
    assert_eq!(out, json!([{ "type": "text", "text": "echo: hi" }]));

    let out = client.call_tool("image", &json!({})).await.unwrap();
    assert_eq!(
        out,
        json!([{ "type": "image", "data": "iVBORw0KGgo=", "mimeType": "image/png" }])
    );

    // Non text/image content comes back as the stringified result
    let out = client.call_tool("resource", &json!({})).await.unwrap();
    let parsed: Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(parsed["content"][0]["type"], json!("resource"));

    assert!(client.call_tool("nope", &json!({})).await.is_err());
    client.cleanup().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn manager_routes_tool_calls_and_reuses_clients() {
    let Some(py) = python() else { return };
    let manager = McpManager::default();
    let servers = vec![stdio_server(&py)];

    let specs = manager.get_tool_specs(Some(&servers)).await.unwrap();
    assert_eq!(specs.len(), 3);

    let r = manager
        .execute_tool("echo", &json!({ "text": "yo" }), Some(&servers))
        .await;
    assert!(r.found && r.success, "{r:?}");
    assert_eq!(r.message, "MCP tool execution successful: echo");
    assert_eq!(r.result, json!([{ "type": "text", "text": "echo: yo" }]));

    let r = manager
        .execute_tool("missing", &json!({}), Some(&servers))
        .await;
    assert!(!r.found);
    assert_eq!(r.message, "No MCP server provides tool \"missing\"");

    // Through the adapter, the way the renderer's `mcp` tool calls it
    let agents = vec![json!({ "id": "a", "name": "A", "mcpServers": servers })];
    let out = mcp::adapter::execute(
        &manager,
        &json!({ "type": "mcp_echo", "text": "legacy" }),
        Some("a"),
        &agents,
    )
    .await
    .unwrap();
    assert_eq!(
        out["result"],
        json!([{ "type": "text", "text": "echo: legacy" }])
    );
    assert_eq!(manager.connected_servers().await, vec!["fake"]);

    let test = manager.test_connection(&servers[0]).await;
    assert!(test.success, "{test:?}");
    assert_eq!(
        test.message,
        "Successfully connected to MCP server \"fake\" via command"
    );
    assert_eq!(test.details.unwrap().tool_count, Some(3));

    manager.cleanup().await;
    assert!(manager.connected_servers().await.is_empty());
}

struct HttpServer {
    child: Child,
    port: u16,
    _dir: tempfile::TempDir,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn http_server(py: &str, mode: &str) -> HttpServer {
    let dir = tempfile::TempDir::new().unwrap();
    let port_file = dir.path().join("port");
    let child = Command::new(py)
        .arg(script())
        .arg("http")
        .arg(&port_file)
        .arg(mode)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let port = loop {
        if let Ok(s) = std::fs::read_to_string(&port_file) {
            break s.trim().parse().unwrap();
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "server did not start"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    HttpServer {
        child,
        port,
        _dir: dir,
    }
}

fn headers() -> IndexMap<String, String> {
    let mut h = IndexMap::new();
    h.insert("X-Test".to_string(), "1".to_string());
    h
}

#[tokio::test(flavor = "multi_thread")]
async fn streamable_http_with_headers() {
    let Some(py) = python() else { return };
    let server = http_server(&py, "streamable");
    let url = format!("http://127.0.0.1:{}/mcp", server.port);
    let client = McpClient::from_url(&url, Some(&headers()), reqwest::Client::new())
        .await
        .expect("connect");
    assert_eq!(client.tools().len(), 3);
    let out = client
        .call_tool("echo", &json!({ "text": "http" }))
        .await
        .unwrap();
    assert_eq!(out, json!([{ "type": "text", "text": "echo: http" }]));
    client.cleanup().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn falls_back_to_sse() {
    let Some(py) = python() else { return };
    let server = http_server(&py, "sse");
    let url = format!("http://127.0.0.1:{}/sse", server.port);
    let client = McpClient::from_url(&url, Some(&headers()), reqwest::Client::new())
        .await
        .expect("connect");
    assert_eq!(client.tools().len(), 3);
    let out = client
        .call_tool("echo", &json!({ "text": "sse" }))
        .await
        .unwrap();
    assert_eq!(out, json!([{ "type": "text", "text": "echo: sse" }]));
    client.cleanup().await.unwrap();
}
