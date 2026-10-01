//! Filesystem tools (`src/preload/tools/handlers/filesystem`).

mod apply_diff_edit;
mod create_folder;
mod list_files;
mod move_copy;
mod read_files;
mod write_to_file;

pub use apply_diff_edit::{js_replace_first, ApplyDiffEditTool};
pub use create_folder::CreateFolderTool;
pub use list_files::ListFilesTool;
pub use move_copy::{CopyFileTool, MoveFileTool};
pub use read_files::{decode_with_encoding, ReadFilesTool};
pub use write_to_file::WriteToFileTool;

use crate::base::Tool;
use crate::util::line_range::LineRange;
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

/// Extension point for PDF / DOCX text extraction (`PdfReader` / `DocxReader`, which
/// delegate to the main-process `pdf-extract-text` / `docx-extract-text` handlers).
/// Implementations return the extracted text with `lines` already applied, like those
/// handlers; errors are the JS `error.message`.
#[async_trait]
pub trait DocumentReader: Send + Sync {
    async fn extract_pdf_text(
        &self,
        path: &str,
        lines: Option<LineRange>,
    ) -> std::result::Result<String, String>;
    async fn extract_docx_text(
        &self,
        path: &str,
        lines: Option<LineRange>,
    ) -> std::result::Result<String, String>;
}

/// `createFilesystemTools()`, in the TS registration order.
pub fn create_filesystem_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(CreateFolderTool),
        Arc::new(WriteToFileTool),
        Arc::new(ReadFilesTool),
        Arc::new(ApplyDiffEditTool),
        Arc::new(ListFilesTool),
        Arc::new(MoveFileTool),
        Arc::new(CopyFileTool),
    ]
}

pub(crate) fn is_str(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::String(_)))
}

pub(crate) fn str_of<'a>(input: &'a Value, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// `path.dirname` for the host platform.
pub(crate) fn parent_dir(p: &str) -> String {
    match Path::new(p).parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_string_lossy().into_owned(),
        Some(_) => ".".to_string(),
        None => p.to_string(),
    }
}

/// `fs.mkdir(dir, { recursive: true })` with Node-style errors.
pub(crate) async fn mkdir_recursive(dir: &str) -> std::result::Result<(), NodeIoError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| NodeIoError::new(&e, "mkdir", dir, None))
}

#[cfg(test)]
mod tests;
