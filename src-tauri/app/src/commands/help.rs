//! `window.api.help.*` → `help_*`: the user guide for the Help page (`help_user_guide`), search
//! over its shipped knowledge base (`help_search`, offline: keyword-only, or hybrid when the
//! on-device model is installed), and attaching the whole guide to a Help chat as the fallback
//! (`help_prepare_user_guide`, port of `src/main/handlers/help-handlers.ts`).

use crate::backend::Backend;
use crate::errors;
use crate::settings::attachment_paths;
use crate::state::{store_all, StoreMutex};
use common::help::PrepareUserGuideResult;
use knowledge::help::{HelpHit, SearchMode};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

/// The guide in the repository, read directly in development so edits show up without a rebuild.
fn repository_user_guide_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/USER_GUIDE.md")
}

/// `getUserGuidePath()`: the repository copy in development, the bundled `docs/USER_GUIDE.md`
/// resource otherwise.
fn user_guide_path(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        Ok(repository_user_guide_path())
    } else {
        app.path()
            .resource_dir()
            .map(|d| d.join("docs").join("USER_GUIDE.md"))
            .map_err(errors::plain)
    }
}

/// The shipped guide knowledge base (`docs/help-kb.db`): the repository copy in development, the
/// bundled resource otherwise.
pub fn shipped_help_kb_path(app: &AppHandle) -> PathBuf {
    let file = knowledge::help::SHIPPED_FILE;
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs")
            .join(file)
    } else {
        app.path()
            .resource_dir()
            .map(|d| d.join("docs").join(file))
            .unwrap_or_else(|_| PathBuf::from(file))
    }
}

/// The user guide's Markdown, for the Help page.
#[tauri::command]
pub fn help_user_guide(app: AppHandle) -> Result<String, String> {
    let path = user_guide_path(&app)?;
    std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read user guide {}: {e}", path.display()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpSearchResult {
    mode: SearchMode,
    hits: Vec<HelpHit>,
}

/// The guide sections that best match `query` (default 6). Works offline.
#[tauri::command]
pub async fn help_search(
    backend: State<'_, Backend>,
    query: String,
    top_k: Option<usize>,
) -> Result<HelpSearchResult, String> {
    let local = &backend.local_model;
    // Queries embed with the model the guide was built with, when it is installed.
    let embedder = local
        .status()
        .installed
        .then(|| local.embedder(&knowledge::help::embedding_spec()).ok())
        .flatten();
    let (mode, hits) = backend
        .help_kb
        .search(&query, top_k.unwrap_or(6).clamp(1, 20), embedder)
        .await?;
    Ok(HelpSearchResult { mode, hits })
}

/// Attach the shipped guide to the Help chat, or return its text when there is no project
/// directory to attach into. Fails only when the guide itself cannot be read.
#[tauri::command]
pub fn help_prepare_user_guide(
    app: AppHandle,
    store: State<'_, StoreMutex>,
    session_id: String,
) -> Result<PrepareUserGuideResult, String> {
    let path = user_guide_path(&app)?;
    let guide = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read user guide {}: {e}", path.display()))?;
    let dir =
        attachments::ensure_attachments_dir(&attachment_paths(&store_all(&store)), &session_id)
            .map_err(|e| e.to_string());
    Ok(common::help::prepare_user_guide(&guide, dir))
}

#[cfg(test)]
mod tests {
    //! Port of the Electron `userGuide.test.ts`: the Help chat is useless if the guide moves and
    //! nothing notices, so pin both the development path and the bundled resource entry.
    use super::repository_user_guide_path;

    #[test]
    fn development_path_points_at_the_repository_guide() {
        let path = repository_user_guide_path();
        assert!(path.ends_with("docs/USER_GUIDE.md"), "{}", path.display());
        assert!(path.is_file(), "missing {}", path.display());
    }

    /// The guide is exempt from the per-file attachment cap but not from the total budget;
    /// past it, Help would silently lose the end of the guide.
    #[test]
    fn the_guide_fits_the_attachment_text_budget() {
        let guide = std::fs::read_to_string(repository_user_guide_path()).unwrap();
        let utf16 = guide.encode_utf16().count();
        assert!(
            utf16 < attachments::MAX_TEXT_CHARS_TOTAL,
            "USER_GUIDE.md is {utf16} UTF-16 units, over the {} attachment budget",
            attachments::MAX_TEXT_CHARS_TOTAL
        );
    }

    #[test]
    fn the_bundle_ships_the_guide_where_release_builds_read_it() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).expect("tauri.conf.json");
        assert_eq!(
            conf["bundle"]["resources"]["bundle-resources/docs/USER_GUIDE.md"],
            "docs/USER_GUIDE.md"
        );
    }
}
