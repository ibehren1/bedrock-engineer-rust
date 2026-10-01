//! Tool configuration for Rust-side agent runs — `generateToolSpecs` of
//! `BackgroundAgentService.ts` and `MainToolSpecProvider.ts`.
//!
//! The TS main process asked the renderer's preload for `ToolMetadataCollector.getToolSpecs()`
//! over IPC; here the static specs are [`tools::ToolRegistry::tool_specs`]. MCP specs come from
//! a [`McpToolSpecProvider`] the app implements with the `mcp` crate.

use crate::prompt::replace_agent_placeholders;
use async_trait::async_trait;
use common::delegation::{
    build_invoke_agent_tool_spec, can_delegate, filter_delegation_targets, DelegationContext,
    DelegationTarget, MAX_DELEGATION_DEPTH,
};
use serde_json::{json, Value};
use tools::agent::INVOKE_AGENT_TOOL_NAME;

/// `MainToolSpecProvider.getMcpToolSpecs(mcpServers)` (`getMcpToolSpecs` in `src/main/mcp`).
#[async_trait]
pub trait McpToolSpecProvider: Send + Sync {
    /// Tool specs (`[{ toolSpec: { name, description, inputSchema } }]`) of the agent's MCP
    /// servers (`agent.mcpServers`, as stored). Errors are logged and yield no MCP tools.
    async fn tool_specs(&self, mcp_servers: &Value) -> Result<Vec<Value>, String>;
}

