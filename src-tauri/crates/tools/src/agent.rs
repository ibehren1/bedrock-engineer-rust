//! Port of `src/preload/tools/handlers/agent` (`InvokeAgentTool`) and the types of the
//! `sub-agent:invoke` IPC channel (`SubAgentInvokeParams` / `SubAgentInvokeResult`).
//!
//! The tool does not run the sub-agent itself. In TS it called `api.subAgent.invoke` (IPC to
//! the main-process `SubAgentRunner`); here it calls the [`SubAgentInvoker`] plugged into
//! [`ToolContext::sub_agents`]. The `agents` crate implements the trait, so this crate does not
//! depend on it.

use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::validate::Issues;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// Tool name.
pub const INVOKE_AGENT_TOOL_NAME: &str = "invokeAgent";

const DESCRIPTION: &str = "Delegate a self-contained task to another configured agent";

const SPEC_DESCRIPTION: &str = r#"Delegate a self-contained task to another configured agent and wait for its answer.

The sub-agent runs to completion with its own system prompt and its own tool set, then returns its final text. This call blocks until it finishes.

## When to delegate

1. The task needs expertise or a persona that another agent is configured for
2. The task is a large, separable chunk of work with a clear deliverable
3. The user explicitly asked for a specific agent to handle part of the request

## When NOT to delegate

1. You can do the work yourself with the tools you already have — delegation costs extra time and tokens
2. The task needs the full conversation history for context; the sub-agent cannot see it
3. The task is a single tool call

## Writing a good task

The sub-agent starts from nothing. It sees only what you write in `task`, `context`, and `expectedOutput` — not this conversation, not the user's original message, not your earlier tool results. Restate every file path, identifier, constraint, and decision it needs. Describe the outcome you want rather than the steps to take, and let the sub-agent choose its own tools.

## Limits

- Delegation is limited in depth, so a sub-agent may not be able to delegate further
- A sub-agent may only be invoked when the user has permitted it (see the list below, if present)
- If the sub-agent hits its tool budget you receive its partial answer with `stoppedReason: "maxToolExecutions"`"#;

const AGENT_ID_DESCRIPTION: &str = "The id of the agent to delegate to. Must be one of the ids listed in this tool description. Do not guess ids.";
const TASK_DESCRIPTION: &str = "A complete, self-contained instruction for the sub-agent. The sub-agent does NOT see this conversation, so restate every fact, path, and constraint it needs. Describe the outcome you want, not the tools to use.";
const CONTEXT_DESCRIPTION: &str =
    "Optional background: relevant file paths, prior findings, decisions already made.";
const EXPECTED_OUTPUT_DESCRIPTION: &str =
    "Optional description of the shape or format of the answer you want back.";

/// `SubAgentInvokeParams['options']`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_executions: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// `SubAgentInvokeParams` — the `sub-agent:invoke` IPC params (`sub_agent_invoke` command).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentInvokeParams {
    pub agent_id: String,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_agent_id: Option<String>,
    /// Depth of the *caller*. 0 = top-level chat.
    #[serde(default)]
    pub depth: u32,
    /// Agent ids from the root down to and including the caller.
    #[serde(default)]
    pub lineage: Vec<String>,
    #[serde(default)]
    pub allowed_agent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<SubAgentOptions>,
}

/// `InvokeAgentResult['result']['stoppedReason']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoppedReason {
    Completed,
    MaxToolExecutions,
}

/// `SubAgentInvokeResult` (`InvokeAgentResult['result'] & { success, error? }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentInvokeResult {
    pub success: bool,
    pub agent_id: String,
    pub agent_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_icon_color: Option<String>,
    pub task: String,
    pub final_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    pub tool_call_count: usize,
    pub tool_names: Vec<String>,
    pub duration_ms: u64,
    pub depth: u32,
    pub stopped_reason: StoppedReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The `sub-agent:invoke` handler (`SubAgentRunner.invoke`). The error is the message the
/// runner threw (policy violations, timeouts, run failures); it is reported to the model verbatim.
#[async_trait]
pub trait SubAgentInvoker: Send + Sync {
    async fn invoke(
        &self,
        params: SubAgentInvokeParams,
    ) -> std::result::Result<SubAgentInvokeResult, String>;
}

