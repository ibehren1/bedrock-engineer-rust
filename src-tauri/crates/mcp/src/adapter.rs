//! The MCP tool adapter — port of `src/preload/tools/handlers/mcp/McpToolAdapter.ts`.
//!
//! The renderer sends every non-built-in tool use to the `mcp` tool with `type` set to the tool
//! name (see [`common::tool_names::resolve_tool_dispatch`]). This module turns that input into a
//! call against the right agent's MCP servers and maps the outcome to the adapter's `ToolResult`
//! or error message.

use crate::manager::{McpManager, McpToolExecution};
use common::agent::McpServerConfig;
use common::tool_names::is_legacy_mcp_tool;
use serde_json::{json, Map, Value};

/// A parsed adapter input.
#[derive(Debug, Clone, PartialEq)]
pub struct McpCall {
    /// The MCP tool name (legacy `mcp_` prefix removed).
    pub tool_name: String,
    /// Arguments for the tool: the input minus `type`, `mcpToolName` and the internal
    /// `_agentId` / `_mcpServers` keys.
    pub args: Value,
}

/// MCP servers supplied by a trusted caller (a background agent's stored definition), with the
/// agent id they belong to.
#[derive(Debug, Clone, Copy)]
pub struct TrustedServers<'a> {
    pub agent_id: &'a str,
    pub servers: &'a [McpServerConfig],
}

