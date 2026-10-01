//! One MCP server connection — port of `src/main/mcp/mcp-client.ts` (`MCPClient`).

use crate::command_resolver::resolve_command;
use crate::sse::SseClientTransport;
use crate::{Error, Result};
use common::zod::{array, object, req, Schema};
use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation, JsonObject,
};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

/// The TS SDK's default request timeout (`DEFAULT_REQUEST_TIMEOUT_MSEC`).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Client identity sent in `initialize` (`{ name: 'mcp-client-cli', version: '1.0.0' }`).
pub fn client_info() -> ClientConfig {
    ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new("mcp-client-cli", "1.0.0"),
    )
}

fn timed_out() -> Error {
    Error::Mcp("MCP error -32001: Request timed out".into())
}

async fn with_timeout<T, E: std::fmt::Display>(
    fut: impl std::future::Future<Output = std::result::Result<T, E>>,
) -> Result<T> {
    match tokio::time::timeout(REQUEST_TIMEOUT, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(Error::Mcp(e.to_string())),
        Err(_) => Err(timed_out()),
    }
}

/// Convert an MCP tool (as JSON: `{ name, description?, inputSchema }`) to a Bedrock `Tool`
/// (`{ toolSpec: { name, description, inputSchema: { json } } }`). Names are kept unchanged.
pub fn mcp_tool_to_bedrock(tool: &Value) -> Value {
    let mut spec = serde_json::Map::new();
    spec.insert(
        "name".into(),
        tool.get("name").cloned().unwrap_or(Value::Null),
    );
    if let Some(d) = tool.get("description").filter(|d| !d.is_null()) {
        spec.insert("description".into(), d.clone());
    }
    spec.insert(
        "inputSchema".into(),
        json!({ "json": tool.get("inputSchema").cloned().unwrap_or(Value::Null) }),
    );
    json!({ "toolSpec": Value::Object(spec) })
}

fn content_schema() -> &'static Schema {
    static CELL: OnceLock<Schema> = OnceLock::new();
    CELL.get_or_init(|| {
        array(Schema::Union(vec![
            object(vec![
                req("type", Schema::Enum(vec!["text"])),
                req("text", Schema::String),
            ]),
            object(vec![
                req("type", Schema::Enum(vec!["image"])),
                req("data", Schema::String),
                req("mimeType", Schema::String),
            ]),
        ]))
    })
}

/// Shape a `tools/call` result the way `MCPClient.callTool` does: the `content` array when it is
/// only text/image blocks (extra keys stripped), otherwise the whole result as a JSON string.
pub fn shape_call_tool_result(result: &Value) -> Value {
    match result
        .get("content")
        .map(|c| content_schema().safe_parse(c))
    {
        Some(Ok(content)) => content,
        _ => Value::String(serde_json::to_string(result).unwrap_or_default()),
    }
}

/// Environment for a stdio server: the configured `env`, overridden by the app's own environment
/// (the TS spreads `process.env` after `env`), with `PATH` taken from the app.
pub fn stdio_environment(env: Option<&IndexMap<String, String>>) -> Vec<(String, String)> {
    let process: Vec<(String, String)> = std::env::vars().collect();
    let mut out: IndexMap<String, String> = env.cloned().unwrap_or_default();
    for (k, v) in process {
        out.insert(k, v);
    }
    out.insert("PATH".into(), std::env::var("PATH").unwrap_or_default());
    out.into_iter().collect()
}

fn header_map(headers: Option<&IndexMap<String, String>>) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for (k, v) in headers.into_iter().flatten() {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| Error::Mcp(format!("Invalid header name {k}: {e}")))?;
        let value = HeaderValue::from_str(v)
            .map_err(|e| Error::Mcp(format!("Invalid header value for {k}: {e}")))?;
        map.insert(name, value);
    }
    Ok(map)
}

/// A connected MCP client with its tool list cached as Bedrock tool specs.
pub struct McpClient {
    service: RunningService<RoleClient, ClientConfig>,
    tools: Vec<Value>,
}

