//! `window.workspaceFiles.*` → `workspace_files_*`: the Files panel (backed by the
//! `workspace_files` crate). The root is the store's current `projectPath`, read on every call;
//! every path argument is root-relative or absolute and must stay under it.
//!
//! The watcher ([`WatchState`]) is started by `workspace_files_watch`, follows `projectPath`
//! changes while it is on ([`on_store_change`]), and emits [`CHANGED_EVENT`] to every window
//! with `{ root, paths }` (root-relative, deduplicated) after 300 ms of quiet. Several windows
//! can want it (the main window's Files panel, pop-out file viewers): it runs while any window
//! that called `workspace_files_watch` has neither called `workspace_files_unwatch` nor been
//! destroyed ([`on_window_destroyed`]). Within a window the bridge reference-counts its own
//! callers and sends one watch / unwatch.

use crate::errors;
use crate::state::{project_path, StoreMutex};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Window};
use tauri_plugin_opener::OpenerExt;
use workspace_files::watch::ProjectWatcher;
use workspace_files::{Error, Found, Listing, TextFile, Written, MAX_TEXT_BYTES};

/// Emitted with [`workspace_files::watch::Changes`] (`{ root, paths }`).
pub const CHANGED_EVENT: &str = "workspace-files-changed";

/// The current `projectPath`, or "No project folder is set".
pub(crate) fn root(store: &StoreMutex) -> Result<PathBuf, String> {
    project_path(store).ok_or_else(|| Error::NoRoot.to_string())
}

/// Run blocking file I/O off the async workers.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> workspace_files::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(errors::plain)?
        .map_err(String::from)
}

/// One level of `dir` (`""` / missing: the root).
#[tauri::command]
pub async fn workspace_files_list(
    store: State<'_, StoreMutex>,
    dir: Option<String>,
) -> Result<Listing, String> {
    let root = root(&store)?;
    blocking(move || workspace_files::list_dir(&root, dir.as_deref().unwrap_or(""))).await
}

/// A text file's content (first 2 MB); "Not a text file" for binary / non-UTF-8 files.
#[tauri::command]
pub async fn workspace_files_read_text(
    store: State<'_, StoreMutex>,
    path: String,
) -> Result<TextFile, String> {
    let root = root(&store)?;
    blocking(move || workspace_files::read_text(&root, &path, MAX_TEXT_BYTES)).await
}

/// Save a text file. With `expectedModifiedMs`, an error starting with `CONFLICT:` when the
/// file changed on disk since it was read.
#[tauri::command]
pub async fn workspace_files_write_text(
    store: State<'_, StoreMutex>,
    path: String,
    content: String,
    expected_modified_ms: Option<f64>,
) -> Result<Written, String> {
    let root = root(&store)?;
    blocking(move || workspace_files::write_text(&root, &path, &content, expected_modified_ms))
        .await
}

/// Resolve `path` under the current project folder.
fn resolve(store: &StoreMutex, path: &str) -> Result<PathBuf, String> {
    let root = workspace_files::canonical_root(&root(store)?)?;
    Ok(workspace_files::resolve_in_root(&root, path)?)
}

/// Open a file (or folder) in the OS default app.
#[tauri::command]
pub fn workspace_files_open(
    app: AppHandle,
    store: State<'_, StoreMutex>,
    path: String,
) -> Result<(), String> {
    let abs = resolve(&store, &path)?;
    if !abs.exists() {
        return Err(Error::NotFound(path).to_string());
    }
    app.opener()
        .open_path(abs.display().to_string(), None::<&str>)
        .map_err(errors::plain)
}

/// Show `path` selected in Finder / Explorer / the file manager; without a path, open the
/// project folder itself.
#[tauri::command]
pub fn workspace_files_reveal(
    app: AppHandle,
    store: State<'_, StoreMutex>,
    path: Option<String>,
) -> Result<(), String> {
    let path = path.unwrap_or_default();
    let abs = resolve(&store, &path)?;
    if path.is_empty() {
        return app
            .opener()
            .open_path(abs.display().to_string(), None::<&str>)
            .map_err(errors::plain);
    }
    if !abs.exists() {
        return Err(Error::NotFound(path).to_string());
    }
    app.opener().reveal_item_in_dir(&abs).map_err(errors::plain)
}

/// For each input path: `{ path, kind }` (root-relative) if it exists under the project folder,
/// else `null`. Keyed by the input string.
#[tauri::command]
pub async fn workspace_files_exists(
    store: State<'_, StoreMutex>,
    paths: Vec<String>,
) -> Result<BTreeMap<String, Option<Found>>, String> {
    let root = root(&store)?;
    blocking(move || workspace_files::exists(&root, &paths)).await
}

/// Start watching the current project folder (or retarget to it) for the calling window;
/// idempotent per window. Stays on (and follows `projectPath`) until every watching window has
/// called `workspace_files_unwatch` or closed, even when this call fails because no folder is
/// set yet.
#[tauri::command]
pub async fn workspace_files_watch(
    app: AppHandle,
    window: Window,
    store: State<'_, StoreMutex>,
) -> Result<(), String> {
    let root = project_path(&store);
    let state = app.state::<WatchState>();
    state.add_watcher(window.label());
    let generation = state.next_generation();
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle
            .state::<WatchState>()
            .retarget(&handle, root, generation)
    })
    .await
    .map_err(errors::plain)?
}

