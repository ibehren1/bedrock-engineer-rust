use super::{is_str, mkdir_recursive, str_of};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Value};

const NAME: &str = "createFolder";
const DESCRIPTION: &str = "Create a new folder at the specified path. Use this when you need to create a new directory in the project structure.\n\nCreate directories in your project. Use absolute paths starting from {{projectPath}}.";

/// `CreateFolderTool`.
pub struct CreateFolderTool;

#[async_trait]
impl Tool for CreateFolderTool {
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
                    "path": { "type": "string", "description": "The path where the folder should be created" }
                },
                "required": ["path"]
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
        errors
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let path = str_of(&input, "path");
        tracing::debug!("Creating folder: {path}");
        match mkdir_recursive(path).await {
            Ok(()) => Ok(ToolOutput::Text(format!("Folder created: {path}"))),
            Err(e) => Err(ToolError::execution(
                format!("Error creating folder: {e}"),
                NAME,
                Some(e.to_json()),
                None,
            )),
        }
    }
}
