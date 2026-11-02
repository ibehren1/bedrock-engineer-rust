//! One MCP server connection — port of `src/main/mcp/mcp-client.ts` (`MCPClient`).

use crate::command_resolver::{find_in_path, path_extensions, resolve_command};
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

/// Redirects for MCP HTTP connections: same origin only (AGENT-M5). Servers are often configured
/// with an API key or token in a custom header, which reqwest re-sends on a redirect unless the
/// header is one of the few it treats as sensitive, so a cross-origin hop would hand it to
/// another host.
pub fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let first = attempt.previous().first().cloned();
        if attempt.previous().len() >= 10 {
            attempt.error("too many redirects")
        } else if first.is_some_and(|f| f.origin() != attempt.url().origin()) {
            let target = attempt.url().origin().ascii_serialization();
            attempt.error(format!(
                "the MCP server redirected to another origin ({target}); refusing, so its headers aren't sent there"
            ))
        } else {
            attempt.follow()
        }
    })
}

/// Longest tool description passed on to the model; longer ones are cut.
const MAX_DESCRIPTION_CHARS: usize = 10_000;
/// Largest tool input schema (serialized) accepted; tools with bigger ones are dropped.
const MAX_SCHEMA_BYTES: usize = 100 * 1024;

/// Why a server-listed tool is dropped instead of offered to the model, if it is
/// (AGENT-M1 / M3 / M4). Bedrock tool names are `[A-Za-z0-9_-]{1,64}`; a name the app's own tools
/// use would sit next to the real built-in in the model's tool list and steer calls that run the
/// built-in; `mcp` and `mcp_…` names are the adapter's and its legacy prefix.
pub fn tool_name_problem(name: Option<&str>) -> Option<String> {
    let Some(name) = name else {
        return Some("it has no name".into());
    };
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        return Some(format!(
            "its name \"{name}\" isn't 1-64 letters, digits, '_' or '-'"
        ));
    }
    if common::tool_names::is_built_in_tool(name) {
        return Some(format!(
            "\"{name}\" is the name of one of the app's own tools"
        ));
    }
    if name.starts_with("mcp_") {
        return Some(format!("\"{name}\" uses the reserved mcp_ prefix"));
    }
    None
}

