//! The filesystem half of `src/main/handlers/help-handlers.ts` (`help-prepare-user-guide`).
//!
//! The app crate supplies the shipped guide text and the chat's attachments directory (from the
//! `attachments` crate's `ensure_attachments_dir`, which fails when no project directory is
//! configured); this module decides whether to attach or fall back to the system prompt.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Name the guide is saved under inside a chat's attachments folder.
pub const USER_GUIDE_ATTACHMENT_NAME: &str = "BEDROCK_ENGINEER_USER_GUIDE.md";

/// `PrepareUserGuideResult`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum PrepareUserGuideResult {
    Attached {
        attached: True,
        name: String,
        path: PathBuf,
    },
    Fallback {
        attached: False,
        reason: String,
        text: String,
    },
}

/// Serializes as `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct True;
/// Serializes as `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct False;

impl Serialize for True {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(true)
    }
}
impl Serialize for False {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(false)
    }
}

impl PrepareUserGuideResult {
    pub fn is_attached(&self) -> bool {
        matches!(self, PrepareUserGuideResult::Attached { .. })
    }
}

fn write_guide(directory: &Path, guide: &str) -> std::io::Result<PathBuf> {
    let target = directory.join(USER_GUIDE_ATTACHMENT_NAME);
    // Written directly rather than through the de-duplicating attachment add, so reopening the
    // Help chat never leaves a -1, -2, … copy. A length mismatch refreshes an outdated guide.
    let current_len = fs::metadata(&target).ok().map(|m| m.len());
    if current_len != Some(guide.len() as u64) {
        fs::write(&target, guide)?;
    }
    Ok(target)
}

/// Attach `guide` to the chat's attachments folder, or return its text when that is impossible.
///
/// `attachments_dir` is the result of ensuring the chat's attachments directory exists; its
/// error message becomes the fallback `reason`.
pub fn prepare_user_guide(
    guide: &str,
    attachments_dir: Result<PathBuf, String>,
) -> PrepareUserGuideResult {
    let outcome =
        attachments_dir.and_then(|dir| write_guide(&dir, guide).map_err(|e| e.to_string()));
    match outcome {
        Ok(path) => PrepareUserGuideResult::Attached {
            attached: True,
            name: USER_GUIDE_ATTACHMENT_NAME.to_string(),
            path,
        },
        Err(reason) => {
            tracing::info!(category = "help:ipc", reason = %reason, "Falling back to the system prompt for the user guide");
            PrepareUserGuideResult::Fallback {
                attached: False,
                reason,
                text: guide.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Port of `src/main/handlers/help-handlers.test.ts`. The attachments directory is a stand-in
    //! for `ensureAttachmentsDir` (the `attachments` crate): `<project>/attachments/<chat>`.
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    const GUIDE: &str = "# Bedrock Engineer\n\nThe guide body.\n";

    fn ensure_dir(project: Option<&Path>) -> Result<PathBuf, String> {
        let project = project.ok_or_else(|| {
            "No project directory is selected, so there is nowhere to keep chat attachments"
                .to_string()
        })?;
        let dir = project.join("attachments").join("session-abc");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    fn attached_files(project: &Path) -> Vec<String> {
        let dir = project.join("attachments").join("session-abc");
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn attaches_the_guide_to_the_chat_folder() {
        let project = TempDir::new().unwrap();
        let result = prepare_user_guide(GUIDE, ensure_dir(Some(project.path())));
        match result {
            PrepareUserGuideResult::Attached { name, path, .. } => {
                assert_eq!(name, USER_GUIDE_ATTACHMENT_NAME);
                assert_eq!(fs::read_to_string(path).unwrap(), GUIDE);
            }
            other => panic!("expected attached, got {other:?}"),
        }
    }

    #[test]
    fn leaves_exactly_one_copy_behind_when_the_chat_is_reopened() {
        let project = TempDir::new().unwrap();
        prepare_user_guide(GUIDE, ensure_dir(Some(project.path())));
        prepare_user_guide(GUIDE, ensure_dir(Some(project.path())));
        assert_eq!(
            attached_files(project.path()),
            vec![USER_GUIDE_ATTACHMENT_NAME]
        );
    }

    #[test]
    fn rewrites_the_attached_copy_when_the_shipped_guide_has_changed() {
        let project = TempDir::new().unwrap();
        let PrepareUserGuideResult::Attached { path, .. } =
            prepare_user_guide(GUIDE, ensure_dir(Some(project.path())))
        else {
            panic!("expected the guide to attach");
        };
        fs::write(&path, "stale, and a different length").unwrap();

        prepare_user_guide(GUIDE, ensure_dir(Some(project.path())));

        assert_eq!(fs::read_to_string(&path).unwrap(), GUIDE);
    }

    #[test]
    fn returns_the_guide_text_instead_when_no_project_directory_is_configured() {
        let result = prepare_user_guide(GUIDE, ensure_dir(None));
        match &result {
            PrepareUserGuideResult::Fallback { reason, text, .. } => {
                assert_eq!(text, GUIDE);
                assert!(reason.contains("project directory"));
            }
            other => panic!("expected fallback, got {other:?}"),
        }
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["attached"], json!(false));
    }
}
