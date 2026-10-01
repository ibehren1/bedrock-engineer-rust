//! Per-agent MCP client pool — port of `src/main/mcp/index.ts` and the result shapes of
//! `src/main/handlers/mcp-handlers.ts`.
//!
//! The TS module keeps module-level state; here it lives in an [`McpManager`] the app holds in
//! managed state. Clients are (re)created only when the server config materially changes.

use crate::client::McpClient;
use crate::schemas::mcp_server_config_schema;
use crate::utils::infer_connection_type;
use crate::{Error, Result};
use common::agent::{ConnectionType, McpServerConfig};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::time::Instant;
use tokio::sync::Mutex;

const CATEGORY: &str = "mcp";

/// `generateConfigHash`: a stable string of the parts of the config that matter.
pub fn generate_config_hash(servers: &[McpServerConfig]) -> String {
    if servers.is_empty() {
        return "empty".into();
    }
    let mut sorted: Vec<&McpServerConfig> = servers.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let essential: Vec<Value> = sorted
        .iter()
        .map(|server| {
            let connection_type = infer_connection_type(server);
            let mut obj = Map::new();
            obj.insert("name".into(), json!(server.name));
            obj.insert("connectionType".into(), json!(connection_type));
            if connection_type == ConnectionType::Command {
                if let (Some(command), Some(args)) = (
                    server.command.as_deref().filter(|c| !c.is_empty()),
                    &server.args,
                ) {
                    obj.insert("command".into(), json!(command));
                    obj.insert("args".into(), json!(args));
                }
            }
            if connection_type == ConnectionType::Url {
                if let Some(url) = server.url.as_deref().filter(|u| !u.is_empty()) {
                    obj.insert("url".into(), json!(url));
                }
            }
            if let Some(env) = server.env.as_ref().filter(|e| !e.is_empty()) {
                obj.insert("env".into(), json!(env));
            }
            Value::Object(obj)
        })
        .collect();
    serde_json::to_string(&essential).unwrap_or_default()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ConfigCache {
    hash: Option<String>,
    length: usize,
    names: Vec<String>,
}

impl ConfigCache {
    /// `hasConfigChanged`
    fn has_changed(&self, servers: &[McpServerConfig]) -> bool {
        if servers.len() != self.length {
            return true;
        }
        if servers.is_empty() && self.length == 0 {
            return false;
        }
        let mut names: Vec<String> = servers.iter().map(|s| s.name.clone()).collect();
        names.sort();
        if names != self.names {
            return true;
        }
        self.hash.as_deref() != Some(generate_config_hash(servers).as_str())
    }

    /// `updateConfigCache`
    fn update(&mut self, servers: &[McpServerConfig]) {
        self.hash = Some(generate_config_hash(servers));
        self.length = servers.len();
        let mut names: Vec<String> = servers.iter().map(|s| s.name.clone()).collect();
        names.sort();
        self.names = names;
    }
}

#[derive(Default)]
struct State {
    clients: Vec<(String, McpClient)>,
    cache: ConfigCache,
}

/// `tryExecuteMcpTool` / `mcp:executeTool` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpToolExecution {
    pub found: bool,
    pub success: bool,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub message: String,
    pub result: Value,
}

/// `details` of a connection test.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTestDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_time: Option<u64>,
}

/// `testMcpServerConnection` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<McpTestDetails>,
}

/// `${mcpServer.connectionType}` in the TS messages — the configured value, `undefined` if unset.
fn connection_type_label(server: &McpServerConfig) -> &'static str {
    match server.connection_type {
        Some(ConnectionType::Command) => "command",
        Some(ConnectionType::Url) => "url",
        None => "undefined",
    }
}

/// `analyzeServerError`: a hint for a failed connection.
pub fn analyze_server_error(error_message: &str) -> &'static str {
    let lower = error_message.to_lowercase();
    if lower.contains("enoent") || lower.contains("command not found") {
        return "Command not found. Please make sure the command is installed and the path is correct.";
    }
    if lower.contains("timeout") {
        return "The response from the server timed out. Please check if the server is running properly.";
    }
    if lower.contains("permission denied") || lower.contains("eacces") {
        return "A permission error occurred. Please make sure you have the execution permissions.";
    }
    if lower.contains("port") && lower.contains("use") {
        return "The port is already in use. Please make sure that no other process is using the same port.";
    }
    if lower.contains("cors") || lower.contains("access-control-allow-origin") {
        return "CORS policy blocked the request. This error should not occur in Main Process - please check the implementation.";
    }
    "Please make sure your command and arguments are correct."
}

fn is_command_server(s: &McpServerConfig) -> bool {
    infer_connection_type(s) == ConnectionType::Command && s.command.is_some()
}