impl McpClient {
    async fn initialize<T, E, A>(transport: T) -> Result<Self>
    where
        T: rmcp::transport::IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let service = with_timeout(client_info().serve(transport)).await?;
        let listed = with_timeout(service.list_tools(None)).await?;
        let tools = listed
            .tools
            .iter()
            .map(|t| mcp_tool_to_bedrock(&serde_json::to_value(t).unwrap_or(Value::Null)))
            .collect::<Vec<_>>();
        tracing::info!(
            category = "mcp",
            tools = ?tools.iter().map(|t| t["toolSpec"]["name"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>(),
            "Connected to server with tools"
        );
        Ok(Self { service, tools })
    }

    /// `MCPClient.fromCommand`: spawn a stdio server.
    pub async fn from_command(
        command: &str,
        args: &[String],
        env: Option<&IndexMap<String, String>>,
    ) -> Result<Self> {
        let resolved = resolve_command(command);
        if resolved != command {
            tracing::info!(category = "mcp", resolved = %resolved, original = %command, "Using resolved command path");
        }
        let mut cmd = if cfg!(windows) {
            // Node's cross-spawn resolves `npx` to `npx.cmd`; `cmd /c` gives the same lookup.
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/c").arg(&resolved);
            c
        } else {
            tokio::process::Command::new(&resolved)
        };
        cmd.args(args);
        cmd.env_clear();
        cmd.envs(stdio_environment(env));

        let (transport, _stderr) = TokioChildProcess::builder(cmd)
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                Error::Mcp(match e.kind() {
                    std::io::ErrorKind::NotFound => format!("spawn {resolved} ENOENT"),
                    std::io::ErrorKind::PermissionDenied => format!("spawn {resolved} EACCES"),
                    _ => e.to_string(),
                })
            })?;
        Self::initialize(transport).await.inspect_err(|e| {
            tracing::warn!(category = "mcp", error = %e, "Failed to connect to MCP server");
        })
    }

    /// `MCPClient.fromUrl`: Streamable HTTP, falling back to legacy SSE.
    pub async fn from_url(
        url: &str,
        headers: Option<&IndexMap<String, String>>,
        http: reqwest::Client,
    ) -> Result<Self> {
        let base = reqwest::Url::parse(url).map_err(|e| Error::Mcp(format!("Invalid URL: {e}")))?;
        let header_map = header_map(headers)?;

        let custom: HashMap<HeaderName, HeaderValue> = header_map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let config = StreamableHttpClientTransportConfig::with_uri(url).custom_headers(custom);
        let transport = StreamableHttpClientTransport::with_client(http.clone(), config);
        match Self::initialize(transport).await {
            Ok(client) => {
                tracing::info!(
                    category = "mcp",
                    "Connected using Streamable HTTP transport"
                );
                Ok(client)
            }
            Err(e) => {
                tracing::info!(category = "mcp", error = %e, "Streamable HTTP connection failed, falling back to SSE transport");
                let transport =
                    SseClientTransport::connect(http, base, header_map, REQUEST_TIMEOUT)
                        .await
                        .map_err(|e| Error::Mcp(e.to_string()))?;
                let client = Self::initialize(transport).await?;
                tracing::info!(category = "mcp", "Connected using SSE transport");
                Ok(client)
            }
        }
    }

    /// Bedrock tool specs for this server's tools (names unchanged).
    pub fn tools(&self) -> &[Value] {
        &self.tools
    }

    /// Whether this server provides `tool_name`.
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tools
            .iter()
            .any(|t| t["toolSpec"]["name"].as_str() == Some(tool_name))
    }

    /// `callTool(toolName, input)`; see [`shape_call_tool_result`].
    pub async fn call_tool(&self, tool_name: &str, input: &Value) -> Result<Value> {
        let arguments: JsonObject = input.as_object().cloned().unwrap_or_default();
        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);
        let result = with_timeout(self.service.call_tool(params)).await?;
        let value = serde_json::to_value(&result)?;
        Ok(shape_call_tool_result(&value))
    }

    /// `cleanup()`: close the connection (and stop a stdio child).
    pub async fn cleanup(mut self) -> Result<()> {
        self.service
            .close()
            .await
            .map(|_| ())
            .map_err(|e| Error::Mcp(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_tools_to_bedrock_specs() {
        let tool = json!({
            "name": "search_docs",
            "description": "Search",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        });
        assert_eq!(
            mcp_tool_to_bedrock(&tool),
            json!({
                "toolSpec": {
                    "name": "search_docs",
                    "description": "Search",
                    "inputSchema": { "json": { "type": "object", "properties": {} } }
                }
            })
        );
        let no_desc = json!({ "name": "x", "inputSchema": { "type": "object" } });
        assert!(mcp_tool_to_bedrock(&no_desc)["toolSpec"]
            .get("description")
            .is_none());
    }

    #[test]
    fn shapes_text_and_image_content() {
        let result = json!({
            "content": [
                { "type": "text", "text": "hi", "annotations": {} },
                { "type": "image", "data": "AAA", "mimeType": "image/png" }
            ],
            "isError": false
        });
        assert_eq!(
            shape_call_tool_result(&result),
            json!([
                { "type": "text", "text": "hi" },
                { "type": "image", "data": "AAA", "mimeType": "image/png" }
            ])
        );
    }

    #[test]
    fn stringifies_other_content() {
        let result = json!({ "content": [{ "type": "resource", "resource": {} }] });
        assert_eq!(
            shape_call_tool_result(&result),
            Value::String(result.to_string())
        );
    }

    #[test]
    fn app_environment_overrides_configured_env() {
        let mut env = IndexMap::new();
        env.insert("MCP_TEST_ONLY_VAR".to_string(), "configured".to_string());
        env.insert("PATH".to_string(), "/nope".to_string());
        let out: HashMap<String, String> = stdio_environment(Some(&env)).into_iter().collect();
        assert_eq!(out["MCP_TEST_ONLY_VAR"], "configured");
        assert_eq!(out["PATH"], std::env::var("PATH").unwrap_or_default());
    }
}
