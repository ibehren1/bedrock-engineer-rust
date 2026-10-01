//! Port of `src/preload/tools/registry.ts` (`ToolRegistry`) and the `executeTool` entry
//! point of `src/preload/tools/index.ts`.

use crate::base::{run_tool, Tool};
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{
    is_mcp_tool, original_mcp_tool_name, ToolCategory, ToolOutput, TOOL_SPEC_ORDER,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// `{ name, category, description }` from `getAllTools()`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub category: ToolCategory,
    pub description: String,
}

/// `getStatistics()`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatistics {
    pub total_tools: usize,
    pub tools_by_category: HashMap<ToolCategory, usize>,
}

/// Registry dispatching tool inputs to tools by name.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    /// Registration order is kept (JS `Map` iteration order).
    tools: Vec<(String, Arc<dyn Tool>)>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// `register(tool, category)`; replaces an existing tool with the same name in place.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        tracing::info!(
            category = tool.category().as_str(),
            "Registered tool: {name}"
        );
        if let Some(slot) = self.tools.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = tool;
        } else {
            self.tools.push((name, tool));
        }
    }

    /// `registerMany`.
    pub fn register_many(&mut self, tools: impl IntoIterator<Item = Arc<dyn Tool>>) {
        for t in tools {
            self.register(t);
        }
    }

    /// `unregister`.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.tools.len();
        self.tools.retain(|(n, _)| n != name);
        before != self.tools.len()
    }

    /// `getTool`.
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
    }

    /// `hasTool`.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|(n, _)| n == name)
    }

    /// `getToolsByCategory`.
    pub fn get_tools_by_category(&self, category: ToolCategory) -> Vec<Arc<dyn Tool>> {
        self.tools
            .iter()
            .filter(|(_, t)| t.category() == category)
            .map(|(_, t)| t.clone())
            .collect()
    }

    /// `getAllTools`.
    pub fn get_all_tools(&self) -> Vec<ToolInfo> {
        self.tools
            .iter()
            .map(|(n, t)| ToolInfo {
                name: n.clone(),
                category: t.category(),
                description: t.description().to_string(),
            })
            .collect()
    }

    /// `getStatistics`.
    pub fn get_statistics(&self) -> ToolStatistics {
        let mut by_cat: HashMap<ToolCategory, usize> =
            ToolCategory::ALL.iter().map(|c| (*c, 0)).collect();
        for (_, t) in &self.tools {
            *by_cat.entry(t.category()).or_default() += 1;
        }
        ToolStatistics {
            total_tools: self.tools.len(),
            tools_by_category: by_cat,
        }
    }

    /// `findTools(pattern)`: case-insensitive regex over tool names.
    pub fn find_tools(&self, pattern: &str) -> Vec<String> {
        let Ok(re) = regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
        else {
            return Vec::new();
        };
        self.tools
            .iter()
            .filter(|(n, _)| re.is_match(n))
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// `clear`.
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    /// `ToolMetadataCollector.getToolSpecs()` for the registered tools, in the TS order
    /// (unknown tools follow in registration order). Each element is
    /// `{ toolSpec: { name, description, inputSchema: { json } } }`.
    pub fn tool_specs(&self) -> Vec<Value> {
        let mut specs: Vec<(usize, Value)> = self
            .tools
            .iter()
            .enumerate()
            .filter_map(|(i, (n, t))| {
                let rank = TOOL_SPEC_ORDER
                    .iter()
                    .position(|o| o == n)
                    .unwrap_or(TOOL_SPEC_ORDER.len() + i);
                t.spec().map(|s| (rank, s.to_bedrock_tool()))
            })
            .collect();
        specs.sort_by_key(|(r, _)| *r);
        specs.into_iter().map(|(_, v)| v).collect()
    }

    /// `ToolRegistry.execute(input, context)`.
    ///
    /// `input` is the tool input object including its `type`. MCP tools (any name that is
    /// not built in, or legacy `mcp_`-prefixed names) route to the tool registered as
    /// `mcp` with `mcpToolName` added, and without the context, like the TS registry.
    pub async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let ty = input
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let tool_name = resolve_tool_name(&ty);

        if input.get("__jsonParseError") == Some(&Value::Bool(true)) {
            return Err(ToolError::validation(
                json_parse_error_message(&input),
                tool_name,
                Some(input),
            ));
        }

        tracing::info!(original_type = %ty, is_mcp = is_mcp_tool(&ty), "Executing tool: {tool_name}");

        let is_mcp = ty.starts_with("mcp_") || is_mcp_tool(&ty);
        let mut input = input;
        let stripped = strip_internal_keys(&mut input, is_mcp);
        if !stripped.is_empty() {
            tracing::warn!(keys = ?stripped, "Ignored internal metadata keys in tool input: {tool_name}");
        }

        let result = if is_mcp {
            let Some(mcp) = self.get_tool("mcp") else {
                return Err(ToolError::not_found("MCP adapter not registered"));
            };
            let mut mcp_input = input;
            if let Some(o) = mcp_input.as_object_mut() {
                o.insert("mcpToolName".into(), json!(original_mcp_tool_name(&ty)));
            }
            let mcp_ctx = ToolContext {
                settings: ctx.settings.clone(),
                caller: ctx.caller.clone(),
                ..Default::default()
            };
            run_tool(mcp.as_ref(), mcp_input, &mcp_ctx).await
        } else {
            let Some(tool) = self.get_tool(tool_name) else {
                return Err(ToolError::not_found(tool_name));
            };
            run_tool(tool.as_ref(), input, ctx).await
        };

        if let Err(e) = &result {
            tracing::error!(error = %e.message, error_type = e.name, "Tool execution failed: {tool_name}");
        }
        result
    }
}

