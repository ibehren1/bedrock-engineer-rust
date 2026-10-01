use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js;
use crate::util::line_range::{
    filter_by_line_range, get_line_range_info, validate_line_range, LineRange,
};
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

const NAME: &str = "readFiles";
const DESCRIPTION: &str = "Read the content of multiple files at the specified paths with line range filtering support. For Excel files, the content is converted to CSV format. For PDF files, text content is extracted. For Word documents (.docx), text content is extracted.\n\nRead content from multiple files simultaneously. Supports line range filtering, Excel conversion, PDF text extraction, and DOCX text extraction.";

/// `ReadFilesTool`.
///
/// Like the TS tool, text files (including `.xlsx` / `.xls`, despite the description) are
/// read with the requested encoding; `.pdf` and `.docx` go through the
/// [`super::DocumentReader`] in the context.
pub struct ReadFilesTool;

/// A read failure: `(message, JSON.stringify(error))`.
type ReadError = (String, Value);

fn ext_lower(p: &str) -> String {
    Path::new(p)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Decode bytes the way `fs.readFile(path, encoding)` does for the Node encodings.
pub fn decode_with_encoding(bytes: &[u8], encoding: &str) -> std::result::Result<String, String> {
    match encoding.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(String::from_utf8_lossy(bytes).into_owned()),
        "latin1" | "binary" => Ok(bytes.iter().map(|b| *b as char).collect()),
        "ascii" => Ok(bytes.iter().map(|b| (b & 0x7f) as char).collect()),
        "hex" => Ok(bytes.iter().map(|b| format!("{b:02x}")).collect()),
        "base64" => Ok(base64_encode(bytes, false)),
        "base64url" => Ok(base64_encode(bytes, true)),
        "utf16le" | "utf-16le" | "ucs2" | "ucs-2" => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&units))
        }
        _ => Err(format!(
            "The argument 'encoding' is invalid encoding. Received '{encoding}'"
        )),
    }
}

fn base64_encode(bytes: &[u8], url: bool) -> String {
    let table: &[u8; 64] = if url {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32,
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            _ => (chunk[0] as u32) << 16,
        };
        let chars = [
            table[(n >> 18) as usize & 63],
            table[(n >> 12) as usize & 63],
            table[(n >> 6) as usize & 63],
            table[n as usize & 63],
        ];
        let keep = chunk.len() + 1;
        for c in &chars[..keep] {
            out.push(*c as char);
        }
        if !url {
            for _ in keep..4 {
                out.push('=');
            }
        }
    }
    out
}

impl ReadFilesTool {
    async fn read_text(path: &str, encoding: &str) -> std::result::Result<String, ReadError> {
        if tokio::fs::metadata(path).await.is_ok_and(|m| m.is_dir()) {
            // Node reports EISDIR from read(2), without a path.
            return Err((
                "EISDIR: illegal operation on a directory, read".to_string(),
                json!({"errno": -21, "code": "EISDIR", "syscall": "read"}),
            ));
        }
        let bytes = tokio::fs::read(path).await.map_err(|e| {
            let ne = NodeIoError::new(&e, "open", path, None);
            (ne.message.clone(), ne.to_json())
        })?;
        decode_with_encoding(&bytes, encoding)
            .map_err(|m| (m, json!({"code": "ERR_INVALID_ARG_VALUE"})))
    }

    /// Read one file of any supported kind (without formatting).
    async fn read_any(
        path: &str,
        encoding: &str,
        lines: Option<LineRange>,
        ctx: &ToolContext,
    ) -> std::result::Result<String, ReadError> {
        let ext = ext_lower(path);
        if ext == "pdf" || ext == "docx" {
            let Some(reader) = &ctx.documents else {
                return Err((
                    format!("{} text extraction is not available", ext.to_uppercase()),
                    json!({}),
                ));
            };
            let r = if ext == "pdf" {
                reader.extract_pdf_text(path, lines).await
            } else {
                reader.extract_docx_text(path, lines).await
            };
            return r.map_err(|m| (m, json!({})));
        }
        Self::read_text(path, encoding).await
    }

