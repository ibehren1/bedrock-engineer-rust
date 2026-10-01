//! Workspace folders for Code Interpreter runs. Port of `FileManager.ts`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::json;

use super::logger::ToolLogger;
use super::types::WorkspaceConfig;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

pub struct FileManager {
    logger: Arc<dyn ToolLogger>,
    config: WorkspaceConfig,
    workspace_path: Mutex<Option<PathBuf>>,
}

/// `YYYYMMDD` in local time.
fn date_string() -> String {
    chrono::Local::now().format("%Y%m%d").to_string()
}

impl FileManager {
    pub fn new(logger: Arc<dyn ToolLogger>, config: WorkspaceConfig) -> Self {
        Self {
            logger,
            config,
            workspace_path: Mutex::new(None),
        }
    }

    fn workspaces_dir(&self) -> PathBuf {
        Path::new(&self.config.base_path)
            .join(".bedrock-engineer")
            .join("workspaces")
    }

    /// Create `<base>/.bedrock-engineer/workspaces/workspace-<YYYYMMDD>-<sessionId>`.
    pub fn initialize_workspace(&self) -> Result<()> {
        let workspaces = self.workspaces_dir();
        let session_dir = workspaces.join(format!(
            "workspace-{}-{}",
            date_string(),
            self.config.session_id
        ));
        let created = std::fs::create_dir_all(&workspaces)
            .and_then(|_| std::fs::create_dir_all(&session_dir));
        if let Err(error) = created {
            self.logger.error(
                "Failed to initialize workspace",
                json!({ "error": error.to_string() }),
            );
            return Err(Error::msg(format!(
                "Failed to initialize workspace: {error}"
            )));
        }
        self.logger.debug(
            "Workspace initialized",
            json!({
                "path": session_dir.to_string_lossy(),
                "sessionId": self.config.session_id,
                "workspacesDir": workspaces.to_string_lossy(),
            }),
        );
        *self.workspace_path.lock().unwrap() = Some(session_dir);
        Ok(())
    }

    pub fn get_workspace_path(&self) -> Result<PathBuf> {
        self.workspace_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Error::msg("Workspace not initialized"))
    }

    /// Files in the workspace; empty on any error.
    pub fn list_files(&self) -> Vec<FileInfo> {
        let Ok(workspace) = self.get_workspace_path() else {
            self.logger.error(
                "Failed to list files",
                json!({ "error": "Workspace not initialized" }),
            );
            return Vec::new();
        };
        let entries = match std::fs::read_dir(&workspace) {
            Ok(entries) => entries,
            Err(error) => {
                self.logger.error(
                    "Failed to list files",
                    json!({ "error": error.to_string() }),
                );
                return Vec::new();
            }
        };
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            match std::fs::metadata(entry.path()) {
                Ok(meta) => files.push(FileInfo {
                    kind: if meta.is_dir() {
                        FileKind::Directory
                    } else {
                        FileKind::File
                    },
                    size: meta.is_file().then_some(meta.len()),
                    modified: meta.modified().ok(),
                    name,
                }),
                Err(error) => self.logger.warn(
                    "Failed to get file stats",
                    json!({ "filename": name, "error": error.to_string() }),
                ),
            }
        }
        // readdir order is platform-defined; Node returns names sorted on common filesystems.
        files.sort_by(|a, b| a.name.cmp(&b.name));
        files
    }

    /// Remove this session's workspace.
    pub fn cleanup(&self) {
        let Some(path) = self.workspace_path.lock().unwrap().clone() else {
            return;
        };
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                self.logger.debug(
                    "Workspace cleaned up",
                    json!({ "path": path.to_string_lossy(), "sessionId": self.config.session_id }),
                );
                *self.workspace_path.lock().unwrap() = None;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                *self.workspace_path.lock().unwrap() = None;
            }
            Err(error) => self.logger.warn(
                "Failed to cleanup workspace",
                json!({ "path": path.to_string_lossy(), "error": error.to_string() }),
            ),
        }
    }

    /// Remove `workspace-*` folders older than `older_than_hours`, except the current one.
    pub fn cleanup_old_workspaces(&self, older_than_hours: u64) {
        let dir = self.workspaces_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(older_than_hours * 3600))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let current = self.workspace_path.lock().unwrap().clone();
        let mut cleaned = 0;
        for entry in entries.flatten() {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_dir || !name.starts_with("workspace-") {
                continue;
            }
            let path = entry.path();
            let modified = std::fs::metadata(&path).and_then(|m| m.modified());
            match modified {
                Ok(modified) if modified < cutoff && current.as_deref() != Some(path.as_path()) => {
                    match std::fs::remove_dir_all(&path) {
                        Ok(()) => cleaned += 1,
                        Err(error) => self.logger.warn(
                            "Failed to clean up old workspace",
                            json!({ "path": path.to_string_lossy(), "error": error.to_string() }),
                        ),
                    }
                }
                Ok(_) => {}
                Err(error) => self.logger.warn(
                    "Failed to clean up old workspace",
                    json!({ "path": path.to_string_lossy(), "error": error.to_string() }),
                ),
            }
        }
        if cleaned > 0 {
            self.logger.info(
                "Old workspaces cleanup completed",
                json!({ "cleanedCount": cleaned, "olderThanHours": older_than_hours }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::logger::LogFacadeLogger;

    fn manager(base: &Path) -> FileManager {
        FileManager::new(
            Arc::new(LogFacadeLogger),
            WorkspaceConfig {
                base_path: base.to_string_lossy().into_owned(),
                session_id: "session_1_abc".into(),
                max_files: 20,
                max_file_size: 1024 * 1024,
                cleanup_on_exit: true,
            },
        )
    }

    #[test]
    fn creates_lists_and_cleans_a_workspace() {
        let base = tempfile::tempdir().unwrap();
        let files = manager(base.path());
        assert!(files.get_workspace_path().is_err());

        files.initialize_workspace().unwrap();
        let workspace = files.get_workspace_path().unwrap();
        let name = workspace
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("workspace-") && name.ends_with("-session_1_abc"));
        assert!(workspace.starts_with(base.path().join(".bedrock-engineer").join("workspaces")));

        std::fs::write(workspace.join("out.csv"), "a,b").unwrap();
        std::fs::create_dir(workspace.join("data")).unwrap();
        let listed = files.list_files();
        assert_eq!(listed.len(), 2);
        let csv = listed.iter().find(|f| f.name == "out.csv").unwrap();
        assert_eq!(csv.kind, FileKind::File);
        assert_eq!(csv.size, Some(3));
        assert_eq!(listed.iter().find(|f| f.name == "data").unwrap().size, None);

        files.cleanup_old_workspaces(0);
        assert!(workspace.exists(), "the current workspace is never swept");

        files.cleanup();
        assert!(!workspace.exists());
        assert!(files.get_workspace_path().is_err());
    }
}
