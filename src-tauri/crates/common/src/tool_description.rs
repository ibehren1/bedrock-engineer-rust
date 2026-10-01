//! Port of `src/common/agents/toolDescriptionProvider.ts`.

/// Provides detailed tool descriptions for system prompt generation.
pub trait ToolDescriptionProvider {
    fn get_tool_description(&self, tool_name: &str) -> String;
}