/// `InvokeAgentTool`.
pub struct InvokeAgentTool;

/// `createAgentTools()`.
pub fn create_agent_tools() -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(InvokeAgentTool)]
}

/// JS `Math.round` for non-negative values.
fn js_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// Build the runner params from a tool input, as `InvokeAgentTool.executeInternal` does. The
/// delegation metadata (caller agent, depth, lineage, allowed agents, model) comes from the
/// trusted [`ToolContext::caller`], never from the model-authored input: the TS read it from
/// `_agentId`/`_delegationDepth`/... input keys, which a model could forge.
pub fn params_from_input(input: &Value, ctx: &ToolContext) -> SubAgentInvokeParams {
    let meta = &ctx.caller;
    let caller = meta.agent_id.clone().filter(|s| !s.is_empty());
    let depth = meta.delegation_depth.unwrap_or(0);
    let lineage = meta
        .delegation_lineage
        .clone()
        .unwrap_or_else(|| caller.iter().cloned().collect());
    SubAgentInvokeParams {
        agent_id: input
            .get("agentId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        task: input
            .get("task")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        context: input
            .get("context")
            .and_then(Value::as_str)
            .map(str::to_string),
        expected_output: input
            .get("expectedOutput")
            .and_then(Value::as_str)
            .map(str::to_string),
        caller_agent_id: caller,
        depth,
        lineage,
        allowed_agent_ids: meta.allowed_agent_ids.clone().unwrap_or_default(),
        project_directory: ctx.settings.project_path.clone(),
        model_id: meta.model_id.clone().filter(|s| !s.is_empty()),
        options: None,
    }
}

#[async_trait]
impl Tool for InvokeAgentTool {
    fn name(&self) -> &str {
        INVOKE_AGENT_TOOL_NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Agent
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            INVOKE_AGENT_TOOL_NAME,
            SPEC_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": INVOKE_AGENT_TOOL_NAME },
                    "agentId": { "type": "string", "minLength": 1, "description": AGENT_ID_DESCRIPTION },
                    "task": { "type": "string", "minLength": 1, "description": TASK_DESCRIPTION },
                    "context": { "type": "string", "description": CONTEXT_DESCRIPTION },
                    "expectedOutput": { "type": "string", "description": EXPECTED_OUTPUT_DESCRIPTION }
                },
                "required": ["type", "agentId", "task"],
                "additionalProperties": false
            }),
        ))
    }

    /// `invokeAgentInputSchema.parse(input)` (Zod object: unknown keys such as the `_`
    /// metadata are stripped, not rejected).
    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        if input.get("type").and_then(Value::as_str) != Some(INVOKE_AGENT_TOOL_NAME) {
            issues.push(
                &["type"],
                format!("Invalid literal value, expected \"{INVOKE_AGENT_TOOL_NAME}\""),
            );
        }
        for key in ["agentId", "task"] {
            if let Some(s) = issues.string(&[key], input.get(key), false) {
                if s.encode_utf16().count() < 1 {
                    issues.push(&[key], "String must contain at least 1 character(s)");
                }
            }
        }
        for key in ["context", "expectedOutput"] {
            issues.string(&[key], input.get(key), true);
        }
        issues.into_vec()
    }

    /// Throws on hard failures so the caller records the tool result as an error; the
    /// renderer's result mapping treats any returned object as a success.
    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let params = params_from_input(&input, ctx);
        tracing::info!(
            agent_id = %params.agent_id,
            caller_agent_id = ?params.caller_agent_id,
            depth = params.depth,
            "Delegating task to agent"
        );

        let Some(invoker) = ctx.sub_agents.clone() else {
            return Err(ToolError::plain("Sub-agent runner is not available"));
        };
        let agent_id = params.agent_id.clone();
        let result = invoker.invoke(params).await.map_err(ToolError::plain)?;

        tracing::info!(
            agent_id = %agent_id,
            depth = result.depth,
            duration_ms = result.duration_ms,
            tool_call_count = result.tool_call_count,
            stopped_reason = ?result.stopped_reason,
            "Delegation completed"
        );

        let message = format!(
            "Agent \"{}\" completed the task in {}s using {} tool call(s)",
            result.agent_name,
            js_round(result.duration_ms as f64 / 1000.0),
            result.tool_call_count
        );
        Ok(ToolOutput::Json(json!({
            "name": INVOKE_AGENT_TOOL_NAME,
            "success": true,
            "message": message,
            "result": serde_json::to_value(&result).unwrap_or(Value::Null)
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use crate::context::{CallerMetadata, ToolSettings};
    use std::sync::Mutex;

    struct Recorder {
        seen: Mutex<Vec<SubAgentInvokeParams>>,
        fail: Option<String>,
    }

    #[async_trait]
    impl SubAgentInvoker for Recorder {
        async fn invoke(
            &self,
            params: SubAgentInvokeParams,
        ) -> std::result::Result<SubAgentInvokeResult, String> {
            self.seen.lock().unwrap().push(params.clone());
            if let Some(e) = &self.fail {
                return Err(e.clone());
            }
            Ok(SubAgentInvokeResult {
                success: true,
                agent_id: params.agent_id,
                agent_name: "Reviewer".into(),
                agent_icon: None,
                agent_icon_color: None,
                task: params.task,
                final_text: "ok".into(),
                truncated: None,
                tool_call_count: 3,
                tool_names: vec!["readFiles".into()],
                duration_ms: 2500,
                depth: params.depth + 1,
                stopped_reason: StoppedReason::Completed,
                usage: None,
                session_id: "subagent-x".into(),
                error: None,
            })
        }
    }

    fn ctx(invoker: Option<Arc<dyn SubAgentInvoker>>) -> ToolContext {
        let mut c = ToolContext::new(ToolSettings {
            project_path: Some("/proj".into()),
            ..Default::default()
        });
        c.sub_agents = invoker;
        c
    }

    fn recorder(fail: Option<&str>) -> Arc<Recorder> {
        Arc::new(Recorder {
            seen: Mutex::new(Vec::new()),
            fail: fail.map(Into::into),
        })
    }

    #[test]
    fn validation_messages_match_zod() {
        let errors =
            InvokeAgentTool.validate_input(&json!({"type": "x", "agentId": "", "context": 3}));
        assert_eq!(
            errors,
            vec![
                "type: Invalid literal value, expected \"invokeAgent\"",
                "agentId: String must contain at least 1 character(s)",
                "task: Required",
                "context: Expected string, received number"
            ]
        );
        assert!(InvokeAgentTool
            .validate_input(
                &json!({"type": "invokeAgent", "agentId": "a", "task": "t", "_agentId": "c"})
            )
            .is_empty());
    }

    fn caller_ctx(invoker: Arc<dyn SubAgentInvoker>, meta: CallerMetadata) -> ToolContext {
        let mut c = ctx(Some(invoker));
        c.caller = meta;
        c
    }

    #[tokio::test]
    async fn passes_injected_delegation_metadata_to_the_runner() {
        let rec = recorder(None);
        let out = run_tool(
            &InvokeAgentTool,
            json!({
                "type": "invokeAgent", "agentId": "reviewer", "task": "Review", "context": "ctx"
            }),
            &caller_ctx(
                rec.clone(),
                CallerMetadata {
                    agent_id: Some("caller".into()),
                    delegation_depth: Some(1),
                    delegation_lineage: Some(vec!["root".into(), "caller".into()]),
                    allowed_agent_ids: Some(vec!["reviewer".into()]),
                    model_id: Some("m1".into()),
                    ..Default::default()
                },
            ),
        )
        .await
        .unwrap();
        let seen = rec.seen.lock().unwrap()[0].clone();
        assert_eq!(seen.caller_agent_id.as_deref(), Some("caller"));
        assert_eq!(seen.depth, 1);
        assert_eq!(seen.lineage, vec!["root", "caller"]);
        assert_eq!(seen.allowed_agent_ids, vec!["reviewer"]);
        assert_eq!(seen.model_id.as_deref(), Some("m1"));
        assert_eq!(seen.project_directory.as_deref(), Some("/proj"));
        assert_eq!(seen.context.as_deref(), Some("ctx"));

        let v = out.into_value();
        assert_eq!(v["name"], "invokeAgent");
        assert_eq!(v["success"], true);
        assert_eq!(
            v["message"],
            "Agent \"Reviewer\" completed the task in 3s using 3 tool call(s)"
        );
        assert_eq!(v["result"]["finalText"], "ok");
        assert_eq!(v["result"]["stoppedReason"], "completed");
        assert_eq!(v["result"]["depth"], 2);
    }

    #[tokio::test]
    async fn lineage_defaults_to_the_caller() {
        let rec = recorder(None);
        run_tool(
            &InvokeAgentTool,
            json!({"type": "invokeAgent", "agentId": "a", "task": "t"}),
            &caller_ctx(
                rec.clone(),
                CallerMetadata {
                    agent_id: Some("caller".into()),
                    ..Default::default()
                },
            ),
        )
        .await
        .unwrap();
        let seen = rec.seen.lock().unwrap()[0].clone();
        assert_eq!(seen.depth, 0);
        assert_eq!(seen.lineage, vec!["caller"]);
        assert!(seen.allowed_agent_ids.is_empty());
    }

    #[tokio::test]
    async fn model_supplied_delegation_metadata_is_ignored() {
        let rec = recorder(None);
        let mut registry = crate::ToolRegistry::new();
        registry.register(Arc::new(InvokeAgentTool));
        registry
            .execute(
                json!({
                    "type": "invokeAgent", "agentId": "admin", "task": "t",
                    "_agentId": "forged", "_delegationDepth": 0, "_delegationLineage": [],
                    "_allowedAgentIds": ["admin"], "_modelId": "expensive"
                }),
                &caller_ctx(
                    rec.clone(),
                    CallerMetadata {
                        agent_id: Some("caller".into()),
                        delegation_depth: Some(1),
                        allowed_agent_ids: Some(vec!["reviewer".into()]),
                        model_id: Some("m1".into()),
                        ..Default::default()
                    },
                ),
            )
            .await
            .unwrap();
        let seen = rec.seen.lock().unwrap()[0].clone();
        assert_eq!(seen.caller_agent_id.as_deref(), Some("caller"));
        assert_eq!(seen.depth, 1);
        assert_eq!(seen.lineage, vec!["caller"]);
        assert_eq!(seen.allowed_agent_ids, vec!["reviewer"]);
        assert_eq!(seen.model_id.as_deref(), Some("m1"));
    }

    #[tokio::test]
    async fn runner_errors_become_tool_errors() {
        let rec = recorder(Some(
            "Delegation depth limit reached (max 2). Complete this task yourself.",
        ));
        let err = run_tool(
            &InvokeAgentTool,
            json!({"type": "invokeAgent", "agentId": "a", "task": "t"}),
            &ctx(Some(rec)),
        )
        .await
        .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(v["success"], false);
        assert_eq!(v["type"], "EXECUTION");
        assert_eq!(v["toolName"], "invokeAgent");
        assert!(v["error"].as_str().unwrap().contains("depth limit"));
    }

    #[tokio::test]
    async fn missing_runner_is_an_error() {
        let err = run_tool(
            &InvokeAgentTool,
            json!({"type": "invokeAgent", "agentId": "a", "task": "t"}),
            &ctx(None),
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("Sub-agent runner is not available"));
    }

    #[test]
    fn params_round_trip_camel_case() {
        let p: SubAgentInvokeParams = serde_json::from_value(json!({
            "agentId": "a", "task": "t", "depth": 1, "lineage": ["x"], "allowedAgentIds": ["a"],
            "options": {"maxToolExecutions": 2, "timeoutMs": 20}
        }))
        .unwrap();
        assert_eq!(p.options.as_ref().unwrap().max_tool_executions, Some(2));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["allowedAgentIds"], json!(["a"]));
        assert_eq!(v["options"]["timeoutMs"], 20);
    }
}
