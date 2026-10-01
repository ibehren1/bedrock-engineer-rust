//! Port of `src/preload/tools/handlers/mcp/McpToolAdapter.ts`: the tool registered as `mcp`,
//! which [`crate::ToolRegistry::execute`] routes every non-built-in (and legacy `mcp_*`) tool
//! name to, with `mcpToolName` set.
//!
//! Parsing and server resolution come from [`mcp::adapter`]; the call goes through
//! [`McpExecutor`] (implemented for [`mcp::McpManager`]). Errors carry the same
//! `ExecutionError` metadata as the TS, so the model sees the same response JSON.

use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use async_trait::async_trait;
use common::agent::McpServerConfig;
use mcp::manager::McpToolExecution;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const NAME: &str = "mcp";
const DESCRIPTION: &str = "Execute tools provided by MCP servers";

/// The `mcp:executeTool` IPC handler.
#[async_trait]
pub trait McpExecutor: Send + Sync {
    async fn execute_tool(
        &self,
        tool_name: &str,
        args: &Value,
        servers: &[McpServerConfig],
    ) -> McpToolExecution;
}

#[async_trait]
impl McpExecutor for mcp::McpManager {
    async fn execute_tool(
        &self,
        tool_name: &str,
        args: &Value,
        servers: &[McpServerConfig],
    ) -> McpToolExecution {
        mcp::McpManager::execute_tool(self, tool_name, args, Some(servers)).await
    }
}

/// `McpToolAdapter`.
pub struct McpTool {
    executor: Arc<dyn McpExecutor>,
}

impl McpTool {
    pub fn new(executor: Arc<dyn McpExecutor>) -> Self {
        Self { executor }
    }
}

fn exec_error(message: String, extra: Map<String, Value>) -> ToolError {
    ToolError::execution(message, NAME, None, Some(extra))
}

