//! One function per `chat-attachments-*` IPC channel (port of
//! `src/main/handlers/chat-attachments-handlers.ts`), for the app crate to wrap as Tauri
//! commands. Results serialize to the same camelCase JSON shapes the renderer already expects.
//!
//! The two OS interactions stay with the caller: the native file picker (pass its result to
//! [`add_from_picker`]) and revealing a folder (pass an opener to [`open_folder`]).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context::{build_attachment_context, TextExtractor};
use crate::error::Result;
use crate::manager::{self, AttachmentPaths};
use crate::types::*;

/// `chat-attachments-list`
pub fn list(paths: &AttachmentPaths, session_id: &str) -> AttachmentListing {
    manager::list_attachments(paths, session_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithFilesResult {
    pub session_ids: Vec<String>,
}

/// `chat-attachments-with-files`: which of the sidebar's chats have files.
pub fn with_files(paths: &AttachmentPaths, session_ids: &[String]) -> WithFilesResult {
    WithFilesResult {
        session_ids: manager::list_session_ids_with_attachments(paths, session_ids),
    }
}

/// `chat-attachments-add`
pub fn add(
    paths: &AttachmentPaths,
    session_id: &str,
    files: &[NewAttachment],
) -> Result<AttachmentAddResult> {
    manager::add_attachments(paths, session_id, files)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerAddResult {
    pub canceled: bool,
    #[serde(flatten)]
    pub result: AttachmentAddResult,
}

/// `chat-attachments-add-from-picker`. `picked` is the dialog result: `None` or an empty list
/// means the dialog was dismissed, and the folder is left untouched.
pub fn add_from_picker<P: AsRef<Path>>(
    paths: &AttachmentPaths,
    session_id: &str,
    picked: Option<&[P]>,
) -> Result<PickerAddResult> {
    match picked {
        Some(file_paths) if !file_paths.is_empty() => Ok(PickerAddResult {
            canceled: false,
            result: manager::add_attachments_from_paths(paths, session_id, file_paths)?,
        }),
        _ => {
            let listing = manager::list_attachments(paths, session_id);
            Ok(PickerAddResult {
                canceled: true,
                result: AttachmentAddResult {
                    directory: listing.directory,
                    files: listing.files,
                    added: vec![],
                    errors: vec![],
                },
            })
        }
    }
}

/// `chat-attachments-remove`
pub fn remove(
    paths: &AttachmentPaths,
    session_id: &str,
    name: &str,
) -> Result<AttachmentRemoveResult> {
    manager::remove_attachment(paths, session_id, name)
}

/// `chat-attachments-remove-all`
pub fn remove_all(paths: &AttachmentPaths, session_id: &str) -> Result<RemovedFolder> {
    manager::remove_all_attachments(paths, session_id)
}

/// `chat-attachments-remove-every-folder`
pub fn remove_every_folder(paths: &AttachmentPaths) -> Result<RemovedFolders> {
    manager::remove_every_attachments_folder(paths)
}

/// `chat-attachments-rename`
pub fn rename(paths: &AttachmentPaths, session_id: &str) -> RenameResult {
    manager::rename_attachments_dir(paths, session_id)
}

/// `chat-attachments-build-context`
pub fn build_context(
    paths: &AttachmentPaths,
    session_id: &str,
    extractor: &dyn TextExtractor,
) -> AttachmentContextResult {
    build_attachment_context(paths, session_id, extractor)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenFolderResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `chat-attachments-open-folder`: reveal a chat's folder in the OS file manager, creating it
/// first so the menu item always opens something. `opener` receives the path verbatim and
/// returns `Err(message)` on failure (Electron's `shell.openPath` returned the message).
pub fn open_folder(
    paths: &AttachmentPaths,
    session_id: &str,
    opener: impl FnOnce(&str) -> std::result::Result<(), String>,
) -> OpenFolderResult {
    open_folder_with(
        || {
            manager::ensure_attachments_dir(paths, session_id)
                .map(|dir| dir.to_string_lossy().into_owned())
        },
        opener,
    )
}

fn open_folder_with(
    ensure: impl FnOnce() -> Result<String>,
    opener: impl FnOnce(&str) -> std::result::Result<(), String>,
) -> OpenFolderResult {
    let directory = match ensure() {
        Ok(directory) => directory,
        Err(error) => {
            return OpenFolderResult {
                success: false,
                path: None,
                error: Some(error.to_string()),
            }
        }
    };

    match opener(&directory) {
        Ok(()) => OpenFolderResult {
            success: true,
            path: Some(directory),
            error: None,
        },
        Err(message) => OpenFolderResult {
            success: false,
            path: None,
            error: Some(format!("Could not open {directory}: {message}")),
        },
    }
}

#[cfg(test)]
mod tests {
    //! Port of `chat-attachments-handlers.test.ts`.
    use std::cell::RefCell;

    use super::*;
    use crate::error::Error;
    use crate::manager::tests::Fixture;

    mod open_folder {
        use super::*;

        #[test]
        fn opens_the_chat_folder_creating_it_first_so_the_menu_item_always_works() {
            let fx = Fixture::new();
            let opened = RefCell::new(None::<String>);

            let result = open_folder(&fx.paths, "session_1", |dir| {
                *opened.borrow_mut() = Some(dir.to_string());
                Ok(())
            });

            let opened = opened.into_inner().unwrap();
            assert!(
                Path::new(&opened).is_dir(),
                "folder should exist before opening"
            );
            assert_eq!(
                result,
                OpenFolderResult {
                    success: true,
                    path: Some(opened),
                    error: None
                }
            );
        }

        #[test]
        fn passes_a_windows_path_through_verbatim() {
            let windows = "C:\\projects\\demo\\attachments\\chat-a3f21c";
            let opened = RefCell::new(None::<String>);

            open_folder_with(
                || Ok(windows.to_string()),
                |dir| {
                    *opened.borrow_mut() = Some(dir.to_string());
                    Ok(())
                },
            );

            assert_eq!(opened.into_inner().as_deref(), Some(windows));
        }

        #[test]
        fn reports_why_the_folder_could_not_be_created() {
            let mut fx = Fixture::new();
            fx.no_project();
            let called = RefCell::new(false);

            let result = open_folder(&fx.paths, "session_1", |_| {
                *called.borrow_mut() = true;
                Ok(())
            });

            assert_eq!(
                result,
                OpenFolderResult {
                    success: false,
                    path: None,
                    error: Some(Error::NoProjectPath.to_string())
                }
            );
            assert!(!called.into_inner());
        }

        #[test]
        fn wraps_opener_failure() {
            let result = open_folder_with(
                || Ok("/project/attachments/chat-a3f21c".to_string()),
                |_| Err("no file manager found".to_string()),
            );

            assert_eq!(
                result,
                OpenFolderResult {
                    success: false,
                    path: None,
                    error: Some(
                        "Could not open /project/attachments/chat-a3f21c: no file manager found"
                            .into()
                    )
                }
            );
        }
    }

    mod add_from_picker {
        use super::*;

        #[test]
        fn copies_the_picked_files_into_the_chat_folder() {
            let fx = Fixture::new();
            let source_dir = tempfile::tempdir().unwrap();
            let source = source_dir.path().join("a.txt");
            std::fs::write(&source, "hello").unwrap();

            let result = add_from_picker(&fx.paths, "session_1", Some(&[&source][..])).unwrap();

            assert!(!result.canceled);
            assert_eq!(result.result.added.len(), 1);
            assert_eq!(result.result.added[0].name, "a.txt");
            // The original stays where the user keeps it.
            assert!(source.exists());
        }

        #[test]
        fn reports_a_dismissed_dialog_without_touching_the_folder() {
            let fx = Fixture::new();

            let result = add_from_picker::<&Path>(&fx.paths, "session_1", None).unwrap();
            assert!(result.canceled);
            let result = add_from_picker::<&Path>(&fx.paths, "session_1", Some(&[])).unwrap();
            assert!(result.canceled);

            assert!(!fx.attachments_root().exists());
        }

        #[test]
        fn reports_unreadable_picked_files() {
            let fx = Fixture::new();
            let missing = fx.project.path().join("missing.txt");

            let result = add_from_picker(&fx.paths, "session_1", Some(&[&missing][..])).unwrap();

            assert_eq!(result.result.errors.len(), 1);
            assert_eq!(result.result.errors[0].name, "missing.txt");
        }
    }

    #[test]
    fn results_serialize_to_the_renderer_shapes() {
        let value = serde_json::to_value(PickerAddResult {
            canceled: true,
            result: AttachmentAddResult {
                directory: String::new(),
                files: vec![],
                added: vec![],
                errors: vec![],
            },
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "canceled": true, "directory": "", "files": [], "added": [], "errors": [] })
        );

        let value = serde_json::to_value(WithFilesResult {
            session_ids: vec!["a".into()],
        })
        .unwrap();
        assert_eq!(value, serde_json::json!({ "sessionIds": ["a"] }));

        let value = serde_json::to_value(RenameResult {
            renamed: false,
            directory: None,
        })
        .unwrap();
        assert_eq!(value, serde_json::json!({ "renamed": false }));
    }
}
