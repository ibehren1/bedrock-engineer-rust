//! Port of `src/preload/tools/handlers/thinking/ThinkTool.ts`.

use crate::base::{raw_handle_error, Tool};
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

const NAME: &str = "think";
const DESCRIPTION: &str = "Use the tool to think about something. It will not obtain new information or make any changes to the repository, but just log the thought. Use it when complex reasoning or brainstorming is needed. For example, if you explore the repo and discover the source of a bug, call this tool to brainstorm several unique ways of fixing the bug, and assess which change(s) are likely to be simplest and most effective. Alternatively, if you receive some test results, call this tool to brainstorm ways to fix the failing tests.\n\nProcess complex reasoning and brainstorming. Use for analysis before making changes.\n\nBefore taking any action or responding to the user after receiving tool results, use the think tool as a scratchpad to:\n- List the specific rules that apply to the current request\n- Check if all required information is collected\n- Verify that the planned action complies with all policies\n- Iterate over tool results for correctness";

/// `ThinkTool`: echoes the thought back as `result.reasoning`.
pub struct ThinkTool;

/// `createThinkingTools()`.
pub fn create_thinking_tools() -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(ThinkTool)]
}

#[async_trait]
impl Tool for ThinkTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Thinking
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": { "thought": { "type": "string", "description": "Your thoughts." } },
                "required": ["thought"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let thought = input.get("thought");
        let mut errors = Vec::new();
        if !truthy(thought) {
            errors.push("Thought is required".to_string());
        }
        if !matches!(thought, Some(Value::String(_))) {
            errors.push("Thought must be a string".to_string());
        }
        if truthy(thought) {
            if let Some(s) = thought.and_then(Value::as_str) {
                if s.trim().is_empty() {
                    errors.push("Thought cannot be empty".to_string());
                }
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let thought = input
            .get("thought")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Ok(ToolOutput::Json(json!({
            "success": true,
            "name": NAME,
            "message": "Thinking process completed",
            "result": { "reasoning": thought }
        })))
    }

    /// `shouldReturnErrorAsString() === false`: the ToolError itself is thrown.
    fn handle_error(&self, error: ToolError) -> ToolError {
        raw_handle_error(error, NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;

    #[tokio::test]
    async fn echoes_reasoning() {
        let out = run_tool(
            &ThinkTool,
            json!({"type": "think", "thought": "hmm"}),
            &ToolContext::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            out.into_value(),
            json!({"success": true, "name": "think", "message": "Thinking process completed", "result": {"reasoning": "hmm"}})
        );
    }

    #[tokio::test]
    async fn validation_error_is_thrown_raw() {
        let err = run_tool(
            &ThinkTool,
            json!({"type": "think", "thought": "   "}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err.to_js_string(),
            "ValidationError: Invalid input: Thought cannot be empty"
        );
        let err = run_tool(
            &ThinkTool,
            json!({"type": "think"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err.message,
            "Invalid input: Thought is required, Thought must be a string"
        );
    }

    #[tokio::test]
    async fn large_results_are_chunked() {
        let mut ctx = ToolContext::default();
        ctx.settings.max_tokens = Some(10.0);
        let out = run_tool(
            &ThinkTool,
            json!({"type": "think", "thought": "x".repeat(200)}),
            &ctx,
        )
        .await
        .unwrap();
        let text = out.as_text().expect("chunked results become strings");
        assert!(text.starts_with("{\n  \"success\": true,"));
        assert!(text.contains("[Content truncated due to token limit. This is chunk 1 of "));
    }
}
