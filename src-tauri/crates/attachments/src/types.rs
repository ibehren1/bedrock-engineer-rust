//! Types and limits for chat attachments (port of `types.ts`).
//!
//! The caps bound how much of a chat's attachments folder can reach the model on each send.

use serde::{Deserialize, Serialize};

/// Folder under the project directory that holds every chat's attachments folder.
pub const ATTACHMENTS_ROOT_DIRNAME: &str = "attachments";

/// How a file is treated when the request context is assembled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    Text,
    Pdf,
    Docx,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    /// File name inside the chat's attachments folder.
    pub name: String,
    /// Absolute path, so the agent can also reach the file with its filesystem tools.
    pub path: String,
    pub size: u64,
    /// Modification time in epoch milliseconds (fractional, like Node's `mtimeMs`).
    pub mtime: f64,
    pub kind: AttachmentKind,
    /// Whether the file's text is inlined into the injected context.
    pub extractable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentListing {
    pub directory: String,
    pub files: Vec<ChatAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentError {
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentAddResult {
    pub directory: String,
    pub files: Vec<ChatAttachment>,
    pub added: Vec<ChatAttachment>,
    pub errors: Vec<AttachmentError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRemoveResult {
    pub directory: String,
    pub files: Vec<ChatAttachment>,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentContextResult {
    pub directory: String,
    /// Bedrock `ContentBlock[]` as JSON (`{ text }` and `{ image: { format, source: { bytes } } }`
    /// with base64 bytes).
    pub blocks: Vec<serde_json::Value>,
    pub file_count: usize,
    pub image_count: usize,
    pub total_text_chars: usize,
    pub truncated_files: Vec<String>,
    pub skipped_files: Vec<SkippedFile>,
}

/// A file to write into a chat's folder (dropped or pasted in the renderer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAttachment {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// `{ removed }` result of deleting one chat's folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedFolder {
    pub removed: bool,
}

/// `{ removed }` count of folders deleted by "delete all chats".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedFolders {
    pub removed: usize,
}

/// Result of the explicit rename hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameResult {
    pub renamed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

/// Bedrock Converse image limits.
pub const MAX_IMAGE_BYTES: usize = 3_932_160; // 3.75 * 1024 * 1024
pub const MAX_IMAGE_DIMENSION: u32 = 8000;
pub const MAX_IMAGES: usize = 20;

/// Image formats Bedrock accepts as image content blocks.
pub const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp"];

/// Injected-text budget (UTF-16 code units, like JS string length), per file and in total.
pub const MAX_TEXT_CHARS_PER_FILE: usize = 120_000;
pub const MAX_TEXT_CHARS_TOTAL: usize = 480_000;

/// Extensions whose text is extracted inline. Anything else is attached and listed, but reaches
/// the model as a path only. Spreadsheets are deliberately absent.
pub const EXTRACTABLE_TEXT_EXTENSIONS: &[&str] = &[
    // Documents
    ".pdf",
    ".docx", // Plain text / data
    ".txt",
    ".md",
    ".markdown",
    ".csv",
    ".tsv",
    ".json",
    ".jsonl",
    ".yaml",
    ".yml",
    ".xml",
    ".html",
    ".htm",
    ".log", // Code / config
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".py",
    ".java",
    ".go",
    ".rs",
    ".c",
    ".cpp",
    ".sh",
    ".sql",
    ".toml",
    ".ini",
    ".env",
    ".cfg",
    ".tf",
    ".hcl",
];