fn tool_names(agent: &Value) -> Vec<String> {
    agent
        .get("tools")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn with_description(mut spec: Value, agent: &Value, working_directory: &str) -> Value {
    let description = spec
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(o) = spec.as_object_mut() {
        o.insert(
            "description".into(),
            Value::String(replace_agent_placeholders(
                &description,
                agent,
                working_directory,
            )),
        );
    }
    spec
}

/// The delegation targets `generateToolSpecs` resolves for `invokeAgent`, or `None` when the tool
/// must be stripped (depth limit reached, nothing permitted, or no permitted agent exists).
pub fn delegation_targets(
    agent: &Value,
    delegation: &DelegationContext,
    all_agents: impl FnOnce() -> Vec<Value>,
) -> Option<Vec<DelegationTarget>> {
    let agent_id = agent.get("id").and_then(Value::as_str);
    let remaining =
        filter_delegation_targets(&delegation.allowed_agent_ids, &delegation.lineage, agent_id);
    if !can_delegate(delegation.depth, remaining.len()) {
        tracing::debug!(
            agent_id = ?agent_id,
            depth = delegation.depth,
            max_depth = MAX_DELEGATION_DEPTH,
            remaining_target_count = remaining.len(),
            "Stripped invokeAgent from sub-agent tools"
        );
        return None;
    }
    let all = all_agents();
    let targets: Vec<DelegationTarget> = remaining
        .iter()
        .filter_map(|id| {
            all.iter()
                .find(|a| a.get("id").and_then(Value::as_str) == Some(id))
        })
        .map(|a| DelegationTarget {
            id: a
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            name: a
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            description: a
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect();
    (!targets.is_empty()).then_some(targets)
}

/// `generateToolSpecs(agent.tools, agent, projectDirectory, delegation)`.
///
/// Returns the `toolSpec` objects (`{ name, description, inputSchema }`) in the agent's tool
/// order, followed by the MCP tool specs when the agent has `mcpServers`. Descriptions have
/// their placeholders replaced. `static_specs` is the registry's
/// `[{ toolSpec }]` list; `all_agents` is only called when `invokeAgent` survives the depth
/// check.
pub async fn generate_tool_specs(
    agent: &Value,
    static_specs: &[Value],
    mcp: Option<&dyn McpToolSpecProvider>,
    delegation: &DelegationContext,
    working_directory: &str,
    all_agents: impl FnOnce() -> Vec<Value>,
) -> Vec<Value> {
    let requested = tool_names(agent);
    let mut effective = requested.clone();
    let mut targets = Vec::new();

    if requested.iter().any(|n| n == INVOKE_AGENT_TOOL_NAME) {
        match delegation_targets(agent, delegation, all_agents) {
            Some(t) => targets = t,
            None => effective.retain(|n| n != INVOKE_AGENT_TOOL_NAME),
        }
    }

    let mut specs = Vec::new();
    for name in &effective {
        let found = static_specs
            .iter()
            .filter_map(|s| s.get("toolSpec"))
            .find(|s| s.get("name").and_then(Value::as_str) == Some(name.as_str()));
        match found {
            Some(spec) => {
                let resolved = if name == INVOKE_AGENT_TOOL_NAME {
                    build_invoke_agent_tool_spec(Some(spec), &targets)
                } else {
                    Some(spec.clone())
                };
                if let Some(resolved) = resolved {
                    specs.push(with_description(resolved, agent, working_directory));
                    tracing::debug!(tool_name = %name, "Found static tool spec");
                }
            }
            None => {
                tracing::warn!(tool_name = %name, "Static tool spec not found, generating basic spec");
                specs.push(json!({
                    "name": name,
                    "description": replace_agent_placeholders(&format!("{name} tool"), agent, working_directory),
                    "inputSchema": { "json": { "type": "object", "properties": {}, "required": [] } }
                }));
            }
        }
    }

    if let (Some(servers), Some(mcp)) = (
        agent
            .get("mcpServers")
            .filter(|s| s.as_array().is_some_and(|a| !a.is_empty())),
        mcp,
    ) {
        match mcp.tool_specs(servers).await {
            Ok(mcp_specs) => {
                for spec in mcp_specs {
                    if let Some(s) = spec.get("toolSpec").filter(|s| !s.is_null()) {
                        specs.push(with_description(s.clone(), agent, working_directory));
                    }
                }
            }
            Err(error) => tracing::error!(%error, "Failed to fetch MCP tool specs"),
        }
    }

    tracing::info!(
        static_tools_requested = effective.len(),
        total_generated_count = specs.len(),
        "Generated tool specs from static and MCP tools"
    );
    specs
}

/// `{ tools: [{ toolSpec }, ...] }`, or `None` when there are no tools.
pub fn tool_config(specs: &[Value]) -> Option<Value> {
    if specs.is_empty() {
        return None;
    }
    Some(json!({
        "tools": specs.iter().map(|s| json!({ "toolSpec": s })).collect::<Vec<_>>()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_specs() -> Vec<Value> {
        let mut r = tools::ToolRegistry::new();
        r.register_many(tools::thinking::create_thinking_tools());
        r.register_many(tools::agent::create_agent_tools());
        r.tool_specs()
    }

    fn names(specs: &[Value]) -> Vec<String> {
        specs
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn delegation(depth: u32, lineage: &[&str], allowed: &[&str]) -> DelegationContext {
        DelegationContext {
            depth,
            lineage: lineage.iter().map(|s| s.to_string()).collect(),
            allowed_agent_ids: allowed.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn others() -> Vec<Value> {
        vec![json!({"id": "writer", "name": "Writer", "description": "Writes"})]
    }

    struct Mcp;
    #[async_trait]
    impl McpToolSpecProvider for Mcp {
        async fn tool_specs(&self, servers: &Value) -> Result<Vec<Value>, String> {
            assert_eq!(servers[0]["name"], "srv");
            Ok(vec![
                json!({"toolSpec": {"name": "search", "description": "in {{projectPath}}", "inputSchema": {"json": {}}}}),
            ])
        }
    }

    #[tokio::test]
    async fn keeps_agent_tool_order_and_fills_unknown_tools_with_a_basic_spec() {
        let agent = json!({"id": "me", "tools": ["unknownTool", "think"]});
        let specs = generate_tool_specs(
            &agent,
            &registry_specs(),
            None,
            &DelegationContext::default(),
            "/p",
            Vec::new,
        )
        .await;
        assert_eq!(names(&specs), vec!["unknownTool", "think"]);
        assert_eq!(specs[0]["description"], "unknownTool tool");
        assert_eq!(specs[0]["inputSchema"]["json"]["required"], json!([]));
    }

    #[tokio::test]
    async fn invoke_agent_is_narrowed_to_the_permitted_targets() {
        let agent = json!({"id": "me", "tools": ["invokeAgent"]});
        let specs = generate_tool_specs(
            &agent,
            &registry_specs(),
            None,
            &delegation(1, &["root", "me"], &["writer", "root"]),
            "/p",
            others,
        )
        .await;
        assert_eq!(names(&specs), vec!["invokeAgent"]);
        assert_eq!(
            specs[0]["inputSchema"]["json"]["properties"]["agentId"]["enum"],
            json!(["writer"])
        );
        assert!(specs[0]["description"]
            .as_str()
            .unwrap()
            .contains("- `writer` — **Writer** — Writes"));
    }

    #[tokio::test]
    async fn invoke_agent_is_stripped_at_the_depth_limit_or_without_targets() {
        let agent = json!({"id": "me", "tools": ["invokeAgent", "think"]});
        for d in [
            delegation(MAX_DELEGATION_DEPTH, &[], &["writer"]),
            delegation(0, &[], &[]),
            delegation(0, &[], &["ghost"]),
        ] {
            let specs =
                generate_tool_specs(&agent, &registry_specs(), None, &d, "/p", others).await;
            assert_eq!(names(&specs), vec!["think"]);
        }
    }

    #[tokio::test]
    async fn appends_mcp_specs_with_placeholders_replaced() {
        let agent = json!({"id": "me", "tools": [], "mcpServers": [{"name": "srv"}]});
        let specs = generate_tool_specs(
            &agent,
            &registry_specs(),
            Some(&Mcp),
            &DelegationContext::default(),
            "/p",
            Vec::new,
        )
        .await;
        assert_eq!(names(&specs), vec!["search"]);
        assert_eq!(specs[0]["description"], "in /p");
        let cfg = tool_config(&specs).unwrap();
        assert_eq!(cfg["tools"][0]["toolSpec"]["name"], "search");
        assert!(tool_config(&[]).is_none());
    }
}
