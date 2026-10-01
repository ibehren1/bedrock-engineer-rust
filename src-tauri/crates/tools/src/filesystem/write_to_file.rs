use super::{is_str, mkdir_recursive, parent_dir, str_of};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::{json, Value};

const NAME: &str = "writeToFile";
const DESCRIPTION: &str = "Write content to an existing file at the specified path. Use this when you need to add or update content in an existing file. \n !IMPORTANT: Be careful not to exceed the output_tokens limit.\n\nWrite content to files in your project. Always provide complete file content.";

/// `WriteToFileTool`.
pub struct WriteToFileTool;

#[async_trait]
impl Tool for WriteToFileTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Filesystem
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "The path of the file to write to" },
                    "content": { "type": "string", "description": "The content to write to the file" }
                },
                "required": ["path", "content"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        if !truthy(input.get("path")) {
            errors.push("Path is required".to_string());
        }
        if !is_str(input.get("path")) {
            errors.push("Path must be a string".to_string());
        }
        if matches!(input.get("content"), None | Some(Value::Null)) {
            errors.push("Content is required".to_string());
        }
        if !is_str(input.get("content")) {
            errors.push("Content must be a string".to_string());
        }
        errors
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let path = str_of(&input, "path");
        let content = str_of(&input, "content");
        tracing::debug!(content_length = content.len(), "Writing to file: {path}");
        let result = async {
            mkdir_recursive(&parent_dir(path)).await?;
            tokio::fs::write(path, content)
                .await
                .map_err(|e| NodeIoError::new(&e, "open", path, None))
        }
        .await;
        match result {
            Ok(()) => Ok(ToolOutput::Text(format!(
                "Content written to file: {path}\n\n{content}"
            ))),
            Err(e) => Err(ToolError::execution(
                format!("Error writing to file: {e}"),
                NAME,
                Some(e.to_json()),
                None,
            )),
        }
    }
}
