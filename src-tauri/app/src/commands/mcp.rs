//! `window.api.mcp.*` (port of `src/main/handlers/mcp-handlers.ts`). Every command returns the
//! handler's `{ success, error?, ... }` envelope; the bridge unwraps it like the preload did.

use crate::backend::Backend;
use crate::settings::http_client;
use crate::state::{store_all, StoreMutex};
use common::agent::McpServerConfig;
use serde_json::{json, Value};
use tauri::State;

/// `McpServerConfig[]` from renderer / stored agent JSON. `null` is no servers; a missing
/// `description` (optional in the TS type) is filled in as empty.
pub fn parse_servers(value: &Value) -> Result<Vec<McpServerConfig>, String> {
    let Some(items) = value.as_array() else {
        return if value.is_null() {
            Ok(Vec::new())
        } else {
            Err("mcpServers must be an array".into())
        };
    };
    items.iter().map(parse_server).collect()
}

fn parse_server(value: &Value) -> Result<McpServerConfig, String> {
    let mut v = value.clone();
    if let Some(o) = v.as_object_mut() {
        if !o.get("description").is_some_and(Value::is_string) {
            o.insert("description".into(), json!(""));
        }
    }
    serde_json::from_value(v).map_err(|e| format!("Invalid MCP server configuration: {e}"))
}

fn failure(e: impl std::fmt::Display, extra: Value) -> Value {
    let mut out = json!({ "success": false, "error": e.to_string() });
    if let (Some(o), Some(x)) = (out.as_object_mut(), extra.as_object()) {
        o.extend(x.clone());
    }
    out
}

/// `mcp:init` → `{ success }`.
#[tauri::command]
pub async fn mcp_init(backend: State<'_, Backend>, mcp_servers: Value) -> Result<Value, String> {
    let result = async {
        let servers = parse_servers(&mcp_servers)?;
        tracing::info!(count = servers.len(), "mcp:init");
        backend
            .mcp
            .init_from_agent_config(&servers)
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    Ok(match result {
        Ok(()) => json!({ "success": true }),
        Err(e) => failure(e, json!({})),
    })
}

/// `mcp:getTools` → `{ success, tools }`.
#[tauri::command]
pub async fn mcp_get_tools(
    backend: State<'_, Backend>,
    mcp_servers: Value,
) -> Result<Value, String> {
    let result = async {
        let servers = parse_servers(&mcp_servers)?;
        backend
            .mcp
            .get_tool_specs(Some(&servers))
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    Ok(match result {
        Ok(tools) => json!({ "success": true, "tools": tools }),
        Err(e) => failure(e, json!({ "tools": [] })),
    })
}

/// `mcp:executeTool` → `{ found, success, name, error?, message, result }`.
#[tauri::command]
pub async fn mcp_execute_tool(
    backend: State<'_, Backend>,
    tool_name: String,
    input: Option<Value>,
    mcp_servers: Value,
) -> Result<Value, String> {
    let input = input.unwrap_or(Value::Null);
    let result = match parse_servers(&mcp_servers) {
        Ok(servers) => serde_json::to_value(
            backend
                .mcp
                .execute_tool(&tool_name, &input, Some(&servers))
                .await,
        )
        .map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    Ok(result.unwrap_or_else(|e| {
        json!({
            "found": false,
            "success": false,
            "name": tool_name,
            "error": e,
            "message": format!("Error executing MCP tool \"{tool_name}\": {e}"),
            "result": null
        })
    }))
}

/// `mcp:testConnection` → `{ success, message, details? }`.
#[tauri::command]
pub async fn mcp_test_connection(
    backend: State<'_, Backend>,
    mcp_server: Value,
) -> Result<Value, String> {
    let name = mcp_server
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    match parse_server(&mcp_server) {
        Ok(server) => serde_json::to_value(backend.mcp.test_connection(&server).await)
            .map_err(|e| e.to_string()),
        Err(e) => Ok(json!({
            "success": false,
            "message": format!("Failed to test MCP server \"{name}\": {e}"),
            "details": {
                "error": e,
                "errorDetails": "An unexpected error occurred during connection testing"
            }
        })),
    }
}

/// `mcp:testAllConnections` → `{ success, results }` keyed by server name.
#[tauri::command]
pub async fn mcp_test_all_connections(
    backend: State<'_, Backend>,
    mcp_servers: Value,
) -> Result<Value, String> {
    Ok(match parse_servers(&mcp_servers) {
        Ok(servers) => {
            let results = backend.mcp.test_all_connections(&servers).await;
            json!({ "success": true, "results": results })
        }
        Err(e) => failure(e, json!({ "results": {} })),
    })
}

/// `mcp:searchRegistry` → `{ success, servers }` (through the configured proxy).
#[tauri::command]
pub async fn mcp_search_registry(
    store: State<'_, StoreMutex>,
    query: String,
    limit: Option<u32>,
) -> Result<Value, String> {
    let client = http_client(&store_all(&store));
    Ok(
        match mcp::registry::search_mcp_registry(&client, &query, limit.unwrap_or(10)).await {
            Ok(servers) => json!({ "success": true, "servers": servers }),
            Err(e) => failure(e, json!({ "servers": [] })),
        },
    )
}

/// `mcp:cleanup` → `{ success: true }`.
#[tauri::command]
pub async fn mcp_cleanup(backend: State<'_, Backend>) -> Result<Value, String> {
    backend.mcp.cleanup().await;
    Ok(json!({ "success": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_renderer_server_configs() {
        let servers = parse_servers(&json!([
            { "name": "fs", "command": "npx", "args": ["-y", "server"], "env": { "A": "1" } },
            { "name": "web", "description": "d", "url": "https://example.com/mcp", "connectionType": "url" }
        ]))
        .unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].description, "");
        assert_eq!(servers[0].command.as_deref(), Some("npx"));
        assert_eq!(servers[1].url.as_deref(), Some("https://example.com/mcp"));
        assert!(parse_servers(&Value::Null).unwrap().is_empty());
        assert!(parse_servers(&json!("nope")).is_err());
        assert!(parse_servers(&json!([{ "command": "x" }])).is_err());
    }

    #[test]
    fn failure_envelope_merges_payload_defaults() {
        assert_eq!(
            failure("boom", json!({ "tools": [] })),
            json!({ "success": false, "error": "boom", "tools": [] })
        );
    }
}
