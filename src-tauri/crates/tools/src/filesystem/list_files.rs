use super::{is_str, str_of};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::gitignore::GitignoreLikeMatcher;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use crate::util::line_range::{
    filter_by_line_range, get_line_range_info, validate_line_range, LineRange,
};
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;

const NAME: &str = "listFiles";
const DESCRIPTION: &str = "List the entire directory structure, including all subdirectories and files, in a hierarchical format with line range filtering support. Use maxDepth to limit directory depth and lines to filter output.\n\nList directory contents with optional filtering. Use to understand project structure before modifications.";

/// `ListFilesTool`.
pub struct ListFilesTool;

/// Lexically normalized absolute form of `p` (relative paths resolve against the cwd).
fn absolutize(p: &Path, cwd: &Path) -> PathBuf {
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path.relative(from, to)`, joined with `/`.
fn relative(from: &Path, to: &Path) -> String {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..from.len() {
        parts.push("..".to_string());
    }
    for c in &to[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

struct TreeBuilder {
    matcher: GitignoreLikeMatcher,
    has_patterns: bool,
    max_depth: i64,
    cwd: PathBuf,
}

impl TreeBuilder {
    /// `buildFileTree`.
    fn build<'a>(
        &'a self,
        dir: &'a Path,
        prefix: String,
        depth: i64,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let inner = async {
                if self.max_depth != -1 && depth > self.max_depth {
                    return Ok(format!("{prefix}...\n"));
                }
                let dir_str = dir.to_string_lossy().into_owned();
                let mut rd = tokio::fs::read_dir(dir)
                    .await
                    .map_err(|e| NodeIoError::new(&e, "scandir", &dir_str, None).message)?;
                // Node's readdir (libuv scandir) returns entries sorted by name.
                let mut entries = Vec::new();
                while let Some(e) = rd
                    .next_entry()
                    .await
                    .map_err(|e| NodeIoError::new(&e, "scandir", &dir_str, None).message)?
                {
                    let is_dir = e.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                    entries.push((e.file_name().to_string_lossy().into_owned(), is_dir));
                }
                entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

                let mut result = String::new();
                let count = entries.len();
                for (i, (name, is_dir)) in entries.into_iter().enumerate() {
                    let is_last = i == count - 1;
                    let current_prefix =
                        format!("{prefix}{}", if is_last { "└── " } else { "├── " });
                    let next_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
                    let file_path = dir.join(&name);
                    if self.has_patterns {
                        let rel = relative(&self.cwd, &absolutize(&file_path, &self.cwd));
                        if self.matcher.is_ignored(&rel) {
                            continue;
                        }
                    }
                    if is_dir {
                        result.push_str(&format!("{current_prefix}📁 {name}\n"));
                        let sub = self.build(&file_path, next_prefix, depth + 1).await?;
                        result.push_str(&sub);
                    } else {
                        result.push_str(&format!("{current_prefix}📄 {name}\n"));
                    }
                }
                Ok(result)
            };
            inner
                .await
                .map_err(|msg: String| format!("Error building file tree: {msg}"))
        })
    }
}

#[async_trait]
impl Tool for ListFilesTool {
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
                    "path": { "type": "string", "description": "The root path to start listing the directory structure from" },
                    "options": {
                        "type": "object",
                        "description": "Optional configurations for listing files",
                        "properties": {
                            "ignoreFiles": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Array of patterns to ignore when listing files (gitignore format)"
                            },
                            "maxDepth": { "type": "number", "description": "Maximum depth of directory traversal (-1 for unlimited)" },
                            "recursive": { "type": "boolean", "description": "Whether to list files recursively" },
                            "lines": {
                                "type": "object",
                                "description": "Line range to display from the directory listing output",
                                "properties": {
                                    "from": { "type": "number", "description": "Starting line number (1-based, inclusive)" },
                                    "to": { "type": "number", "description": "Ending line number (1-based, inclusive)" }
                                }
                            }
                        }
                    }
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
        if let Some(options) = input.get("options").filter(|o| truthy(Some(o))) {
            let lines = options.get("lines");
            if truthy(lines) {
                errors.extend(validate_line_range(lines));
            }
            if let Some(md) = options.get("maxDepth") {
                if md.as_f64().is_none_or(|n| n < -1.0) {
                    errors.push("maxDepth must be -1 or a non-negative number".to_string());
                }
            }
            if let Some(ig) = options.get("ignoreFiles") {
                if !ig.is_array() {
                    errors.push("ignoreFiles must be an array".to_string());
                }
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let dir = str_of(&input, "path");
        let options = input.get("options");
        let ignore_files: Vec<String> = match options.and_then(|o| o.get("ignoreFiles")) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect(),
            _ => ctx.settings.ignore_files.clone(),
        };
        let max_depth = options
            .and_then(|o| o.get("maxDepth"))
            .and_then(Value::as_f64)
            .map(|n| n.trunc() as i64)
            .unwrap_or(-1);
        let lines = LineRange::from_value(options.and_then(|o| o.get("lines")));

        let builder = TreeBuilder {
            matcher: GitignoreLikeMatcher::new(&ignore_files),
            has_patterns: !ignore_files.is_empty(),
            max_depth,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        };
        match builder.build(Path::new(dir), String::new(), 0).await {
            Ok(tree) => {
                let filtered = filter_by_line_range(&tree, lines.as_ref());
                let total = tree.split('\n').count();
                let info = get_line_range_info(total, lines.as_ref());
                Ok(ToolOutput::Text(format!(
                    "Directory Structure{info}:\n\n{filtered}"
                )))
            }
            Err(msg) => Err(ToolError::execution(
                format!("Error listing directory structure: {msg}"),
                NAME,
                Some(json!({})),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths() {
        assert_eq!(relative(Path::new("/a/b"), Path::new("/a/b/c/d")), "c/d");
        assert_eq!(relative(Path::new("/a/b"), Path::new("/a/x")), "../x");
        assert_eq!(
            absolutize(Path::new("x/../y"), Path::new("/cwd")),
            PathBuf::from("/cwd/y")
        );
    }
}