/// Metadata keys the TS app passed through the tool input. The Rust tools read them from
/// [`crate::CallerMetadata`] instead; a model that sets them is ignored.
pub const INTERNAL_INPUT_KEYS: &[&str] = &[
    "_agentId",
    "_sessionId",
    "_mcpServers",
    "_delegationDepth",
    "_delegationLineage",
    "_allowedAgentIds",
    "_modelId",
];

/// Remove caller metadata from a model-authored tool input, returning the removed keys.
///
/// Built-in tools have no `_`-prefixed parameters, so every `_`-prefixed key goes. MCP tools
/// belong to third-party servers whose schemas may use such names, so only
/// [`INTERNAL_INPUT_KEYS`] are removed there.
pub fn strip_internal_keys(input: &mut Value, is_mcp: bool) -> Vec<String> {
    let Some(object) = input.as_object_mut() else {
        return Vec::new();
    };
    let doomed: Vec<String> = object
        .keys()
        .filter(|k| {
            if is_mcp {
                INTERNAL_INPUT_KEYS.contains(&k.as_str())
            } else {
                k.starts_with('_')
            }
        })
        .cloned()
        .collect();
    for key in &doomed {
        object.remove(key);
    }
    doomed
}

/// `resolveToolName`.
fn resolve_tool_name(ty: &str) -> &str {
    if ty.starts_with("mcp_") || is_mcp_tool(ty) {
        "mcp"
    } else {
        ty
    }
}

fn js_display(v: Option<&Value>, fallback: &str) -> String {
    match v {
        None | Some(Value::Null) => fallback.to_string(),
        Some(Value::String(s)) if s.is_empty() => fallback.to_string(),
        Some(Value::Bool(false)) => fallback.to_string(),
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => fallback.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(_)) => "[object Object]".to_string(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(other) => other.to_string(),
    }
}

