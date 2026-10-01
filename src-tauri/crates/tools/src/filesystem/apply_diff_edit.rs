use super::{is_str, str_of};
use crate::base::{default_handle_error, Tool};
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::{json, Map, Value};

const NAME: &str = "applyDiffEdit";
const DESCRIPTION: &str = "Apply a diff edit to a file. This tool replaces the specified original text with updated text at the exact location in the file. Use this when you need to make precise modifications to existing file content. The tool ensures that only the specified text is replaced, keeping the rest of the file intact.\n\nMake precise edits to existing files. Requires exact text matching including whitespace.\n\nExample:\n{\n   path: '/path/to/file.ts',\n   originalText: 'function oldName() {\\n  // old implementation\\n}',\n   updatedText: 'function newName() {\\n  // new implementation\\n}'\n}\n        ";

/// `ApplyDiffEditTool`.
pub struct ApplyDiffEditTool;

/// `haystack.replace(needle, replacement)` with a string pattern: replaces the first
/// occurrence, expanding the `GetSubstitution` patterns `$$`, `$&`, `` $` `` and `$'`.
pub fn js_replace_first(haystack: &str, needle: &str, replacement: &str) -> Option<String> {
    let pos = haystack.find(needle)?;
    let before = &haystack[..pos];
    let after = &haystack[pos + needle.len()..];
    let mut sub = String::with_capacity(replacement.len());
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            sub.push(c);
            continue;
        }
        match chars.peek() {
            Some('$') => {
                sub.push('$');
                chars.next();
            }
            Some('&') => {
                sub.push_str(needle);
                chars.next();
            }
            Some('`') => {
                sub.push_str(before);
                chars.next();
            }
            Some('\'') => {
                sub.push_str(after);
                chars.next();
            }
            _ => sub.push('$'),
        }
    }
    Some(format!("{before}{sub}{after}"))
}

#[async_trait]
impl Tool for ApplyDiffEditTool {
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
                    "path": {
                        "type": "string",
                        "description": "The absolute path of the file to modify. Make sure to provide the complete path starting from the root directory."
                    },
                    "originalText": {
                        "type": "string",
                        "description": "The exact original text to be replaced. Must match the text in the file exactly, including whitespace and line breaks. If the text is not found, the operation will fail."
                    },
                    "updatedText": {
                        "type": "string",
                        "description": "The new text that will replace the original text. Can be of different length than the original text. Whitespace and line breaks in this text will be preserved exactly as provided."
                    }
                },
                "required": ["path", "originalText", "updatedText"]
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
        for (key, label) in [
            ("originalText", "Original text"),
            ("updatedText", "Updated text"),
        ] {
            if matches!(input.get(key), None | Some(Value::Null)) {
                errors.push(format!("{label} is required"));
            }
            if !is_str(input.get(key)) {
                errors.push(format!("{label} must be a string"));
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let path = str_of(&input, "path");
        let original = str_of(&input, "originalText");
        let updated = str_of(&input, "updatedText");
        let fail = |e: NodeIoError| {
            let mut extra = Map::new();
            extra.insert("path".into(), json!(path));
            ToolError::execution(
                format!("Error applying diff edit: {e}"),
                NAME,
                Some(e.to_json()),
                Some(extra),
            )
        };
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| fail(NodeIoError::new(&e, "open", path, None)))?;
        let content = String::from_utf8_lossy(&bytes);
        let Some(new_content) = js_replace_first(&content, original, updated) else {
            tracing::warn!("Original text not found in file: {path}");
            return Ok(ToolOutput::Json(json!({
                "name": NAME,
                "success": false,
                "error": "Original text not found in file",
                "result": null
            })));
        };
        tokio::fs::write(path, new_content)
            .await
            .map_err(|e| fail(NodeIoError::new(&e, "open", path, None)))?;
        Ok(ToolOutput::Json(json!({
            "name": NAME,
            "success": true,
            "message": "Successfully applied diff edit",
            "result": { "path": path, "originalText": original, "updatedText": updated }
        })))
    }

    /// Rethrows `{ name, success: false, error: <BaseTool error message>, result: null }`.
    fn handle_error(&self, error: ToolError) -> ToolError {
        let base = default_handle_error(error, NAME);
        ToolError::plain(
            json!({ "name": NAME, "success": false, "error": base.message, "result": null })
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_first_with_js_substitutions() {
        assert_eq!(js_replace_first("a b a", "a", "x").unwrap(), "x b a");
        assert_eq!(js_replace_first("cost", "cost", "$$5").unwrap(), "$5");
        assert_eq!(js_replace_first("ab", "a", "[$&]").unwrap(), "[a]b");
        assert_eq!(js_replace_first("xay", "a", "$`$'").unwrap(), "xxyy");
        assert_eq!(js_replace_first("a", "a", "$1 $").unwrap(), "$1 $");
        assert_eq!(js_replace_first("abc", "", "-").unwrap(), "-abc");
        assert!(js_replace_first("abc", "z", "-").is_none());
    }
}
