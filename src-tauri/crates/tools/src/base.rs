//! Port of `src/preload/tools/base/BaseTool.ts`.
//!
//! [`Tool`] is the per-tool interface (the abstract members of `BaseTool`); [`run_tool`]
//! is the template method `BaseTool.execute`: validate → execute → chunk oversized results
//! → on failure, `handleError`.

use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::chunker;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Instant;

/// A tool callable by the model.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (the `type` field of tool inputs).
    fn name(&self) -> &str;

    /// Short description (`tool.description`, used in registry listings).
    fn description(&self) -> &str;

    /// Registry category.
    fn category(&self) -> ToolCategory;

    /// Bedrock tool specification (`static toolSpec`). `None` for tools, like the MCP
    /// adapter, that don't publish one.
    fn spec(&self) -> Option<ToolSpec>;

    /// `validateInput`: returns the list of validation errors (empty when valid).
    fn validate_input(&self, input: &Value) -> Vec<String>;

    /// `executeInternal`.
    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;

    /// `handleError`. The default is `BaseTool.handleError` with
    /// `shouldReturnErrorAsString() === true`: wrap, then rethrow a plain `Error` whose
    /// message is `toolError.toResponse()`.
    fn handle_error(&self, error: ToolError) -> ToolError {
        default_handle_error(error, self.name())
    }

    /// `shouldUseChunking`.
    fn use_chunking(&self) -> bool {
        true
    }
}

/// `BaseTool.handleError` with `shouldReturnErrorAsString() === true`.
pub fn default_handle_error(error: ToolError, tool_name: &str) -> ToolError {
    ToolError::plain(error.wrap(tool_name).to_response())
}

/// `BaseTool.handleError` with `shouldReturnErrorAsString() === false`.
pub fn raw_handle_error(error: ToolError, tool_name: &str) -> ToolError {
    error.wrap(tool_name)
}

/// `BaseTool.execute`.
pub async fn run_tool(tool: &dyn Tool, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let start = Instant::now();
    let name = tool.name().to_string();
    tracing::info!(tool = %name, "Executing tool: {name}");

    let result = async {
        let errors = tool.validate_input(&input);
        if !errors.is_empty() {
            return Err(ToolError::validation(
                format!("Invalid input: {}", errors.join(", ")),
                &name,
                Some(input.clone()),
            ));
        }
        let out = tool.execute_internal(input, ctx).await?;
        Ok(if tool.use_chunking() {
            process_result_for_model(out, ctx)
        } else {
            out
        })
    }
    .await;

    let duration = start.elapsed().as_millis();
    match result {
        Ok(out) => {
            tracing::info!(tool = %name, duration, "Tool execution successful: {name}");
            Ok(out)
        }
        Err(e) => {
            tracing::error!(tool = %name, duration, error = %e.message, "Tool execution failed: {name}");
            Err(tool.handle_error(e))
        }
    }
}

/// `BaseTool.processResultForModel`: results larger than `inferenceParams.maxTokens`
/// (estimated at 4 chars/token) are replaced by their first chunk, as a string.
pub fn process_result_for_model(out: ToolOutput, ctx: &ToolContext) -> ToolOutput {
    let serialized = match &out {
        ToolOutput::Text(s) => s.clone(),
        ToolOutput::Json(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
    };
    match chunker::truncate_for_model(&serialized, ctx.settings.max_tokens_or_default()) {
        Some(truncated) => ToolOutput::Text(truncated),
        None => out,
    }
}
