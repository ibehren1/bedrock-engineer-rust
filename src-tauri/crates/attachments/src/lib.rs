//! Per-chat folders and file attachments stored under the project directory.
//!
//! Port of `src/main/api/attachments` and `src/main/handlers/chat-attachments-handlers.ts`.
//! Each chat has one folder, `<projectPath>/chats/<chat-title>/`, for everything the chat makes;
//! attached files are real files in its `attachments/` subfolder. Folders of the earlier layout
//! (`<projectPath>/attachments/<chat-title-slug>-<shortId>/`) are moved in on first use. The
//! attachments folder's contents are rebuilt into request context on every send
//! ([`build_attachment_context`]).
//!
//! The TS code read `projectPath` / `userDataPath` from the electron-store on every call; here the
//! caller passes an [`AttachmentPaths`] built from the store each time. PDF/DOCX extraction is
//! injected through [`TextExtractor`] so this crate stays free of document parsers.
//!
//! The handler-equivalent functions (one per `chat-attachments-*` IPC channel) live in
//! [`handlers`].

mod context;
mod error;
pub mod file_naming;
pub mod folder_naming;
pub mod handlers;
pub mod image_validation;
mod manager;
pub mod tidy;
mod types;

pub use context::{build_attachment_context, TextExtractor};
pub use error::{Error, Result};
pub use manager::{
    add_attachments, add_attachments_from_paths, ensure_attachments_dir, ensure_chat_dir,
    get_attachments_root, get_chats_root, is_chat_session, list_attachments,
    list_session_ids_with_attachments, read_chat_title, remove_all_attachments, remove_attachment,
    remove_chat_dir, remove_every_attachments_folder, remove_every_chat_dir,
    rename_attachments_dir, rename_chat_dir, resolve_attachments_dir, resolve_chat_dir,
    AttachmentPaths, MARKER_FILE,
};
pub use tidy::{follow_chat_dir_rename, tidy_chat_files, MovedFile, TidyInput, TidyResult};
pub use types::*;
