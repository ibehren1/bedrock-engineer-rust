//! Built-in vs MCP tool naming — port of the name helpers in `src/types/tools.ts` and
//! `src/types/plan-mode-tools.ts`.
//!
//! MCP tools keep the server-provided name unchanged (no prefix). Anything that is not a built-in
//! tool name is treated as an MCP tool. The legacy `mcp_<name>` form is still accepted and the
//! prefix stripped.

/// `BUILT_IN_TOOLS`
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

/// `READ_ONLY_TOOLS` — tools usable in PLAN MODE.
pub const READ_ONLY_TOOLS: &[&str] = &[
    "readFiles",
    "listFiles",
    "tavilySearch",
    "fetchWebsite",
    "recognizeImage",
    "retrieve",
    "invokeBedrockAgent",
    "think",
];

const LEGACY_MCP_PREFIX: &str = "mcp_";

/// `isBuiltInTool`
pub fn is_built_in_tool(name: &str) -> bool {
    BUILT_IN_TOOLS.contains(&name)
}

/// `isMcpTool` — every non-built-in name is an MCP tool.
pub fn is_mcp_tool(name: &str) -> bool {
    !is_built_in_tool(name)
}

/// `isLegacyMcpTool`
pub fn is_legacy_mcp_tool(name: &str) -> bool {
    name.starts_with(LEGACY_MCP_PREFIX)
}

/// `getOriginalMcpToolName` — strips the legacy `mcp_` prefix, otherwise returns the name as-is.
pub fn get_original_mcp_tool_name(name: &str) -> &str {
    name.strip_prefix(LEGACY_MCP_PREFIX).unwrap_or(name)
}

/// `isPlanModeCompatible`
pub fn is_plan_mode_compatible(name: &str) -> bool {
    READ_ONLY_TOOLS.contains(&name)
}

/// How the tool registry dispatches a tool-use `type` (`ToolRegistry.execute`/`resolveToolName`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolDispatch<'a> {
    /// A built-in tool, by name.
    BuiltIn(&'a str),
    /// Routed through the MCP adapter; carries the original MCP tool name.
    Mcp(&'a str),
}

/// Resolve a tool-use name the way the TS `ToolRegistry` does: legacy `mcp_` names and any
/// non-built-in name go to the MCP adapter with the prefix stripped.
pub fn resolve_tool_dispatch(tool_type: &str) -> ToolDispatch<'_> {
    if is_legacy_mcp_tool(tool_type) || is_mcp_tool(tool_type) {
        ToolDispatch::Mcp(get_original_mcp_tool_name(tool_type))
    } else {
        ToolDispatch::BuiltIn(tool_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_names() {
        assert!(is_built_in_tool("readFiles"));
        assert!(is_mcp_tool("search_documentation"));
        assert!(!is_mcp_tool("mcp"));
        assert_eq!(get_original_mcp_tool_name("mcp_foo"), "foo");
        assert_eq!(get_original_mcp_tool_name("foo"), "foo");
        assert!(is_plan_mode_compatible("think"));
        assert!(!is_plan_mode_compatible("writeToFile"));
    }

    #[test]
    fn dispatch_matches_registry() {
        assert_eq!(
            resolve_tool_dispatch("readFiles"),
            ToolDispatch::BuiltIn("readFiles")
        );
        assert_eq!(
            resolve_tool_dispatch("mcp_search"),
            ToolDispatch::Mcp("search")
        );
        assert_eq!(resolve_tool_dispatch("search"), ToolDispatch::Mcp("search"));
    }
}
