//! The `codeInterpreter` tool: `CodeInterpreterTool.ts`'s `BaseTool` half over
//! [`docker::interpreter::CodeInterpreter`], which owns the operations (Docker execution,
//! workspace files, async tasks).

use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use async_trait::async_trait;
use docker::interpreter::{tool as spec_source, CodeInterpreter, TOOL_DESCRIPTION, TOOL_NAME};
use serde_json::Value;
use std::sync::Arc;

pub struct CodeInterpreterTool {
    interpreter: Arc<CodeInterpreter>,
}

impl CodeInterpreterTool {
    pub fn new(interpreter: Arc<CodeInterpreter>) -> Self {
        Self { interpreter }
    }

    /// `CodeInterpreterTool.toolSpec`.
    pub fn tool_spec() -> ToolSpec {
        let spec = spec_source::tool_spec();
        ToolSpec::new(
            TOOL_NAME,
            TOOL_DESCRIPTION,
            spec["inputSchema"]["json"].clone(),
        )
    }
}

#[async_trait]
impl Tool for CodeInterpreterTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }
    fn description(&self) -> &str {
        TOOL_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Interpreter
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(Self::tool_spec())
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        CodeInterpreter::validate_input(input).errors
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let output = self
            .interpreter
            .execute(input)
            .await
            .map_err(|e| ToolError::plain(e.to_string()))?;
        serde_json::to_value(&output)
            .map(ToolOutput::Json)
            .map_err(|e| ToolError::plain(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use docker::runner::{BoxFuture, CommandRunner, RunOptions, RunResult};
    use serde_json::json;

    /// A `docker` CLI that is not installed.
    struct NoDocker;

    impl CommandRunner for NoDocker {
        fn run<'a>(
            &'a self,
            _program: &'a str,
            _args: &'a [String],
            _options: RunOptions,
        ) -> BoxFuture<'a, RunResult> {
            Box::pin(async { RunResult::failed(127, "docker: command not found") })
        }
    }

    fn tool() -> CodeInterpreterTool {
        let settings: Arc<dyn docker::Settings> = Arc::new(|_: &str| None::<Value>);
        CodeInterpreterTool::new(CodeInterpreter::with_runner(
            settings,
            Arc::new(docker::interpreter::RecordingLogger::default()),
            Arc::new(NoDocker),
        ))
    }

    #[test]
    fn spec_matches_ts() {
        let spec = CodeInterpreterTool::tool_spec().to_bedrock_tool();
        assert_eq!(spec["toolSpec"]["name"], "codeInterpreter");
        let schema = &spec["toolSpec"]["inputSchema"]["json"];
        assert_eq!(schema["required"], json!([]));
        assert_eq!(
            schema["properties"]["operation"]["enum"],
            json!(["execute", "status", "cancel", "list"])
        );
    }

    #[tokio::test]
    async fn validation_goes_through_base_tool() {
        let err = run_tool(
            &tool(),
            json!({"type": "codeInterpreter", "operation": "status"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(
            v["error"],
            "Invalid input: taskId is required for status and cancel operations"
        );
        assert_eq!(v["type"], "VALIDATION");
    }

    #[tokio::test]
    async fn operations_return_ts_shapes() {
        let t = tool();
        let v = run_tool(
            &t,
            json!({"type": "codeInterpreter", "operation": "list"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["name"], "codeInterpreter");
        assert_eq!(v["operation"], "list");
        assert_eq!(v["message"], "Found 0 total tasks");

        // Docker missing: the sync execution reports failure in the result, not as an error.
        let v = run_tool(
            &t,
            json!({"type": "codeInterpreter", "code": "print(1)"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["success"], false);
        assert_eq!(v["code"], "print(1)");
        assert!(v["error"]
            .as_str()
            .unwrap()
            .starts_with("Docker is not available"));

        let v = run_tool(
            &t,
            json!({"type": "codeInterpreter", "operation": "status", "taskId": "nope"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["message"], "Task not found");
    }
}