fn is_url_server(s: &McpServerConfig) -> bool {
    infer_connection_type(s) == ConnectionType::Url
        && s.url.as_deref().is_some_and(|u| !u.is_empty())
}

/// The `{ mcpServers }` object the TS validates before connecting.
fn config_data(servers: &[McpServerConfig]) -> Value {
    let mut map = Map::new();
    for s in servers.iter().filter(|s| is_command_server(s)) {
        map.insert(
            s.name.clone(),
            json!({ "command": s.command, "args": s.args, "env": s.env.clone().unwrap_or_default() }),
        );
    }
    for s in servers.iter().filter(|s| is_url_server(s)) {
        let mut entry = json!({ "url": s.url, "enabled": true });
        if let Some(h) = &s.headers {
            entry["headers"] = json!(h);
        }
        map.insert(s.name.clone(), entry);
    }
    json!({ "mcpServers": map })
}

/// Connects to an MCP server according to its config.
async fn connect(server: &McpServerConfig, http: &reqwest::Client) -> Result<McpClient> {
    match infer_connection_type(server) {
        ConnectionType::Command => {
            let command = server.command.as_deref().unwrap_or_default();
            let args = server.args.clone().unwrap_or_default();
            McpClient::from_command(command, &args, server.env.as_ref()).await
        }
        ConnectionType::Url => {
            McpClient::from_url(
                server.url.as_deref().unwrap_or_default(),
                server.headers.as_ref(),
                http.clone(),
            )
            .await
        }
    }
}

/// The MCP client pool for the current agent's servers.
pub struct McpManager {
    state: Mutex<State>,
    http: reqwest::Client,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new(reqwest::Client::new())
    }
}

