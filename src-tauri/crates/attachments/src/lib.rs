//! Per-chat file attachments stored under the project directory.
//!
//! Port of `src/main/api/attachments` and `src/main/handlers/chat-attachments-handlers.ts`.
//! Attachments are real files on disk, one folder per chat, at
//! `<projectPath>/attachments/<chat-title-slug>-<shortId>/` — the same layout the Electron build
//! writes, so existing folders are found. The folder's contents are rebuilt into request context
//! on every send ([`build_attachment_context`]).
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
mod types;

pub use context::{build_attachment_context, TextExtractor};
pub use error::{Error, Result};
pub use manager::{
    add_attachments, add_attachments_from_paths, ensure_attachments_dir, get_attachments_root,
    list_attachments, list_session_ids_with_attachments, read_chat_title, remove_all_attachments,
    remove_attachment, remove_every_attachments_folder, rename_attachments_dir,
    resolve_attachments_dir, AttachmentPaths,
};
pub use types::*;
