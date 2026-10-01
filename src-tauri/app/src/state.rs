//! Managed state shared by the command modules.

use history::ChatSessionManager;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use store::Store;

/// Paths resolved once at startup.
pub struct AppPaths {
    /// Electron's `app.getPath('userData')` (the directory holding config.json).
    #[allow(dead_code)]
    pub user_data: PathBuf,
}

/// Keeps the file logger's writer alive for the life of the app.
pub struct LoggerState(#[allow(dead_code)] pub Option<common::logger::LoggerGuard>);

/// The chat history manager. Every method does its own file I/O, so calls are serialized to
/// keep read-modify-write sequences on the metadata file from interleaving.
///
/// When the history could not be opened at startup (e.g. the metadata file can't be read),
/// the app still starts; every history command then fails with the startup error, and the
/// files on disk are left untouched.
pub struct HistoryState(Result<Mutex<ChatSessionManager>, String>);

impl HistoryState {
    pub fn new(manager: ChatSessionManager) -> Self {
        Self(Ok(Mutex::new(manager)))
    }

    /// History that failed to open with `error`.
    pub fn unavailable(error: impl std::fmt::Display) -> Self {
        Self(Err(format!("Chat history is unavailable: {error}")))
    }

    /// `ChatSessionManager::new`, or [`HistoryState::unavailable`] (logged) on failure.
    pub fn open(user_data: &std::path::Path) -> Self {
        match ChatSessionManager::new(user_data) {
            Ok(manager) => Self::new(manager),
            Err(e) => {
                tracing::error!(
                    user_data_path = %user_data.display(),
                    error = %e,
                    "Failed to open chat history; history commands will fail until restart"
                );
                Self::unavailable(e)
            }
        }
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, ChatSessionManager>, String> {
        self.0
            .as_ref()
            .map_err(Clone::clone)?
            .lock()
            .map_err(|e| format!("chat history lock poisoned: {e}"))
    }
}

pub type StoreMutex = Mutex<Store>;

pub fn lock_store(s: &StoreMutex) -> Result<MutexGuard<'_, Store>, String> {
    s.lock().map_err(|e| format!("store lock poisoned: {e}"))
}

/// `store.get('projectPath')` when it is a non-empty string.
pub fn project_path(s: &StoreMutex) -> Option<PathBuf> {
    let store = lock_store(s).ok()?;
    match store.get("projectPath") {
        Some(Value::String(p)) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => None,
    }
}

/// `store.get('projectPath') || process.cwd()`.
pub fn project_path_or_cwd(s: &StoreMutex) -> PathBuf {
    project_path(s).unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// `store.all()`, read fresh for each command (`{}` if the lock is poisoned).
pub fn store_all(s: &StoreMutex) -> Value {
    lock_store(s)
        .map(|s| s.all())
        .unwrap_or_else(|_| Value::Object(Default::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_that_fails_to_open_reports_errors_and_leaves_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        // A directory where the metadata file should be: a real read error, not "missing".
        let meta = dir.path().join(history::META_FILE);
        std::fs::create_dir(&meta).unwrap();
        std::fs::write(meta.join("keep"), "data").unwrap();

        let state = HistoryState::open(dir.path());
        let error = state.lock().expect_err("history should be unavailable");
        assert!(
            error.starts_with("Chat history is unavailable: "),
            "{error}"
        );
        assert!(state.lock().is_err());
        assert!(meta.is_dir());
        assert_eq!(std::fs::read_to_string(meta.join("keep")).unwrap(), "data");

        let ok = tempfile::tempdir().unwrap();
        assert!(HistoryState::open(ok.path()).lock().is_ok());
    }
}