impl McpManager {
    /// `http` is used for URL servers (configure the app's proxy on it).
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            state: Mutex::new(State::default()),
            http,
        }
    }

    /// `initMcpFromAgentConfig`: (re)connect when the config changed. Connection failures of
    /// individual servers are logged and skipped; an invalid config is an error.
    pub async fn init_from_agent_config(&self, servers: &[McpServerConfig]) -> Result<()> {
        // Holding the lock for the whole initialization serializes concurrent calls, like the TS
        // `initializationInProgress` promise; waiters re-check for changes afterwards.
        let mut state = self.state.lock().await;
        if !state.cache.has_changed(servers) {
            tracing::debug!(category = CATEGORY, "No MCP server config changes detected");
            return Ok(());
        }
        tracing::info!(
            category = CATEGORY,
            count = servers.len(),
            "Initializing MCP servers"
        );

        for (_, client) in state.clients.drain(..) {
            let _ = client.cleanup().await;
        }

        if servers.is_empty() {
            state.cache.update(servers);
            return Ok(());
        }

        if let Err(issues) = mcp_server_config_schema().safe_parse(&config_data(servers)) {
            tracing::error!(category = CATEGORY, error = %crate::schemas::format_zod_error(&issues), "Invalid MCP server configuration");
            state.cache = ConfigCache::default();
            return Err(Error::Mcp("Invalid MCP server configuration".into()));
        }

        let targets: Vec<&McpServerConfig> = servers
            .iter()
            .filter(|s| is_command_server(s))
            .chain(servers.iter().filter(|s| is_url_server(s)))
            .collect();
        let connections = futures_util::future::join_all(targets.iter().map(|server| async {
            tracing::info!(category = CATEGORY, server = %server.name, "Connecting to MCP server");
            match connect(server, &self.http).await {
                Ok(client) => {
                    tracing::info!(category = CATEGORY, server = %server.name, "Successfully connected to MCP server");
                    Some((server.name.clone(), client))
                }
                Err(e) => {
                    tracing::warn!(category = CATEGORY, server = %server.name, error = %e, "Failed to connect to MCP server");
                    None
                }
            }
        }))
        .await;

        state.clients = connections.into_iter().flatten().collect();
        tracing::info!(
            category = CATEGORY,
            count = state.clients.len(),
            "Total connected clients"
        );
        state.cache.update(servers);
        Ok(())
    }

    /// `getMcpToolSpecs`: Bedrock tool specs of every connected server (names unchanged).
    pub async fn get_tool_specs(&self, servers: Option<&[McpServerConfig]>) -> Result<Vec<Value>> {
        let Some(servers) = servers.filter(|s| !s.is_empty()) else {
            return Ok(Vec::new());
        };
        self.init_from_agent_config(servers).await?;
        let state = self.state.lock().await;
        Ok(state
            .clients
            .iter()
            .flat_map(|(_, c)| c.tools().iter().cloned())
            .collect())
    }

    /// `tryExecuteMcpTool`
    pub async fn try_execute_tool(
        &self,
        tool_name: &str,
        input: &Value,
        servers: Option<&[McpServerConfig]>,
    ) -> Result<McpToolExecution> {
        let Some(servers) = servers.filter(|s| !s.is_empty()) else {
            return Ok(McpToolExecution {
                found: false,
                success: false,
                name: tool_name.into(),
                error: Some("No MCP servers configured".into()),
                message: "This agent does not have any MCP servers configured. Please add MCP server configuration in agent settings.".into(),
                result: Value::Null,
            });
        };
        self.init_from_agent_config(servers).await?;

        let state = self.state.lock().await;
        let Some((_, client)) = state.clients.iter().find(|(_, c)| c.has_tool(tool_name)) else {
            return Ok(McpToolExecution {
                found: false,
                success: false,
                name: tool_name.into(),
                error: Some(format!("MCP tool \"{tool_name}\" not found")),
                message: format!("No MCP server provides tool \"{tool_name}\""),
                result: Value::Null,
            });
        };

        let params = if input.is_object() {
            input.clone()
        } else {
            json!({})
        };
        Ok(match client.call_tool(tool_name, &params).await {
            Ok(result) => McpToolExecution {
                found: true,
                success: true,
                name: tool_name.into(),
                error: None,
                message: format!("MCP tool execution successful: {tool_name}"),
                result,
            },
            Err(e) => {
                let msg = e.to_string();
                tracing::error!(category = CATEGORY, tool = tool_name, error = %msg, "Error executing MCP tool");
                McpToolExecution {
                    found: true,
                    success: false,
                    name: tool_name.into(),
                    message: format!("Error executing MCP tool \"{tool_name}\": {msg}"),
                    error: Some(msg),
                    result: Value::Null,
                }
            }
        })
    }

    /// The `mcp:executeTool` handler: [`Self::try_execute_tool`] with errors folded into the
    /// result.
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        input: &Value,
        servers: Option<&[McpServerConfig]>,
    ) -> McpToolExecution {
        match self.try_execute_tool(tool_name, input, servers).await {
            Ok(r) => r,
            Err(e) => McpToolExecution {
                found: false,
                success: false,
                name: tool_name.into(),
                message: format!("Error executing MCP tool \"{tool_name}\": {e}"),
                error: Some(e.to_string()),
                result: Value::Null,
            },
        }
    }

    /// `testMcpServerConnection`: connect, list tools, disconnect.
    pub async fn test_connection(&self, server: &McpServerConfig) -> McpTestResult {
        test_mcp_server_connection(server, &self.http).await
    }

    /// `testAllMcpServerConnections`: sequentially, keyed by server name.
    pub async fn test_all_connections(
        &self,
        servers: &[McpServerConfig],
    ) -> BTreeMap<String, McpTestResult> {
        let mut results = BTreeMap::new();
        for server in servers {
            results.insert(server.name.clone(), self.test_connection(server).await);
        }
        results
    }

    /// `cleanupMcpClients`
    pub async fn cleanup(&self) {
        let mut state = self.state.lock().await;
        for (name, client) in state.clients.drain(..) {
            match client.cleanup().await {
                Ok(()) => {
                    tracing::info!(category = CATEGORY, server = %name, "Cleaned up MCP client")
                }
                Err(e) => {
                    tracing::error!(category = CATEGORY, server = %name, error = %e, "Error cleaning up MCP client")
                }
            }
        }
        state.cache = ConfigCache::default();
    }

    /// Names of the currently connected servers.
    pub async fn connected_servers(&self) -> Vec<String> {
        self.state
            .lock()
            .await
            .clients
            .iter()
            .map(|(n, _)| n.clone())
            .collect()
    }
}

