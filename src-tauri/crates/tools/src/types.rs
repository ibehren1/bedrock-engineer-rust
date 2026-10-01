//! Shared tool types: output shape, categories, tool specs, built-in names.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A tool's return value. The TS tools return either a plain string or a `ToolResult`
/// object; the renderer branches on whether the value has a `name` property, so the two
/// serialize untagged (a JSON string, or the object as-is).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolOutput {
    Text(String),
    Json(Value),
}

impl ToolOutput {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ToolOutput::Text(s) => Some(s),
            ToolOutput::Json(_) => None,
        }
    }

    pub fn as_json(&self) -> Option<&Value> {
        match self {
            ToolOutput::Json(v) => Some(v),
            ToolOutput::Text(_) => None,
        }
    }

    /// The value the renderer receives.
    pub fn into_value(self) -> Value {
        match self {
            ToolOutput::Text(s) => Value::String(s),
            ToolOutput::Json(v) => v,
        }
    }
}

/// `ToolCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Filesystem,
    Bedrock,
    Web,
    Command,
    Thinking,
    Mcp,
    Interpreter,
    System,
    Agent,
    Docker,
}

impl ToolCategory {
    pub const ALL: [ToolCategory; 10] = [
        ToolCategory::Filesystem,
        ToolCategory::Bedrock,
        ToolCategory::Web,
        ToolCategory::Command,
        ToolCategory::Thinking,
        ToolCategory::Mcp,
        ToolCategory::Interpreter,
        ToolCategory::System,
        ToolCategory::Agent,
        ToolCategory::Docker,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ToolCategory::Filesystem => "filesystem",
            ToolCategory::Bedrock => "bedrock",
            ToolCategory::Web => "web",
            ToolCategory::Command => "command",
            ToolCategory::Thinking => "thinking",
            ToolCategory::Mcp => "mcp",
            ToolCategory::Interpreter => "interpreter",
            ToolCategory::System => "system",
            ToolCategory::Agent => "agent",
            ToolCategory::Docker => "docker",
        }
    }
}

/// Bedrock `toolSpec`: `{ name, description, inputSchema: { json } }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl ToolSpec {
    pub fn new(name: &str, description: &str, input_schema: Value) -> Self {
        ToolSpec {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        }
    }

    /// `{ toolSpec: { name, description, inputSchema: { json } } }`, the element shape of
    /// `ToolMetadataCollector.getToolSpecs()`.
    pub fn to_bedrock_tool(&self) -> Value {
        json!({
            "toolSpec": {
                "name": self.name,
                "description": self.description,
                "inputSchema": { "json": self.input_schema }
            }
        })
    }
}

/// `BUILT_IN_TOOLS` from `src/types/tools.ts`. Any other tool name is an MCP tool.
pub const BUILT_IN_TOOLS: &[&str] = &[
    "createFolder",
    "readFiles",
    "writeToFile",
    "listFiles",
    "moveFile",
    "copyFile",
    "tavilySearch",
    "fetchWebsite",
    "generateImage",
    "generateVideo",
    "checkVideoStatus",
    "downloadVideo",
    "retrieve",
    "invokeBedrockAgent",
    "executeCommand",
    "applyDiffEdit",
    "think",
    "recognizeImage",
    "invokeFlow",
    "codeInterpreter",
    "dockerSandbox",
    "mcp",
    "screenCapture",
    "cameraCapture",
    "todo",
    "todoInit",
    "todoUpdate",
    "invokeAgent",
];

/// Order of `ToolMetadataCollector.getToolSpecs()`.
pub const TOOL_SPEC_ORDER: &[&str] = &[
    "createFolder",
    "writeToFile",
    "readFiles",
    "listFiles",
    "applyDiffEdit",
    "moveFile",
    "copyFile",
    "tavilySearch",
    "fetchWebsite",
    "generateImage",
    "generateVideo",
    "checkVideoStatus",
    "downloadVideo",
    "recognizeImage",
    "retrieve",
    "invokeBedrockAgent",
    "invokeFlow",
    "executeCommand",
    "think",
    "codeInterpreter",
    "dockerSandbox",
    "screenCapture",
    "cameraCapture",
    "todoInit",
    "todoUpdate",
    "invokeAgent",
];

/// `isBuiltInTool`.
pub fn is_built_in_tool(name: &str) -> bool {
    BUILT_IN_TOOLS.contains(&name)
}

/// `isMcpTool`: anything that is not built in.
pub fn is_mcp_tool(name: &str) -> bool {
    !is_built_in_tool(name)
}

/// `getOriginalMcpToolName`: strips the legacy `mcp_` prefix.
pub fn original_mcp_tool_name(name: &str) -> &str {
    name.strip_prefix("mcp_").unwrap_or(name)
}
