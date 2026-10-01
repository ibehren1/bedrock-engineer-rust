//! `window.api.help.prepareUserGuide` → `help_prepare_user_guide` (port of
//! `src/main/handlers/help-handlers.ts`).

use crate::errors;
use crate::settings::attachment_paths;
use crate::state::{store_all, StoreMutex};
use common::help::PrepareUserGuideResult;
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