/// `testMcpServerConnection` without a manager.
pub async fn test_mcp_server_connection(
    server: &McpServerConfig,
    http: &reqwest::Client,
) -> McpTestResult {
    let start = Instant::now();
    let invalid = |message: String, details: &str| McpTestResult {
        success: false,
        message,
        details: Some(McpTestDetails {
            error: Some("Invalid server configuration".into()),
            error_details: Some(details.into()),
            ..Default::default()
        }),
    };
    let client = match infer_connection_type(server) {
        ConnectionType::Command => {
            let (Some(command), Some(args)) = (
                server.command.as_deref().filter(|c| !c.is_empty()),
                server.args.as_ref(),
            ) else {
                return invalid(
                    format!(
                        "MCP server \"{}\" is missing required command or args",
                        server.name
                    ),
                    "Command and args fields are required for command-type servers",
                );
            };
            McpClient::from_command(command, args, server.env.as_ref()).await
        }
        ConnectionType::Url => {
            let Some(url) = server.url.as_deref().filter(|u| !u.is_empty()) else {
                return invalid(
                    format!("MCP server \"{}\" is missing required URL", server.name),
                    "URL field is required for URL-type servers",
                );
            };
            McpClient::from_url(url, server.headers.as_ref(), http.clone()).await
        }
    };

    match client {
        Ok(client) => {
            let tool_names: Vec<String> = client
                .tools()
                .iter()
                .filter_map(|t| t["toolSpec"]["name"].as_str().map(String::from))
                .collect();
            let tool_count = client.tools().len();
            let _ = client.cleanup().await;
            McpTestResult {
                success: true,
                message: format!(
                    "Successfully connected to MCP server \"{}\" via {}",
                    server.name,
                    connection_type_label(server)
                ),
                details: Some(McpTestDetails {
                    tool_count: Some(tool_count),
                    tool_names: Some(tool_names),
                    startup_time: Some(start.elapsed().as_millis() as u64),
                    ..Default::default()
                }),
            }
        }
        Err(e) => {
            let msg = e.to_string();
            tracing::error!(category = CATEGORY, server = %server.name, error = %msg, "Failed to test MCP server");
            McpTestResult {
                success: false,
                message: format!(
                    "Failed to connect to MCP server \"{}\" via {}",
                    server.name,
                    connection_type_label(server)
                ),
                details: Some(McpTestDetails {
                    error_details: Some(analyze_server_error(&msg).into()),
                    error: Some(msg),
                    ..Default::default()
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str, command: &str) -> McpServerConfig {
        McpServerConfig {
            name: name.into(),
            description: String::new(),
            command: Some(command.into()),
            args: Some(vec![]),
            ..Default::default()
        }
    }

    #[test]
    fn config_hash_is_order_independent_and_ignores_descriptions() {
        let a = cmd("a", "node");
        let mut b = cmd("b", "python");
        let h1 = generate_config_hash(&[a.clone(), b.clone()]);
        b.description = "changed".into();
        assert_eq!(h1, generate_config_hash(&[b.clone(), a.clone()]));
        b.command = Some("uvx".into());
        assert_ne!(h1, generate_config_hash(&[a, b]));
        assert_eq!(generate_config_hash(&[]), "empty");
    }

    #[test]
    fn cache_detects_changes() {
        let mut cache = ConfigCache::default();
        assert!(!cache.has_changed(&[]));
        let servers = vec![cmd("a", "node")];
        assert!(cache.has_changed(&servers));
        cache.update(&servers);
        assert!(!cache.has_changed(&servers));
        assert!(cache.has_changed(&[cmd("b", "node")]));
    }

    #[test]
    fn analyzes_errors() {
        assert!(analyze_server_error("spawn foo ENOENT").starts_with("Command not found"));
        assert!(analyze_server_error("Request timeout").contains("timed out"));
        assert!(analyze_server_error("EACCES").contains("permission"));
        assert!(analyze_server_error("port already in use").contains("port"));
        assert!(analyze_server_error("???").starts_with("Please make sure"));
    }

    #[tokio::test]
    async fn no_servers_means_no_tools_and_not_found() {
        let m = McpManager::default();
        assert!(m.get_tool_specs(None).await.unwrap().is_empty());
        let r = m.execute_tool("x", &json!({}), Some(&[])).await;
        assert!(!r.found);
        assert_eq!(r.error.as_deref(), Some("No MCP servers configured"));
        assert_eq!(serde_json::to_value(&r).unwrap()["result"], Value::Null);
    }

    #[tokio::test]
    async fn test_connection_reports_missing_fields() {
        let m = McpManager::default();
        let server = McpServerConfig {
            name: "u".into(),
            connection_type: Some(ConnectionType::Url),
            ..Default::default()
        };
        let r = m.test_connection(&server).await;
        assert!(!r.success);
        assert_eq!(r.message, "MCP server \"u\" is missing required URL");
    }

    #[tokio::test]
    async fn failed_connection_is_skipped_not_fatal() {
        let m = McpManager::default();
        let servers = vec![cmd("broken", "definitely-not-a-real-mcp-server-xyz")];
        assert!(m.get_tool_specs(Some(&servers)).await.unwrap().is_empty());
        assert!(m.connected_servers().await.is_empty());
        let r = m.test_connection(&servers[0]).await;
        assert!(!r.success);
        let details = r.details.unwrap();
        assert!(details.error.unwrap().contains("ENOENT"));
        assert!(details
            .error_details
            .unwrap()
            .starts_with("Command not found"));
    }
}
