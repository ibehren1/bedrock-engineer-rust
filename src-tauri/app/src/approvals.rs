//! User approvals the renderer can't forge: what the user allowed is kept in a file under the
//! app data folder that no IPC command reads or writes, and anything new is asked in a native
//! dialog that Rust opens itself. Used for stdio MCP server launches ([`crate::mcp_approval`])
//! and host commands ([`crate::host_command_approval`]).
//!
//! A "Don't allow" is remembered until the app quits, so a model or page that keeps retrying
//! doesn't ask the user again and again; restarting the app asks again.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// An unanswered dialog counts as "Don't allow" after this long (e.g. nobody at the computer).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovedFile {
    /// Opaque keys (hashes) of everything approved.
    approved: BTreeSet<String>,
}

/// A hex sha256 of `parts`, each length-prefixed so no two different lists collide.
pub fn approval_key(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One kind of approval, persisted in `<userData>/<file name>`.
pub struct ApprovalStore {
    app: AppHandle,
    file: PathBuf,
    approved: Mutex<BTreeSet<String>>,
    denied: Mutex<BTreeSet<String>>,
    /// One dialog at a time; a waiter re-checks once it gets its turn, so the same request
    /// arriving twice at once asks once.
    asking: tokio::sync::Mutex<()>,
}

/// What to show when the user has to be asked.
pub struct Question {
    pub title: String,
    pub text: String,
}

impl ApprovalStore {
    pub fn open(app: AppHandle, user_data: &Path, file_name: &str) -> Self {
        let file = user_data.join(file_name);
        let approved = match std::fs::read(&file) {
            Ok(bytes) => serde_json::from_slice::<ApprovedFile>(&bytes)
                .map(|f| f.approved)
                .unwrap_or_else(|e| {
                    tracing::warn!(file = %file.display(), error = %e, "Unreadable approvals file; everything will ask again");
                    BTreeSet::new()
                }),
            Err(_) => BTreeSet::new(),
        };
        Self {
            app,
            file,
            approved: Mutex::new(approved),
            denied: Mutex::new(BTreeSet::new()),
            asking: tokio::sync::Mutex::new(()),
        }
    }

    /// `Some` when `key` was already allowed (saved) or refused (this run).
    pub fn decided(&self, key: &str) -> Option<bool> {
        if self.approved.lock().unwrap().contains(key) {
            return Some(true);
        }
        if self.denied.lock().unwrap().contains(key) {
            return Some(false);
        }
        None
    }

    /// The saved or this-run decision for `key`, else the user's answer to `question`
    /// (remembered). With `may_ask` false an undecided key is refused without asking.
    pub async fn check(
        &self,
        key: &str,
        may_ask: bool,
        question: impl FnOnce() -> Question,
    ) -> bool {
        if let Some(decided) = self.decided(key) {
            return decided;
        }
        if !may_ask {
            return false;
        }
        let _turn = self.asking.lock().await;
        if let Some(decided) = self.decided(key) {
            return decided;
        }
        let allowed = self.ask(question()).await;
        self.remember(key, allowed);
        allowed
    }

    fn remember(&self, key: &str, allowed: bool) {
        if !allowed {
            self.denied.lock().unwrap().insert(key.to_string());
            return;
        }
        let snapshot = {
            let mut approved = self.approved.lock().unwrap();
            approved.insert(key.to_string());
            ApprovedFile {
                approved: approved.clone(),
            }
        };
        let result = serde_json::to_vec_pretty(&snapshot)
            .map_err(std::io::Error::other)
            .and_then(|bytes| common::json_file::write_atomic(&self.file, &bytes));
        if let Err(e) = result {
            tracing::warn!(file = %self.file.display(), error = %e, "Could not save an approval; it will ask again next time");
        }
    }

    async fn ask(&self, question: Question) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut dialog = self
            .app
            .dialog()
            .message(question.text)
            .title(question.title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Allow".into(),
                "Don't allow".into(),
            ));
        if let Some(main) = self.app.get_webview_window("main") {
            dialog = dialog.parent(&main);
        }
        dialog.show(move |allowed| {
            let _ = tx.send(allowed);
        });
        matches!(tokio::time::timeout(ANSWER_TIMEOUT, rx).await, Ok(Ok(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_do_not_collide_across_part_boundaries() {
        assert_ne!(approval_key(&["ab", "c"]), approval_key(&["a", "bc"]));
        assert_eq!(approval_key(&["a", "b"]), approval_key(&["a", "b"]));
    }
}
