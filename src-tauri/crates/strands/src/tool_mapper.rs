//! Port of `src/main/services/strandsAgentsConverter/toolMapper.ts`: the Bedrock Engineer →
//! Strands Agents tool table.

/// `StrandsTool`. No entry sets `providerClass` / `initParams` today, so they are omitted; the
/// code paths that would use them (`generateSpecialSetupCode`, special imports) produce nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrandsTool {
    pub strands_name: &'static str,
    pub import_path: &'static str,
    pub supported: bool,
    pub reason: Option<&'static str>,
}

const fn tool(strands_name: &'static str) -> StrandsTool {
    StrandsTool {
        strands_name,
        import_path: "strands_tools",
        supported: true,
        reason: None,
    }
}

const fn unsupported(reason: &'static str) -> StrandsTool {
    StrandsTool {
        strands_name: "",
        import_path: "",
        supported: false,
        reason: Some(reason),
    }
}

const TODO_REASON: &str =
    "TODO tools are excluded from conversion. Can be replaced with Workflow tools";

/// `TOOL_MAPPING`, in the TS declaration order.
pub const TOOL_MAPPING: &[(&str, StrandsTool)] = &[
    // File system operations
    ("readFiles", tool("file_read")),
    ("writeToFile", tool("file_write")),
    ("listFiles", tool("editor")),
    ("createFolder", tool("shell")),
    ("moveFile", tool("shell")),
    ("copyFile", tool("shell")),
    // Web operations
    ("tavilySearch", tool("http_request")),
    ("fetchWebsite", tool("http_request")),
    // Command execution
    ("executeCommand", tool("shell")),
    // AWS Bedrock integration
    ("generateImage", tool("generate_image_stability")),
    ("generateVideo", tool("shell")), // Implemented via AWS CLI
    ("checkVideoStatus", tool("use_aws")),
    ("downloadVideo", tool("use_aws")),
    ("retrieve", tool("retrieve")),
    ("invokeBedrockAgent", tool("use_aws")),
    ("recognizeImage", tool("image_reader")),
    ("invokeFlow", tool("use_aws")),
    // Code execution
    ("codeInterpreter", tool("python_repl")),
    // Thinking and reasoning
    ("think", tool("think")),
    // File editing
    ("applyDiffEdit", tool("editor")),
    // MCP (Direct conversion is difficult, but possible via shell)
    ("mcp", tool("shell")),
    // Unsupported tools
    (
        "dockerSandbox",
        unsupported(
            "The Docker sandbox is scoped to a Bedrock Engineer chat session and has no equivalent in Strands Agents",
        ),
    ),
    (
        "screenCapture",
        unsupported("No corresponding screen capture tool available in Strands Agents"),
    ),
    (
        "cameraCapture",
        unsupported("No corresponding camera capture tool available in Strands Agents"),
    ),
    ("todo", unsupported(TODO_REASON)),
    ("todoInit", unsupported(TODO_REASON)),
    ("todoUpdate", unsupported(TODO_REASON)),
    (
        "invokeAgent",
        unsupported(
            "Agent-to-agent delegation has no direct Strands equivalent. Use the Strands multi-agent patterns (agent-as-tool, swarm, or graph) instead",
        ),
    ),
];

/// `TOOL_MAPPING[toolName]` for a built-in tool name.
pub fn strands_tool(tool_name: &str) -> Option<&'static StrandsTool> {
    TOOL_MAPPING
        .iter()
        .find(|(name, _)| *name == tool_name)
        .map(|(_, t)| t)
}

/// `generateSpecialSetupCode`: no tool needs special initialization, so always empty.
pub fn generate_special_setup_code(_tool_name: &str, _tool: &StrandsTool) -> String {
    String::new()
}

/// `generateImportStatement`: one `from strands_tools import a, b` line for the (deduplicated,
/// first-seen order) supported tool names.
pub fn generate_import_statement(tools: &[StrandsTool]) -> Vec<String> {
    let mut imports: Vec<&str> = Vec::new();
    for tool in tools.iter().filter(|t| t.supported) {
        if !imports.contains(&tool.strands_name) {
            imports.push(tool.strands_name);
        }
    }
    let mut result = Vec::new();
    if !imports.is_empty() {
        result.push(format!("from strands_tools import {}", imports.join(", ")));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_tool_has_a_mapping() {
        // `TOOL_MAPPING: Record<BuiltInToolName, StrandsTool>` is exhaustive in TS.
        for name in common::tool_names::BUILT_IN_TOOLS {
            assert!(strands_tool(name).is_some(), "{name} has no mapping");
        }
        assert_eq!(TOOL_MAPPING.len(), common::tool_names::BUILT_IN_TOOLS.len());
    }

    #[test]
    fn import_statement_dedupes_in_first_seen_order() {
        let tools = [tool("shell"), tool("file_read"), tool("shell")];
        assert_eq!(
            generate_import_statement(&tools),
            vec!["from strands_tools import shell, file_read".to_string()]
        );
        assert!(generate_import_statement(&[]).is_empty());
        assert!(generate_import_statement(&[unsupported("x")]).is_empty());
    }
}