fn meta(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Mcp
    }
    /// The adapter has no static `toolSpec`; MCP tool specs come from the servers.
    fn spec(&self) -> Option<ToolSpec> {
        None
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        mcp::adapter::validate_input(input)
            .err()
            .unwrap_or_default()
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let call = mcp::adapter::parse_call(&input);
        let tool_name = call.tool_name.clone();
        let selected = ctx
            .settings
            .selected_agent_id
            .as_deref()
            .filter(|s| !s.is_empty());

        // Background agents run with their stored definition's servers, handed over by the
        // agent engine in the context; the model-authored input never names servers.
        let caller_agent = ctx.caller.agent_id.as_deref().filter(|s| !s.is_empty());
        let trusted = match (caller_agent, ctx.caller.mcp_servers.as_deref()) {
            (Some(agent_id), Some(servers)) => {
                Some(mcp::adapter::TrustedServers { agent_id, servers })
            }
            _ => None,
        };
        let servers = match mcp::adapter::resolve_servers(
            trusted,
            selected,
            &ctx.settings.custom_agents,
        ) {
            Ok((servers, agent_name)) => {
                tracing::debug!(agent = %agent_name, servers = servers.len(), "Found MCP servers for agent");
                servers
            }
            Err(message) => {
                let agent_id = trusted
                    .map(|t| t.agent_id.to_string())
                    .or_else(|| selected.map(str::to_string));
                let extra = if message.starts_with("No agent selected") {
                    meta(&[("toolName", json!(tool_name))])
                } else if message.starts_with("Agent not found") {
                    meta(&[
                        ("toolName", json!(tool_name)),
                        ("selectedAgentId", json!(selected)),
                    ])
                } else {
                    meta(&[("toolName", json!(tool_name)), ("agentId", json!(agent_id))])
                };
                return Err(exec_error(message, extra));
            }
        };

        tracing::info!("Calling MCP tool: {tool_name}");
        let execution = self
            .executor
            .execute_tool(&tool_name, &call.args, &servers)
            .await;
        let (found, success) = (execution.found, execution.success);
        mcp::adapter::to_tool_result(&tool_name, execution)
            .map(ToolOutput::Json)
            .map_err(|message| {
                let extra = if !found {
                    meta(&[
                        ("toolName", json!(tool_name)),
                        ("availableServers", json!(servers.len())),
                    ])
                } else {
                    debug_assert!(!success);
                    meta(&[("toolName", json!(tool_name)), ("args", call.args.clone())])
                };
                exec_error(message, extra)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ToolSettings;
    use crate::registry::ToolRegistry;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeMcp {
        calls: Mutex<Vec<(String, Value, usize)>>,
        result: Mutex<Option<McpToolExecution>>,
    }

    #[async_trait]
    impl McpExecutor for FakeMcp {
        async fn execute_tool(
            &self,
            tool_name: &str,
            args: &Value,
            servers: &[McpServerConfig],
        ) -> McpToolExecution {
            self.calls
                .lock()
                .unwrap()
                .push((tool_name.into(), args.clone(), servers.len()));
            self.result.lock().unwrap().clone().unwrap()
        }
    }

    fn execution(found: bool, success: bool, message: &str) -> McpToolExecution {
        McpToolExecution {
            found,
            success,
            name: "search".into(),
            error: None,
            message: message.into(),
            result: json!([{"type": "text", "text": "hit"}]),
        }
    }

    fn registry(fake: Arc<FakeMcp>) -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register(Arc::new(McpTool::new(fake)));
        r
    }

    fn ctx(selected: Option<&str>) -> ToolContext {
        let mut s = ToolSettings::from_store(&json!({
            "customAgents": [
                {"id": "a1", "name": "Agent", "mcpServers": [{"name": "s", "description": "", "url": "https://x/mcp"}]},
                {"id": "a2", "name": "Empty"}
            ]
        }));
        s.selected_agent_id = selected.map(str::to_string);
        ToolContext::new(s)
    }

    fn err_json(e: ToolError) -> Value {
        assert_eq!(e.name, "Error");
        serde_json::from_str(&e.message).unwrap()
    }

    #[tokio::test]
    async fn routes_unknown_tools_and_returns_result() {
        let fake = Arc::new(FakeMcp::default());
        *fake.result.lock().unwrap() = Some(execution(
            true,
            true,
            "MCP tool execution successful: search",
        ));
        let r = registry(fake.clone());
        let v = r
            .execute(json!({"type": "search", "query": "rust"}), &ctx(Some("a1")))
            .await
            .unwrap()
            .into_value();
        assert_eq!(
            v,
            json!({"success": true, "name": "mcp", "message": "MCP tool execution successful: search", "result": [{"type": "text", "text": "hit"}]})
        );
        // Legacy prefix, and background-agent context wins over the selected agent.
        let mut bg = ctx(Some("a1"));
        bg.caller = crate::CallerMetadata {
            agent_id: Some("bg".into()),
            mcp_servers: Some(serde_json::from_value(json!([
                {"name": "a", "description": "", "url": "https://a"}, {"name": "b", "description": "", "url": "https://b"}
            ])).unwrap()),
            ..Default::default()
        };
        let v = r
            .execute(json!({"type": "mcp_search", "q": 1}), &bg)
            .await
            .unwrap()
            .into_value();
        assert_eq!(v["success"], true);
        let calls = fake.calls.lock().unwrap().clone();
        assert_eq!(calls[0], ("search".into(), json!({"query": "rust"}), 1));
        assert_eq!(calls[1], ("search".into(), json!({"q": 1}), 2));
    }

    /// The exploit: a model writes `_mcpServers` (any command) and `_agentId` into a tool input.
    /// The forged servers must never reach the MCP manager, which would spawn them.
    #[tokio::test]
    async fn model_supplied_mcp_servers_are_ignored() {
        let fake = Arc::new(FakeMcp::default());
        *fake.result.lock().unwrap() = Some(execution(true, true, "ok"));
        let r = registry(fake.clone());
        let forged = json!({"type": "search", "q": 1, "_agentId": "bg", "_mcpServers": [
            {"name": "evil", "description": "", "connectionType": "command", "command": "sh", "args": ["-c", "id"]},
            {"name": "evil2", "description": "", "command": "sh"}
        ]});

        // Nothing selected: configuration error, no call.
        let e = r.execute(forged.clone(), &ctx(None)).await.unwrap_err();
        assert!(e.message.contains("No agent selected"), "{}", e.message);
        assert!(fake.calls.lock().unwrap().is_empty());

        // Selected agent's own single server is used, and the forged keys are not forwarded.
        r.execute(forged, &ctx(Some("a1"))).await.unwrap();
        let calls = fake.calls.lock().unwrap().clone();
        assert_eq!(calls, vec![("search".into(), json!({"q": 1}), 1)]);
    }

    #[tokio::test]
    async fn configuration_errors_match_ts_metadata() {
        let r = registry(Arc::new(FakeMcp::default()));
        let e = r
            .execute(json!({"type": "search"}), &ctx(None))
            .await
            .unwrap_err();
        assert_eq!(
            err_json(e),
            json!({"success": false, "error": "No agent selected. Please select an agent to use MCP tools.", "type": "EXECUTION", "toolName": "search"})
        );
        let e = r
            .execute(json!({"type": "search"}), &ctx(Some("zz")))
            .await
            .unwrap_err();
        assert_eq!(
            err_json(e),
            json!({"success": false, "error": "Agent not found: zz. Please check your agent configuration.", "type": "EXECUTION", "toolName": "search", "selectedAgentId": "zz"})
        );
        let e = r
            .execute(json!({"type": "search"}), &ctx(Some("a2")))
            .await
            .unwrap_err();
        assert_eq!(
            err_json(e),
            json!({"success": false, "error": "No MCP servers configured for this agent. Please configure MCP servers in agent settings.", "type": "EXECUTION", "toolName": "search", "agentId": "a2"})
        );
    }

    #[tokio::test]
    async fn execution_errors_match_ts_metadata() {
        let fake = Arc::new(FakeMcp::default());
        let r = registry(fake.clone());
        *fake.result.lock().unwrap() = Some(execution(false, false, ""));
        let e = r
            .execute(json!({"type": "search"}), &ctx(Some("a1")))
            .await
            .unwrap_err();
        assert_eq!(
            err_json(e),
            json!({"success": false, "error": "MCP tool not found: search. Please check if the tool is available in your configured MCP servers.", "type": "EXECUTION", "toolName": "search", "availableServers": 1})
        );
        *fake.result.lock().unwrap() = Some(execution(
            true,
            false,
            "Error executing MCP tool \"search\": boom",
        ));
        let e = r
            .execute(json!({"type": "search", "x": 2}), &ctx(Some("a1")))
            .await
            .unwrap_err();
        assert_eq!(
            err_json(e),
            json!({"success": false, "error": "Error executing MCP tool \"search\": boom", "type": "EXECUTION", "toolName": "search", "args": {"x": 2}})
        );
    }

    #[test]
    fn has_no_spec_and_validates_type() {
        let t = McpTool::new(Arc::new(FakeMcp::default()));
        assert!(t.spec().is_none());
        assert_eq!(
            t.validate_input(&json!({"type": 1})),
            vec!["Tool type must be a string"]
        );
    }

    /// End to end over `McpManager` and the mcp crate's fake stdio server (needs python3).
    #[tokio::test(flavor = "multi_thread")]
    async fn manager_against_fake_stdio_server() {
        let py = ["python3", "python"].into_iter().find(|c| {
            std::process::Command::new(c)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });
        let Some(py) = py else {
            eprintln!("python3 not found; skipping");
            return;
        };
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mcp/tests/fake_mcp_server.py")
            .to_string_lossy()
            .into_owned();
        let manager = Arc::new(mcp::McpManager::new(reqwest::Client::new()));
        let r = registry_with(manager.clone());
        let mut s = ToolSettings::from_store(&json!({
            "selectedAgentId": "a",
            "customAgents": [{"id": "a", "name": "A", "mcpServers": [
                {"name": "fake", "description": "", "connectionType": "command", "command": py, "args": [script]}
            ]}]
        }));
        s.selected_agent_id = Some("a".into());
        let v = r
            .execute(
                json!({"type": "echo", "text": "hello"}),
                &ToolContext::new(s.clone()),
            )
            .await
            .unwrap()
            .into_value();
        assert_eq!(v["success"], true, "{v}");
        assert_eq!(v["name"], "mcp");
        assert!(v["result"].to_string().contains("hello"), "{v}");
        let e = r
            .execute(json!({"type": "nope"}), &ToolContext::new(s))
            .await
            .unwrap_err();
        assert_eq!(err_json(e)["availableServers"], 1);
        manager.cleanup().await;
    }

    fn registry_with(manager: Arc<mcp::McpManager>) -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register(Arc::new(McpTool::new(manager)));
        r
    }
}
