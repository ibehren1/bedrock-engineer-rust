//! Tool execution for the renderer's chat loop: `api.bedrock.executeTool` →
//! `bedrock_execute_tool` (the preload's `executeTool`) and `api.tools.getToolSpecs` →
//! `tools_get_tool_specs` (`ToolMetadataCollector.getToolSpecs()`), over the full registry
//! (filesystem, web, thinking, command, MCP, Bedrock, code interpreter, Docker sandbox, todo,
//! invokeAgent).

use crate::backend::Backend;
use crate::errors;
use crate::state::{store_all, StoreMutex};
use serde_json::Value;
use tauri::State;
use tools::{CallerMetadata, ToolOutput};

/// `context.sessionId` from `executeTool(input, { sessionId })`.
fn session_id(context: Option<&Value>) -> Option<String> {
    context
        .and_then(|c| c.get("sessionId"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Caller metadata from the chat loop's `context` argument (`agentId`, `delegationDepth`,
/// `delegationLineage`, `allowedAgentIds`, `modelId`), which the model cannot write.
///
/// The model-authored `toolInput` is never read for this: the registry strips its
/// `_`-prefixed keys. MCP servers are never taken from the renderer's context either — the
/// MCP adapter resolves them from the selected agent in the store.
fn caller_metadata(context: Option<&Value>) -> CallerMetadata {
    let field = |key: &str| context.and_then(|c| c.get(key));
    let text = |key: &str| {
        field(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let list = |key: &str| {
        field(key).and_then(Value::as_array).map(|items| {
            items
                .iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
    };
    CallerMetadata {
        agent_id: text("agentId"),
        mcp_servers: None,
        delegation_depth: field("delegationDepth")
            .and_then(Value::as_u64)
            .map(|d| u32::try_from(d).unwrap_or(u32::MAX)),
        delegation_lineage: list("delegationLineage"),
        allowed_agent_ids: list("allowedAgentIds"),
        model_id: text("modelId"),
    }
}

async fn execute(
    backend: &Backend,
    store: &StoreMutex,
    tool_input: Value,
    context: Option<Value>,
) -> Result<Value, String> {
    let mut ctx = backend
        .services
        .context(&store_all(store), session_id(context.as_ref()));
    ctx.caller = caller_metadata(context.as_ref());
    backend
        .registry
        .execute(tool_input, &ctx)
        .await
        .map(ToolOutput::into_value)
        .map_err(errors::tool)
}

/// Resolves with the tool's string or `ToolResult` object; rejects with `{ name, message }`.
#[tauri::command]
pub async fn bedrock_execute_tool(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    tool_input: Value,
    context: Option<Value>,
) -> Result<Value, String> {
    execute(&backend, &store, tool_input, context).await
}

/// Same as [`bedrock_execute_tool`], under the `tools_*` name.
#[tauri::command]
pub async fn tools_execute(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    tool_input: Value,
    context: Option<Value>,
) -> Result<Value, String> {
    execute(&backend, &store, tool_input, context).await
}

/// `[{ toolSpec: { name, description, inputSchema: { json } } }]` in the TS spec order.
#[tauri::command]
pub fn tools_get_tool_specs(backend: State<'_, Backend>) -> Vec<Value> {
    backend.registry.tool_specs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_id_from_context() {
        assert_eq!(
            session_id(Some(&json!({ "sessionId": "s1" }))).as_deref(),
            Some("s1")
        );
        assert_eq!(session_id(Some(&json!({ "sessionId": "" }))), None);
        assert_eq!(session_id(Some(&json!({}))), None);
        assert_eq!(session_id(None), None);
    }

    #[test]
    fn caller_metadata_from_context_only() {
        let meta = caller_metadata(Some(&json!({
            "sessionId": "s1", "agentId": "a1", "delegationDepth": 0,
            "delegationLineage": ["a1"], "allowedAgentIds": ["reviewer"], "modelId": "m",
            // Never honored from the renderer either.
            "mcpServers": [{"name": "x", "description": "", "command": "sh"}]
        })));
        assert_eq!(meta.agent_id.as_deref(), Some("a1"));
        assert_eq!(meta.delegation_depth, Some(0));
        assert_eq!(meta.delegation_lineage, Some(vec!["a1".to_string()]));
        assert_eq!(meta.allowed_agent_ids, Some(vec!["reviewer".to_string()]));
        assert_eq!(meta.model_id.as_deref(), Some("m"));
        assert!(meta.mcp_servers.is_none());
        assert_eq!(caller_metadata(None), CallerMetadata::default());
    }

    /// The renderer spreads the model's tool input into `toolInput`, so a model can write
    /// `_mcpServers` / `_agentId` / `_delegationDepth` there. Through the command path those
    /// keys must not reach the tool, and the context's metadata must.
    #[tokio::test]
    async fn model_metadata_in_tool_input_is_dropped() {
        use std::sync::{Arc, Mutex};
        use tools::{Tool, ToolCategory, ToolContext, ToolRegistry};

        #[derive(Default)]
        struct Probe(Mutex<Vec<(Value, CallerMetadata)>>);
        #[async_trait::async_trait]
        impl Tool for Probe {
            fn name(&self) -> &str {
                "think"
            }
            fn description(&self) -> &str {
                "probe"
            }
            fn category(&self) -> ToolCategory {
                ToolCategory::Thinking
            }
            fn spec(&self) -> Option<tools::ToolSpec> {
                None
            }
            fn validate_input(&self, _input: &Value) -> Vec<String> {
                vec![]
            }
            async fn execute_internal(
                &self,
                input: Value,
                ctx: &ToolContext,
            ) -> tools::Result<ToolOutput> {
                self.0.lock().unwrap().push((input, ctx.caller.clone()));
                Ok(ToolOutput::Text("ok".into()))
            }
        }

        let probe = Arc::new(Probe::default());
        let mut registry = ToolRegistry::new();
        registry.register(probe.clone());
        let context = json!({"sessionId": "s1", "agentId": "a1"});
        let mut ctx = ToolContext::from_store(&json!({}), session_id(Some(&context)));
        ctx.caller = caller_metadata(Some(&context));
        registry
            .execute(
                json!({"type": "think", "thought": "x", "_agentId": "admin",
                       "_mcpServers": [{"name": "e", "description": "", "command": "sh"}],
                       "_delegationDepth": 0}),
                &ctx,
            )
            .await
            .unwrap();
        let (input, caller) = probe.0.lock().unwrap()[0].clone();
        assert_eq!(input, json!({"type": "think", "thought": "x"}));
        assert_eq!(caller.agent_id.as_deref(), Some("a1"));
        assert!(caller.mcp_servers.is_none());
    }
}