    /// `formatFileContent`.
    fn format_file_content(path: &str, content: &str, lines: Option<&LineRange>) -> String {
        let filtered = filter_by_line_range(content, lines);
        let total = content.split('\n').count();
        let info = get_line_range_info(total, lines);
        let rule = "=".repeat(js::len(path) + js::len(&info) + 6);
        format!("File: {path}{info}\n{rule}\n{filtered}")
    }
}

#[async_trait]
impl Tool for ReadFilesTool {
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
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of file paths to read. Supports text files, Excel files (.xlsx, .xls), PDF files (.pdf), and Word documents (.docx)."
                    },
                    "options": {
                        "type": "object",
                        "description": "Optional configurations for reading files",
                        "properties": {
                            "encoding": { "type": "string", "description": "File encoding (default: utf-8)" },
                            "lines": {
                                "type": "object",
                                "description": "Line range to read from the file",
                                "properties": {
                                    "from": { "type": "number", "description": "Starting line number (1-based, inclusive)" },
                                    "to": { "type": "number", "description": "Ending line number (1-based, inclusive)" }
                                }
                            }
                        }
                    }
                },
                "required": ["paths"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let paths = input.get("paths");
        if !js::truthy(paths) {
            errors.push("Paths array is required".to_string());
        }
        match paths.and_then(Value::as_array) {
            None => errors.push("Paths must be an array".to_string()),
            Some(a) if a.is_empty() => errors.push("At least one path is required".to_string()),
            Some(a) => {
                for (i, p) in a.iter().enumerate() {
                    if !p.is_string() {
                        errors.push(format!("Path at index {i} must be a string"));
                    }
                }
            }
        }
        let lines = input.pointer("/options/lines");
        if js::truthy(lines) {
            errors.extend(validate_line_range(lines));
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let paths: Vec<String> = input
            .get("paths")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let encoding = input
            .pointer("/options/encoding")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("utf-8")
            .to_string();
        let lines = LineRange::from_value(input.pointer("/options/lines"));

        if paths.len() == 1 {
            let path = &paths[0];
            let ext = ext_lower(path);
            return match Self::read_any(path, &encoding, lines, ctx).await {
                Ok(content) => Ok(ToolOutput::Text(Self::format_file_content(
                    path,
                    &content,
                    lines.as_ref(),
                ))),
                Err((msg, cause)) => {
                    let kind = match ext.as_str() {
                        "pdf" => "PDF file ",
                        "docx" => "DOCX file ",
                        _ => "file ",
                    };
                    Err(ToolError::execution(
                        format!("Error reading {kind}{path}: {msg}"),
                        NAME,
                        Some(cause),
                        None,
                    ))
                }
            };
        }

        let mut contents = Vec::with_capacity(paths.len());
        for path in &paths {
            match Self::read_any(path, &encoding, lines, ctx).await {
                Ok(content) => {
                    contents.push(Self::format_file_content(path, &content, lines.as_ref()))
                }
                Err((msg, _)) => {
                    contents.push(format!("## Error reading file: {path}\nError: {msg}"))
                }
            }
        }
        Ok(ToolOutput::Text(contents.join("\n\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        assert_eq!(decode_with_encoding(b"hi", "utf8").unwrap(), "hi");
        assert_eq!(decode_with_encoding(b"hi", "hex").unwrap(), "6869");
        assert_eq!(decode_with_encoding(b"hi!", "base64").unwrap(), "aGkh");
        assert_eq!(decode_with_encoding(b"h", "base64").unwrap(), "aA==");
        assert_eq!(decode_with_encoding(&[0xe9], "latin1").unwrap(), "é");
        assert_eq!(decode_with_encoding(&[0x68, 0], "utf16le").unwrap(), "h");
        assert!(decode_with_encoding(b"", "klingon").is_err());
    }
}
