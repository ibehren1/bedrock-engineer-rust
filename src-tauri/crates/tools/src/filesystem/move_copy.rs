use super::{is_str, mkdir_recursive, parent_dir, str_of};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::{json, Map, Value};

fn validate(input: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if !truthy(input.get("source")) {
        errors.push("Source path is required".to_string());
    }
    if !is_str(input.get("source")) {
        errors.push("Source path must be a string".to_string());
    }
    if !truthy(input.get("destination")) {
        errors.push("Destination path is required".to_string());
    }
    if !is_str(input.get("destination")) {
        errors.push("Destination path must be a string".to_string());
    }
    errors
}

fn spec(name: &str, description: &str, source_desc: &str, dest_desc: &str) -> ToolSpec {
    ToolSpec::new(
        name,
        description,
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": source_desc },
                "destination": { "type": "string", "description": dest_desc }
            },
            "required": ["source", "destination"]
        }),
    )
}

#[derive(Clone, Copy)]
enum Op {
    Move,
    Copy,
}

async fn transfer(op: Op, tool: &str, input: &Value) -> Result<ToolOutput> {
    let source = str_of(input, "source");
    let destination = str_of(input, "destination");
    let result = async {
        mkdir_recursive(&parent_dir(destination)).await?;
        match op {
            Op::Move => tokio::fs::rename(source, destination)
                .await
                .map_err(|e| NodeIoError::new(&e, "rename", source, Some(destination))),
            Op::Copy => tokio::fs::copy(source, destination)
                .await
                .map(|_| ())
                .map_err(|e| NodeIoError::new(&e, "copyfile", source, Some(destination))),
        }
    }
    .await;
    let (verb, gerund) = match op {
        Op::Move => ("moved", "moving"),
        Op::Copy => ("copied", "copying"),
    };
    match result {
        Ok(()) => Ok(ToolOutput::Text(format!(
            "File {verb}: {source} to {destination}"
        ))),
        Err(e) => {
            let mut extra = Map::new();
            extra.insert("source".into(), json!(source));
            extra.insert("destination".into(), json!(destination));
            Err(ToolError::execution(
                format!("Error {gerund} file: {e}"),
                tool,
                Some(e.to_json()),
                Some(extra),
            ))
        }
    }
}

const MOVE_NAME: &str = "moveFile";
const MOVE_DESCRIPTION: &str = "Move a file from one location to another. Use this when you need to organize files in the project structure.\n\nMove files between locations. Use absolute paths for source and destination.";

/// `MoveFileTool`.
pub struct MoveFileTool;

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &str {
        MOVE_NAME
    }
    fn description(&self) -> &str {
        MOVE_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Filesystem
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(spec(
            MOVE_NAME,
            MOVE_DESCRIPTION,
            "The current path of the file",
            "The new path for the file",
        ))
    }
    fn validate_input(&self, input: &Value) -> Vec<String> {
        validate(input)
    }
    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        transfer(Op::Move, MOVE_NAME, &input).await
    }
}

const COPY_NAME: &str = "copyFile";
const COPY_DESCRIPTION: &str = "Copy a file from one location to another. Use this when you need to duplicate a file in the project structure.\n\nCopy files to new locations. Preserves original file content.";

/// `CopyFileTool`.
pub struct CopyFileTool;

#[async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &str {
        COPY_NAME
    }
    fn description(&self) -> &str {
        COPY_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Filesystem
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(spec(
            COPY_NAME,
            COPY_DESCRIPTION,
            "The path of the file to copy",
            "The new path for the copied file",
        ))
    }
    fn validate_input(&self, input: &Value) -> Vec<String> {
        validate(input)
    }
    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        transfer(Op::Copy, COPY_NAME, &input).await
    }
}