/// The calling window no longer needs the watcher; it stops when no other window does.
#[tauri::command]
pub fn workspace_files_unwatch(window: Window, state: State<'_, WatchState>) -> Result<(), String> {
    state.remove_watcher(window.label())
}

/// A window went away: drop its watch (it can't call `workspace_files_unwatch` any more).
pub fn on_window_destroyed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(state) = app.try_state::<WatchState>() {
        if let Err(e) = state.remove_watcher(label) {
            tracing::warn!(category = "workspace-files", error = %e, "Failed to stop the watcher");
        }
    }
}

/// The project folder watcher and the windows that want it. Retargets are numbered so that,
/// when several race (quick `projectPath` changes), only the newest one takes effect.
#[derive(Default)]
pub struct WatchState {
    /// Labels of the windows watching. The watcher runs while this is non-empty.
    watchers: Mutex<BTreeSet<String>>,
    generation: AtomicU64,
    current: Mutex<Option<ProjectWatcher>>,
}

impl WatchState {
    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Whether any window wants the watcher.
    fn wanted(&self) -> bool {
        self.watchers.lock().map(|w| !w.is_empty()).unwrap_or(false)
    }

    /// `label` wants the watcher (again: no-op).
    fn add_watcher(&self, label: &str) {
        if let Ok(mut w) = self.watchers.lock() {
            w.insert(label.to_string());
        }
    }

    /// `label` is done with the watcher; stops it when that was the last window. Unknown labels
    /// are a no-op.
    fn remove_watcher(&self, label: &str) -> Result<(), String> {
        let now_empty = {
            let mut w = self.watchers.lock().map_err(errors::plain)?;
            if !w.remove(label) {
                return Ok(());
            }
            w.is_empty()
        };
        if now_empty {
            self.next_generation();
            let mut current = self.current.lock().map_err(errors::plain)?;
            // A window may have started watching between the two locks.
            if !self.wanted() {
                *current = None;
            }
        }
        Ok(())
    }

    /// Watch `root` (`None`: stop, "No project folder is set"), unless a newer request came in
    /// or watching was turned off meanwhile.
    fn retarget<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        root: Option<PathBuf>,
        generation: u64,
    ) -> Result<(), String> {
        let mut current = self.current.lock().map_err(errors::plain)?;
        if generation != self.generation.load(Ordering::SeqCst) || !self.wanted() {
            return Ok(());
        }
        let Some(root) = root else {
            *current = None;
            return Err(Error::NoRoot.to_string());
        };
        let canonical = match workspace_files::canonical_root(&root) {
            Ok(p) => p,
            Err(e) => {
                *current = None;
                return Err(e.to_string());
            }
        };
        if current.as_ref().is_some_and(|w| w.root() == canonical) {
            return Ok(());
        }
        // Stop the old watcher before the new one walks the tree.
        *current = None;
        let handle = app.clone();
        let watcher = ProjectWatcher::start(&canonical, move |changes| {
            if let Err(e) = handle.emit(CHANGED_EVENT, &changes) {
                tracing::warn!(category = "workspace-files", error = %e, "Failed to emit changes");
            }
        })?;
        tracing::info!(
            category = "workspace-files",
            root = %canonical.display(),
            "Watching the project folder"
        );
        *current = Some(watcher);
        Ok(())
    }
}

/// Store listener hook: when `projectPath` changes while the watcher is on, retarget it. Runs
/// with the store locked, so the new value is taken from the change and the work is spawned.
pub fn on_store_change<R: Runtime>(app: &AppHandle<R>, key: &str, value: Option<&Value>) {
    if key != "projectPath" {
        return;
    }
    let Some(state) = app.try_state::<WatchState>() else {
        return;
    };
    if !state.wanted() {
        return;
    }
    let root = match value {
        Some(Value::String(s)) if !s.is_empty() => Some(PathBuf::from(s)),
        _ => None,
    };
    let generation = state.next_generation();
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = handle
            .state::<WatchState>()
            .retarget(&handle, root, generation)
        {
            tracing::warn!(category = "workspace-files", error = %e, "Can't watch the project folder");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_is_wanted_while_any_window_watches() {
        let s = WatchState::default();
        assert!(!s.wanted());
        s.add_watcher("main");
        s.add_watcher("file-viewer-abc");
        // Watching twice from one window counts once.
        s.add_watcher("main");
        assert!(s.wanted());
        s.remove_watcher("main").unwrap();
        assert!(s.wanted(), "the file viewer still watches");
        // Unknown and repeated removals are no-ops.
        s.remove_watcher("main").unwrap();
        s.remove_watcher("task-history").unwrap();
        assert!(s.wanted());
        let before = s.generation.load(Ordering::SeqCst);
        s.remove_watcher("file-viewer-abc").unwrap();
        assert!(!s.wanted());
        // Stopping invalidates retargets still in flight.
        assert!(s.generation.load(Ordering::SeqCst) > before);
        assert!(s.current.lock().unwrap().is_none());
    }
}