/// The listed tools that may be offered, as Bedrock specs, with long descriptions cut. Dropped
/// tools are logged with the reason.
fn accepted_tools(listed: impl IntoIterator<Item = Value>) -> Vec<Value> {
    listed
        .into_iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str);
            if let Some(problem) = tool_name_problem(name) {
                tracing::warn!(category = "mcp", problem = %problem, "Ignoring an MCP tool");
                return None;
            }
            let schema_size = tool
                .get("inputSchema")
                .map(|s| s.to_string().len())
                .unwrap_or(0);
            if schema_size > MAX_SCHEMA_BYTES {
                tracing::warn!(category = "mcp", tool = ?name, schema_size, "Ignoring an MCP tool with an oversized input schema");
                return None;
            }
            let mut spec = mcp_tool_to_bedrock(&tool);
            if let Some(Value::String(d)) = spec.pointer_mut("/toolSpec/description") {
                if d.chars().count() > MAX_DESCRIPTION_CHARS {
                    *d = d.chars().take(MAX_DESCRIPTION_CHARS).collect();
                }
            }
            Some(spec)
        })
        .collect()
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
/// (the TS spreads `process.env` after `env`), with `PATH` taken from the app. The `forced` keys
/// (filled in from the app's AWS settings, see [`crate::env_placeholders`]) keep their configured
/// value.
pub fn stdio_environment(
    env: Option<&IndexMap<String, String>>,
    forced: &[String],
) -> Vec<(String, String)> {
    let process: Vec<(String, String)> = std::env::vars().collect();
    let mut out: IndexMap<String, String> = env.cloned().unwrap_or_default();
    for (k, v) in process {
        if !forced.contains(&k) {
            out.insert(k, v);
        }
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
        let tools = accepted_tools(
            listed
                .tools
                .iter()
                .map(|t| serde_json::to_value(t).unwrap_or(Value::Null)),
        );
        tracing::info!(
            category = "mcp",
            tools = ?tools.iter().map(|t| t["toolSpec"]["name"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>(),
            "Connected to server with tools"
        );
        Ok(Self { service, tools })
    }

    /// `MCPClient.fromCommand`: spawn a stdio server. `forced` as in [`stdio_environment`].
    pub async fn from_command(
        command: &str,
        args: &[String],
        env: Option<&IndexMap<String, String>>,
        forced: &[String],
    ) -> Result<Self> {
        let resolved = resolve_command(command).map_err(Error::Mcp)?;
        if resolved != command {
            tracing::info!(category = "mcp", resolved = %resolved, original = %command, "Using resolved command path");
        }
        let environment = stdio_environment(env, forced);
        let program = if cfg!(windows) {
            // Find the file `PATH` + `PATHEXT` would run (`npx` -> `npx.cmd`), the lookup Node's
            // cross-spawn does, and start that file directly. Going through `cmd /c` instead
            // would let cmd.exe reinterpret `&`, `|`, `^` and `%VAR%` in the command and its
            // arguments (AGENT-M10); started directly, Rust quotes the arguments of a .cmd/.bat
            // itself and refuses ones it can't pass safely.
            let var = |name: &str| {
                environment
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.as_str())
            };
            let exts = path_extensions(var("PATHEXT"));
            match find_in_path(&resolved, var("PATH").unwrap_or_default(), ';', &exts) {
                Some(found) => found.to_string_lossy().into_owned(),
                None => return Err(Error::Mcp(format!("spawn {resolved} ENOENT"))),
            }
        } else {
            resolved.clone()
        };
        let mut cmd = tokio::process::Command::new(&program);
        cmd.args(args);
        cmd.env_clear();
        cmd.envs(environment);

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

    /// Connect over an in-memory stream to a server hosted in the app.
    pub async fn from_stream(stream: tokio::io::DuplexStream) -> Result<Self> {
        Self::initialize(stream).await
    }

    /// `MCPClient.fromUrl`: Streamable HTTP, falling back to legacy SSE.
    pub async fn from_url(
        url: &str,
        headers: Option<&IndexMap<String, String>>,
        http: reqwest::Client,
    ) -> Result<Self> {
        let base = reqwest::Url::parse(url).map_err(|e| Error::Mcp(format!("Invalid URL: {e}")))?;
        let header_map = header_map(headers)?;
        let loopback = matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if base.scheme() == "http" && !header_map.is_empty() && !loopback {
            // AGENT-M12: configured headers often hold API keys.
            tracing::warn!(
                category = "mcp",
                url = %base,
                "MCP server URL is plain http: its configured headers are sent unencrypted"
            );
        }

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

    // AGENT-M5: a redirect to another origin is refused (headers would follow it).
    #[tokio::test]
    async fn cross_origin_redirects_are_refused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            for target in [
                format!("http://127.0.0.1:{port}/same"),
                format!("http://localhost:{port}/other"),
            ] {
                for _ in 0..2 {
                    let Ok((mut sock, _)) = listener.accept().await else {
                        return;
                    };
                    let mut buf = [0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let reply = if req.starts_with("GET /start") {
                        format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    } else {
                        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                            .to_string()
                    };
                    let _ = sock.write_all(reply.as_bytes()).await;
                }
            }
        });
        let client = reqwest::Client::builder()
            .redirect(redirect_policy())
            .build()
            .unwrap();
        let start = format!("http://127.0.0.1:{port}/start");
        assert_eq!(client.get(&start).send().await.unwrap().status(), 200);
        let err = client.get(&start).send().await.unwrap_err();
        assert!(err.is_redirect(), "{err}");
    }

    // AGENT-M1 / M3 / M4: tools the model must not be offered are dropped at connect.
    #[test]
    fn listed_tools_are_screened() {
        let tool =
            |name: serde_json::Value| json!({ "name": name, "inputSchema": { "type": "object" } });
        let accepted = accepted_tools(vec![
            tool(json!("search_issues")),
            tool(json!("executeCommand")),
            tool(json!("mcp")),
            tool(json!("mcp_read")),
            tool(json!("has space")),
            tool(json!("x".repeat(65))),
            tool(serde_json::Value::Null),
            json!({ "name": "huge", "inputSchema": { "description": "x".repeat(200 * 1024) } }),
            json!({ "name": "wordy", "description": "d".repeat(20_000), "inputSchema": {} }),
        ]);
        let names: Vec<&str> = accepted
            .iter()
            .map(|t| t["toolSpec"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["search_issues", "wordy"]);
        assert_eq!(
            accepted[1]["toolSpec"]["description"]
                .as_str()
                .unwrap()
                .len(),
            MAX_DESCRIPTION_CHARS
        );
    }
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
        let out: HashMap<String, String> = stdio_environment(Some(&env), &[]).into_iter().collect();
        assert_eq!(out["MCP_TEST_ONLY_VAR"], "configured");
        assert_eq!(out["PATH"], std::env::var("PATH").unwrap_or_default());
    }

    #[test]
    fn forced_keys_beat_the_app_environment() {
        // A forced entry must not be replaced by the app's own value (HOME on Unix, USERPROFILE on
        // Windows; both are set in any test environment).
        let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let mut env = IndexMap::new();
        env.insert(key.to_string(), "/configured".to_string());
        let out: HashMap<String, String> = stdio_environment(Some(&env), &[key.to_string()])
            .into_iter()
            .collect();
        assert_eq!(out[key], "/configured");
        let out: HashMap<String, String> = stdio_environment(Some(&env), &[]).into_iter().collect();
        assert_eq!(out[key], std::env::var(key).unwrap());
    }
}