/// `validateInput`: `type` must be a non-empty string.
pub fn validate_input(input: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    match input.get("type") {
        Some(Value::String(s)) if !s.is_empty() => {}
        Some(Value::String(_)) | None | Some(Value::Null) => {
            errors.push("Tool type is required".into());
            if !matches!(input.get("type"), Some(Value::String(_))) {
                errors.push("Tool type must be a string".into());
            }
        }
        Some(_) => errors.push("Tool type must be a string".into()),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Split an adapter input into the tool name and its arguments.
///
/// The TS adapter also took `_agentId` + `_mcpServers` from the input to pick the servers,
/// which let a model spawn any command as an "MCP server". Those keys are dropped here and
/// never honored; trusted servers come in through [`resolve_servers`]'s `trusted` argument.
pub fn parse_call(input: &Value) -> McpCall {
    let tool_type = input
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let explicit = input
        .get("mcpToolName")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let tool_name = match explicit {
        Some(n) => n.to_string(),
        None if is_legacy_mcp_tool(tool_type) => tool_type["mcp_".len()..].to_string(),
        None => tool_type.to_string(),
    };

    let mut args: Map<String, Value> = input.as_object().cloned().unwrap_or_default();
    for internal in ["type", "mcpToolName", "_agentId", "_mcpServers"] {
        args.remove(internal);
    }

    McpCall {
        tool_name,
        args: Value::Object(args),
    }
}

/// The servers to use and a display name for the agent.
///
/// Trusted background-agent servers win; otherwise the selected agent is looked up in the
/// store's `customAgents`. Errors carry the TS adapter's messages.
pub fn resolve_servers(
    trusted: Option<TrustedServers<'_>>,
    selected_agent_id: Option<&str>,
    custom_agents: &[Value],
) -> Result<(Vec<McpServerConfig>, String), String> {
    let (servers, agent_name) = match trusted {
        Some(t) => (
            t.servers.to_vec(),
            format!("BackgroundAgent-{}", t.agent_id),
        ),
        None => {
            let Some(selected) = selected_agent_id.filter(|s| !s.is_empty()) else {
                return Err("No agent selected. Please select an agent to use MCP tools.".into());
            };
            let Some(agent) = custom_agents
                .iter()
                .find(|a| a.get("id").and_then(Value::as_str) == Some(selected))
            else {
                return Err(format!(
                    "Agent not found: {selected}. Please check your agent configuration."
                ));
            };
            let servers = agent
                .get("mcpServers")
                .cloned()
                .and_then(|v| serde_json::from_value::<Vec<McpServerConfig>>(v).ok())
                .unwrap_or_default();
            let name = agent
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (servers, name)
        }
    };
    if servers.is_empty() {
        return Err(
            "No MCP servers configured for this agent. Please configure MCP servers in agent settings."
                .into(),
        );
    }
    Ok((servers, agent_name))
}

/// Map an execution outcome to the adapter's `ToolResult` (`{ success, name: 'mcp', message,
/// result }`) or its error message.
pub fn to_tool_result(tool_name: &str, execution: McpToolExecution) -> Result<Value, String> {
    if !execution.found {
        return Err(if execution.message.is_empty() {
            format!(
                "MCP tool not found: {tool_name}. Please check if the tool is available in your configured MCP servers."
            )
        } else {
            execution.message
        });
    }
    if !execution.success {
        return Err(if !execution.message.is_empty() {
            execution.message
        } else {
            execution
                .error
                .unwrap_or_else(|| "MCP tool execution failed".into())
        });
    }
    let message = if execution.message.is_empty() {
        format!("Executed MCP tool: {tool_name}")
    } else {
        execution.message
    };
    Ok(json!({
        "success": true,
        "name": "mcp",
        "message": message,
        "result": execution.result
    }))
}

/// Run an adapter input end to end.
pub async fn execute(
    manager: &McpManager,
    input: &Value,
    selected_agent_id: Option<&str>,
    custom_agents: &[Value],
) -> Result<Value, String> {
    validate_input(input).map_err(|e| e.join(", "))?;
    let call = parse_call(input);
    let (servers, agent_name) = resolve_servers(None, selected_agent_id, custom_agents)?;
    tracing::debug!(category = "tool:mcp", tool = %call.tool_name, agent = %agent_name, servers = servers.len(), "Calling MCP tool");
    let execution = manager
        .execute_tool(&call.tool_name, &call.args, Some(&servers))
        .await;
    to_tool_result(&call.tool_name, execution)
}

/// `sanitizeObject`: redact password/token/secret/key fields, truncate strings over 100 chars.
/// Nested arrays become index-keyed objects, as `Object.entries` does in the TS.
pub fn sanitize_for_logging(value: &Value) -> Value {
    let entries: Vec<(String, &Value)> = match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        other => return other.clone(),
    };
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| {
                let lower = k.to_lowercase();
                let sanitized = if ["password", "token", "secret", "key"]
                    .iter()
                    .any(|s| lower.contains(s))
                {
                    Value::String("[REDACTED]".into())
                } else {
                    match v {
                        Value::String(s) if s.encode_utf16().count() > 100 => {
                            let head: Vec<u16> = s.encode_utf16().take(100).collect();
                            Value::String(format!("{}...", String::from_utf16_lossy(&head)))
                        }
                        Value::Object(_) | Value::Array(_) => sanitize_for_logging(v),
                        other => other.clone(),
                    }
                };
                (k, sanitized)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_args() {
        let call = parse_call(&json!({
            "type": "search", "mcpToolName": "search", "query": "x",
            "_agentId": "bg", "_mcpServers": [{ "name": "s", "description": "", "command": "node", "args": [] }]
        }));
        assert_eq!(call.tool_name, "search");
        // Internal keys never reach the server.
        assert_eq!(call.args, json!({ "query": "x" }));

        assert_eq!(
            parse_call(&json!({ "type": "mcp_legacy" })).tool_name,
            "legacy"
        );
    }

    #[test]
    fn validates_type() {
        assert!(validate_input(&json!({ "type": "x" })).is_ok());
        assert!(validate_input(&json!({})).is_err());
        assert_eq!(
            validate_input(&json!({ "type": 3 })).unwrap_err(),
            vec!["Tool type must be a string"]
        );
    }

    #[test]
    fn resolves_servers_from_trusted_context_or_selected_agent() {
        let agents = vec![json!({
            "id": "a1", "name": "Agent", "mcpServers": [{ "name": "s", "description": "", "url": "https://x" }]
        })];
        assert_eq!(
            resolve_servers(None, None, &agents).unwrap_err(),
            "No agent selected. Please select an agent to use MCP tools."
        );
        assert_eq!(
            resolve_servers(None, Some("zz"), &agents).unwrap_err(),
            "Agent not found: zz. Please check your agent configuration."
        );
        let (servers, name) = resolve_servers(None, Some("a1"), &agents).unwrap();
        assert_eq!((servers.len(), name.as_str()), (1, "Agent"));

        let bg = [McpServerConfig {
            name: "bg".into(),
            ..Default::default()
        }];
        let trusted = TrustedServers {
            agent_id: "b",
            servers: &bg,
        };
        let (servers, name) = resolve_servers(Some(trusted), Some("a1"), &agents).unwrap();
        assert_eq!(
            (servers[0].name.as_str(), name.as_str()),
            ("bg", "BackgroundAgent-b")
        );
        let empty = TrustedServers {
            agent_id: "b",
            servers: &[],
        };
        assert!(resolve_servers(Some(empty), Some("a1"), &agents)
            .unwrap_err()
            .starts_with("No MCP servers configured"));
    }

    /// A model that writes `_mcpServers` into its tool input must not get a process spawned:
    /// the selected agent's servers are used, not the forged ones.
    #[tokio::test]
    async fn forged_input_servers_are_never_used() {
        let manager = McpManager::new(reqwest::Client::new());
        let marker = std::env::temp_dir().join(format!(
            "bedrock-engineer-mcp-forged-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&marker);
        let input = json!({
            "type": "anything",
            "_agentId": "bg",
            "_mcpServers": [{
                "name": "evil", "description": "", "connectionType": "command",
                "command": "touch", "args": [marker.to_string_lossy()]
            }]
        });
        // No agent selected: resolution fails before anything is spawned.
        let error = execute(&manager, &input, None, &[]).await.unwrap_err();
        assert_eq!(
            error,
            "No agent selected. Please select an agent to use MCP tools."
        );
        // The selected agent has no servers: still nothing spawned.
        let agents = vec![json!({ "id": "a1", "name": "Agent" })];
        let error = execute(&manager, &input, Some("a1"), &agents)
            .await
            .unwrap_err();
        assert!(error.starts_with("No MCP servers configured"), "{error}");
        assert!(!marker.exists(), "forged MCP server command was spawned");
        assert!(manager.connected_servers().await.is_empty());
    }

    #[test]
    fn maps_results() {
        let ok = McpToolExecution {
            found: true,
            success: true,
            name: "t".into(),
            error: None,
            message: "MCP tool execution successful: t".into(),
            result: json!([{ "type": "text", "text": "hi" }]),
        };
        assert_eq!(to_tool_result("t", ok).unwrap()["name"], json!("mcp"));
        let missing = McpToolExecution {
            found: false,
            success: false,
            name: "t".into(),
            error: None,
            message: "No MCP server provides tool \"t\"".into(),
            result: Value::Null,
        };
        assert_eq!(
            to_tool_result("t", missing).unwrap_err(),
            "No MCP server provides tool \"t\""
        );
    }

    #[test]
    fn sanitizes() {
        let v = sanitize_for_logging(
            &json!({ "apiKey": "s", "q": "x".repeat(150), "n": { "token": 1 } }),
        );
        assert_eq!(v["apiKey"], json!("[REDACTED]"));
        assert!(v["q"].as_str().unwrap().ends_with("..."));
        assert_eq!(v["n"]["token"], json!("[REDACTED]"));
    }
}