/// `createJsonParseErrorMessage`.
fn json_parse_error_message(input: &Value) -> String {
    let max_tokens = js_display(input.get("maxTokens"), "unknown");
    let original = js_display(input.get("originalInput"), "unknown");
    let details = js_display(input.get("error"), "JSON parsing failed");
    format!(
        "Tool input JSON parsing failed. This error occurred because the token limit ({max_tokens}) was exceeded while generating the input JSON for tool use.\n\nOriginal input: {original}\n\nError details: {details}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolSpec;
    use async_trait::async_trait;

    struct Echo(&'static str, ToolCategory);

    #[async_trait]
    impl Tool for Echo {
        fn name(&self) -> &str {
            self.0
        }
        fn description(&self) -> &str {
            "echo"
        }
        fn category(&self) -> ToolCategory {
            self.1
        }
        fn spec(&self) -> Option<ToolSpec> {
            Some(ToolSpec::new(self.0, "echo", json!({"type": "object"})))
        }
        fn validate_input(&self, input: &Value) -> Vec<String> {
            if input.get("bad").is_some() {
                vec!["bad".into()]
            } else {
                vec![]
            }
        }
        async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::Json(input))
        }
    }

    fn registry() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register(Arc::new(Echo("think", ToolCategory::Thinking)));
        r.register(Arc::new(Echo("createFolder", ToolCategory::Filesystem)));
        r
    }

    #[tokio::test]
    async fn dispatches_by_type() {
        let r = registry();
        let out = r
            .execute(json!({"type": "think", "x": 1}), &ToolContext::default())
            .await
            .unwrap();
        assert_eq!(out, ToolOutput::Json(json!({"type": "think", "x": 1})));
    }

    #[tokio::test]
    async fn unknown_builtin_is_not_found() {
        let err = registry()
            .execute(json!({"type": "readFiles"}), &ToolContext::default())
            .await
            .unwrap_err();
        assert_eq!(
            err.to_js_string(),
            "ToolNotFoundError: Tool not found: readFiles"
        );
    }

    #[tokio::test]
    async fn mcp_routing_without_adapter() {
        let err = registry()
            .execute(json!({"type": "someServerTool"}), &ToolContext::default())
            .await
            .unwrap_err();
        assert_eq!(err.message, "Tool not found: MCP adapter not registered");
    }

    #[tokio::test]
    async fn strips_model_supplied_metadata_from_builtin_inputs() {
        let out = registry()
            .execute(
                json!({"type": "think", "x": 1, "_agentId": "evil", "_sessionId": "s",
                       "_mcpServers": [{"name": "x", "description": "", "command": "sh"}],
                       "_delegationDepth": 9, "_anythingElse": true}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(out, ToolOutput::Json(json!({"type": "think", "x": 1})));
    }

    #[tokio::test]
    async fn strips_only_reserved_metadata_from_mcp_inputs() {
        let mut r = registry();
        r.register(Arc::new(Echo("mcp", ToolCategory::Mcp)));
        let out = r
            .execute(
                json!({"type": "search", "_mcpServers": [], "_agentId": "evil", "_cursor": "c"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        // A server's own `_`-prefixed parameters still reach it.
        assert_eq!(
            out.as_json().unwrap(),
            &json!({"type": "search", "_cursor": "c", "mcpToolName": "search"})
        );
    }

    #[tokio::test]
    async fn mcp_routing_adds_original_name() {
        let mut r = registry();
        r.register(Arc::new(Echo("mcp", ToolCategory::Mcp)));
        let out = r
            .execute(json!({"type": "mcp_search"}), &ToolContext::default())
            .await
            .unwrap();
        assert_eq!(out.as_json().unwrap()["mcpToolName"], "search");
    }

    #[tokio::test]
    async fn json_parse_error_is_validation_error() {
        let err = registry()
            .execute(
                json!({"type": "think", "__jsonParseError": true, "maxTokens": 4096, "originalInput": "{\"a\":", "error": "Unexpected end"}),
                &ToolContext::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.name, "ValidationError");
        assert!(err.message.starts_with("Tool input JSON parsing failed. This error occurred because the token limit (4096) was exceeded"));
        assert!(err
            .message
            .ends_with("Original input: {\"a\":\n\nError details: Unexpected end"));
    }

    #[tokio::test]
    async fn validation_failure_is_wrapped_as_response_string() {
        let err = registry()
            .execute(json!({"type": "think", "bad": 1}), &ToolContext::default())
            .await
            .unwrap_err();
        assert_eq!(err.name, "Error");
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(v["error"], "Invalid input: bad");
        assert_eq!(v["type"], "VALIDATION");
        assert_eq!(v["toolName"], "think");
    }

    #[test]
    fn listing_and_specs() {
        let mut r = registry();
        assert!(r.has_tool("think"));
        assert_eq!(r.get_tools_by_category(ToolCategory::Filesystem).len(), 1);
        assert_eq!(r.get_statistics().total_tools, 2);
        assert_eq!(r.get_statistics().tools_by_category[&ToolCategory::Web], 0);
        assert_eq!(r.find_tools("FOLDER"), vec!["createFolder"]);
        // Spec order follows getToolSpecs(), not registration order.
        let names: Vec<_> = r
            .tool_specs()
            .iter()
            .map(|s| s["toolSpec"]["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["createFolder", "think"]);
        assert!(r.unregister("think"));
        assert!(!r.unregister("think"));
        r.clear();
        assert!(r.get_all_tools().is_empty());
    }
}
