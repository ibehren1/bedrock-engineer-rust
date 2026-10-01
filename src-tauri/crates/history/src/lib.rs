//! Chat history — port of `src/main/store/chatSession.ts` (`ChatSessionManager`), which backed
//! `window.chatHistory` in the Electron preload.
//!
//! On-disk format (unchanged, so Electron history loads as-is):
//!
//! - `<userData>/chat-sessions/<sessionId>.json` — one `ChatSession` per file,
//!   `JSON.stringify(session, null, 2)` (two-space indent, no trailing newline).
//! - `<userData>/chat-sessions-meta.json` — an electron-store file (tab indent) with
//!   `{ recentSessions: string[], metadata: { [id]: SessionMetadata }, activeSessionId? }`.
//!
//! Sessions and messages are handled as JSON values rather than typed structs, so fields this
//! port doesn't know about (message metadata, tool state, future additions) and key order survive
//! a read-modify-write untouched.
//!
//! Error behaviour follows the TS: read failures make a session "not found", failed writes and
//! deletes are logged and otherwise ignored, and invalid message indexes are logged no-ops.
//!
//! Data safety beyond the TS:
//!
//! - Files are parsed like `JSON.parse` (lone UTF-16 surrogate escapes are accepted), so a title
//!   with half an emoji doesn't make a session unreadable.
//! - Metadata updates are single read-modify-writes of the current file; when the metadata file
//!   can't be read, nothing is written (see [`meta`]).
//! - Session ids must be plain file names ([`common::json_file::is_safe_file_id`]); any other id
//!   is treated as an unknown session, so it can never address a file outside `chat-sessions/`.

use common::json_file::{is_safe_file_id, parse_lenient};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod meta;
pub use meta::MetaFile;

const CATEGORY: &str = "chat:history";

/// Directory under userData holding one JSON file per session.
pub const SESSIONS_DIR: &str = "chat-sessions";
/// electron-store name (`<userData>/<name>.json`) of the metadata file.
pub const META_FILE: &str = "chat-sessions-meta.json";
/// `updateRecentSessions` keeps this many ids.
pub const MAX_RECENT: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// `Date.now()`.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// `Chat ${new Date().toLocaleString()}` for the en-US locale (`9/30/2026, 3:04:05 PM`). The
/// renderer passes its own locale's string when it can; this is the fallback.
pub fn default_session_title() -> String {
    format!(
        "Chat {}",
        chrono::Local::now().format("%-m/%-d/%Y, %-I:%M:%S %p")
    )
}

/// What `chat_history_snapshot` returns to hydrate the renderer's synchronous getters.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub active_session_id: Option<String>,
    pub metadata: Vec<Value>,
    pub recent: Vec<Value>,
    /// Full sessions for the active session and the recent list. Other sessions are fetched on
    /// demand (see BRIDGE.md).
    pub sessions: Map<String, Value>,
}

/// `ChatSessionManager`.
#[derive(Debug)]
pub struct ChatSessionManager {
    sessions_dir: PathBuf,
    meta: MetaFile,
}

impl ChatSessionManager {
    /// `new ChatSessionManager()` with `store.get('userDataPath')` = `user_data_path`.
    pub fn new(user_data_path: &Path) -> Result<Self> {
        let sessions_dir = user_data_path.join(SESSIONS_DIR);
        fs::create_dir_all(&sessions_dir)?;
        let defaults = json!({ "recentSessions": [], "metadata": {} });
        let meta = MetaFile::open(
            user_data_path.join(META_FILE),
            defaults.as_object().expect("object"),
        )?;
        let manager = Self { sessions_dir, meta };
        manager.initialize_metadata();
        Ok(manager)
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// On first run (or when the metadata is empty), rebuild it from the session files.
    fn initialize_metadata(&self) {
        match self.meta.try_read() {
            Ok(m)
                if m.get("metadata")
                    .and_then(Value::as_object)
                    .is_some_and(|m| !m.is_empty()) =>
            {
                return
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(category = CATEGORY, error = %e, "Error initializing metadata");
                return;
            }
        }
        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(category = CATEGORY, error = %e, "Error initializing metadata");
                return;
            }
        };
        let mut files: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|f| f.ends_with(".json"))
            .collect();
        files.sort();
        let mut rebuilt = Map::new();
        for file in files {
            // `file.replace('.json', '')` replaces the first occurrence.
            let session_id = file.replacen(".json", "", 1);
            if let Some(session) = self.read_session_file(&session_id) {
                rebuilt.insert(session_id, session_metadata(&session));
            }
        }
        let result = self.meta.update_key("metadata", |current| {
            let mut metadata = as_map(current);
            metadata.extend(rebuilt);
            Value::Object(metadata)
        });
        if let Err(e) = result {
            tracing::error!(category = CATEGORY, error = %e, "Error initializing metadata");
            return;
        }
        tracing::info!(category = CATEGORY, "Metadata initialized successfully");
    }

    /// `<sessions dir>/<id>.json`, or `None` (logged) for an id that isn't a plain file name.
    fn session_file_path(&self, session_id: &str) -> Option<PathBuf> {
        if !is_safe_file_id(session_id) {
            tracing::error!(
                category = CATEGORY,
                session_id,
                "Rejected invalid session id"
            );
            return None;
        }
        Some(self.sessions_dir.join(format!("{session_id}.json")))
    }

    fn read_session_file(&self, session_id: &str) -> Option<Value> {
        let path = self.session_file_path(session_id)?;
        // A file that fails to parse is left where it is (and never written over: every writer
        // reads the session first and gives up when that fails).
        let parsed = fs::read_to_string(&path)
            .map_err(Error::from)
            .and_then(|s| parse_lenient::<Value>(&s).map_err(Error::from));
        match parsed {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!(category = CATEGORY, session_id, error = %e, "Error reading session file");
                None
            }
        }
    }

    fn write_session_file(&self, session_id: &str, session: &Value) {
        let Some(path) = self.session_file_path(session_id) else {
            return;
        };
        let result = meta::to_json(session, b"  ").and_then(|b| meta::write_atomic(&path, &b));
        if let Err(e) = result {
            tracing::error!(category = CATEGORY, session_id, error = %e, "Error writing session file");
        }
    }

    fn metadata(&self) -> Map<String, Value> {
        match self.meta.get("metadata") {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }

    fn recent_ids(&self) -> Vec<String> {
        match self.meta.get("recentSessions") {
            Some(Value::Array(a)) => a
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// One read-modify-write of a metadata key; logged and skipped when the file can't be read.
    fn update_meta(&self, key: &str, f: impl FnOnce(Option<Value>) -> Value) {
        if let Err(e) = self.meta.update_key(key, f) {
            tracing::error!(category = CATEGORY, key, error = %e, "Error writing chat session metadata");
        }
    }

    fn update_metadata(&self, session_id: &str, session: &Value) {
        let entry = session_metadata(session);
        self.update_meta("metadata", |current| {
            let mut m = as_map(current);
            m.insert(session_id.to_string(), entry);
            Value::Object(m)
        });
    }

    fn remove_from_recent(&self, ids: &HashSet<&str>) {
        self.update_meta("recentSessions", |current| {
            let recent: Vec<String> = as_ids(current)
                .into_iter()
                .filter(|id| !ids.contains(id.as_str()))
                .collect();
            json!(recent)
        });
    }

    fn update_recent_sessions(&self, session_id: &str) {
        self.update_meta("recentSessions", |current| {
            let mut updated = vec![session_id.to_string()];
            updated.extend(as_ids(current).into_iter().filter(|id| id != session_id));
            updated.truncate(MAX_RECENT);
            json!(updated)
        });
    }

    /// `createSession(agentId, modelId, systemPrompt?)` → the new session id.
    pub fn create_session(
        &self,
        agent_id: &str,
        model_id: &str,
        system_prompt: Option<&str>,
        title: Option<&str>,
    ) -> String {
        let id = format!("session_{}", now_ms());
        let mut session = Map::new();
        session.insert("id".into(), json!(id));
        session.insert(
            "title".into(),
            json!(title
                .map(str::to_owned)
                .unwrap_or_else(default_session_title)),
        );
        session.insert("createdAt".into(), json!(now_ms()));
        session.insert("updatedAt".into(), json!(now_ms()));
        session.insert("messages".into(), json!([]));
        session.insert("agentId".into(), json!(agent_id));
        session.insert("modelId".into(), json!(model_id));
        if let Some(p) = system_prompt {
            session.insert("systemPrompt".into(), json!(p));
        }
        let session = Value::Object(session);
        self.write_session_file(&id, &session);
        self.update_metadata(&id, &session);
        self.update_recent_sessions(&id);
        id
    }

    /// `addMessage`: append, bump `updatedAt`, move the session to the front of the recent list.
    /// No-op for an unknown session.
    pub fn add_message(&self, session_id: &str, message: Value) {
        let Some(mut session) = self.read_session_file(session_id) else {
            return;
        };
        let Some(messages) = messages_mut(&mut session) else {
            return;
        };
        messages.push(message);
        set_updated_at(&mut session);
        self.write_session_file(session_id, &session);
        self.update_metadata(session_id, &session);
        self.update_recent_sessions(session_id);
    }

    /// `getSession`: the parsed session file, or `None` if it's missing or unreadable.
    pub fn get_session(&self, session_id: &str) -> Option<Value> {
        self.read_session_file(session_id)
    }

    /// `updateSessionTitle` (does not touch `updatedAt` or the recent list).
    pub fn update_session_title(&self, session_id: &str, title: &str) {
        let Some(mut session) = self.read_session_file(session_id) else {
            return;
        };
        if let Some(obj) = session.as_object_mut() {
            obj.insert("title".into(), json!(title));
        }
        self.write_session_file(session_id, &session);
        self.update_metadata(session_id, &session);
    }

    /// `deleteSession`. The metadata entry is only removed when the file could be deleted; the id
    /// is always dropped from the recent list. The active session id is left alone.
    pub fn delete_session(&self, session_id: &str) {
        let Some(path) = self.session_file_path(session_id) else {
            return;
        };
        match fs::remove_file(path) {
            Ok(()) => self.update_meta("metadata", |current| {
                let mut m = as_map(current);
                m.shift_remove(session_id);
                Value::Object(m)
            }),
            Err(e) => {
                tracing::error!(category = CATEGORY, session_id, error = %e, "Error deleting session file");
            }
        }
        self.remove_from_recent(&HashSet::from([session_id]));
    }

    /// `deleteSessions`: bulk delete with one metadata write; clears the active session id if it
    /// was among them.
    pub fn delete_sessions(&self, session_ids: &[String]) {
        if session_ids.is_empty() {
            return;
        }
        let mut seen = HashSet::new();
        let ids: Vec<&String> = session_ids
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .collect();
        for id in &ids {
            let Some(path) = self.session_file_path(id) else {
                continue;
            };
            if let Err(e) = fs::remove_file(path) {
                tracing::error!(category = CATEGORY, session_id = %id, error = %e, "Error deleting session file");
            }
        }
        self.update_meta("metadata", |current| {
            let mut m = as_map(current);
            for id in &ids {
                m.shift_remove(id.as_str());
            }
            Value::Object(m)
        });
        self.remove_from_recent(&seen);

        if let Some(active) = self.get_active_session_id() {
            if seen.contains(active.as_str()) {
                if let Err(e) = self.meta.delete("activeSessionId") {
                    tracing::error!(category = CATEGORY, error = %e, "Error writing chat session metadata");
                }
            }
        }
    }

    /// `deleteAllSessions`: remove every `*.json` in the sessions directory and reset the metadata.
    pub fn delete_all_sessions(&self) {
        let result = (|| -> Result<()> {
            for entry in fs::read_dir(&self.sessions_dir)? {
                let entry = entry?;
                if entry.file_name().to_string_lossy().ends_with(".json") {
                    fs::remove_file(entry.path())?;
                }
            }
            self.meta.set("metadata", json!({}))?;
            self.meta.set("recentSessions", json!([]))?;
            self.meta.delete("activeSessionId")?;
            Ok(())
        })();
        match result {
            Ok(()) => tracing::info!(
                category = CATEGORY,
                "All sessions have been deleted successfully"
            ),
            Err(e) => {
                tracing::error!(category = CATEGORY, error = %e, "Error deleting all sessions")
            }
        }
    }

    /// `getRecentSessions`: metadata for the recent ids whose file exists and that have messages.
    pub fn get_recent_sessions(&self) -> Vec<Value> {
        let metadata = self.metadata();
        self.recent_ids()
            .iter()
            .filter_map(|id| metadata.get(id))
            .filter(|m| self.meta_file_exists(m) && has_messages(m))
            .cloned()
            .collect()
    }

    /// `getAllSessionMetadata`: sessions whose file exists and that have messages, newest
    /// `updatedAt` first.
    pub fn get_all_session_metadata(&self) -> Vec<Value> {
        let mut list: Vec<Value> = self
            .metadata()
            .into_iter()
            .map(|(_, m)| m)
            .filter(|m| self.meta_file_exists(m) && has_messages(m))
            .collect();
        let updated = |m: &Value| {
            m.get("updatedAt")
                .and_then(Value::as_f64)
                .unwrap_or(f64::NAN)
        };
        list.sort_by(|a, b| {
            updated(b)
                .partial_cmp(&updated(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }

    fn meta_file_exists(&self, meta: &Value) -> bool {
        meta.get("id")
            .and_then(Value::as_str)
            .filter(|id| is_safe_file_id(id))
            .is_some_and(|id| self.sessions_dir.join(format!("{id}.json")).exists())
    }

    /// `setActiveSession`. electron-store throws on `set(key, undefined)`; `None` clears the key
    /// instead.
    pub fn set_active_session(&self, session_id: Option<&str>) {
        let result = match session_id {
            Some(id) => self.meta.set("activeSessionId", json!(id)),
            None => self.meta.delete("activeSessionId"),
        };
        if let Err(e) = result {
            tracing::error!(category = CATEGORY, error = %e, "Error writing chat session metadata");
        }
    }

    pub fn get_active_session_id(&self) -> Option<String> {
        self.meta
            .get("activeSessionId")
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    /// `updateMessageContent`: replace the message at `message_index` and bump `updatedAt`
    /// (the recent list is not touched).
    pub fn update_message_content(&self, session_id: &str, message_index: i64, updated: Value) {
        self.edit_message(session_id, message_index, |messages, i| {
            messages[i] = updated
        });
    }

    /// `deleteMessage`: remove the message at `message_index` and bump `updatedAt`.
    pub fn delete_message(&self, session_id: &str, message_index: i64) {
        self.edit_message(session_id, message_index, |messages, i| {
            messages.remove(i);
        });
    }

    fn edit_message(
        &self,
        session_id: &str,
        message_index: i64,
        edit: impl FnOnce(&mut Vec<Value>, usize),
    ) {
        let Some(mut session) = self.read_session_file(session_id) else {
            return;
        };
        let Some(messages) = messages_mut(&mut session) else {
            return;
        };
        if message_index < 0 || message_index as usize >= messages.len() {
            tracing::error!(category = CATEGORY, message_index, "Invalid message index");
            return;
        }
        edit(messages, message_index as usize);
        set_updated_at(&mut session);
        self.write_session_file(session_id, &session);
        self.update_metadata(session_id, &session);
    }

    /// Everything the renderer's synchronous getters need at startup.
    pub fn snapshot(&self) -> Snapshot {
        let active_session_id = self.get_active_session_id();
        let metadata = self.get_all_session_metadata();
        let recent = self.get_recent_sessions();
        let mut sessions = Map::new();
        let ids = active_session_id.iter().cloned().chain(
            recent
                .iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_owned)),
        );
        for id in ids {
            if sessions.contains_key(&id) {
                continue;
            }
            if let Some(s) = self.get_session(&id) {
                sessions.insert(id, s);
            }
        }
        Snapshot {
            active_session_id,
            metadata,
            recent,
            sessions,
        }
    }
}

/// `updateMetadata`'s `SessionMetadata` object for a session (absent fields stay absent, the way
/// `JSON.stringify` drops `undefined`).
pub fn session_metadata(session: &Value) -> Value {
    let mut m = Map::new();
    let copy = |m: &mut Map<String, Value>, k: &str| {
        if let Some(v) = session.get(k) {
            m.insert(k.to_string(), v.clone());
        }
    };
    for k in ["id", "title", "createdAt", "updatedAt"] {
        copy(&mut m, k);
    }
    let count = session
        .get("messages")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    m.insert("messageCount".into(), json!(count));
    for k in ["agentId", "modelId", "systemPrompt"] {
        copy(&mut m, k);
    }
    Value::Object(m)
}

fn as_map(v: Option<Value>) -> Map<String, Value> {
    match v {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn as_ids(v: Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn has_messages(meta: &Value) -> bool {
    meta.get("messageCount")
        .and_then(Value::as_f64)
        .is_some_and(|n| n > 0.0)
}

fn messages_mut(session: &mut Value) -> Option<&mut Vec<Value>> {
    let obj = session.as_object_mut()?;
    if !obj.get("messages").is_some_and(Value::is_array) {
        tracing::error!(category = CATEGORY, "session file has no messages array");
        return None;
    }
    obj.get_mut("messages").and_then(Value::as_array_mut)
}

fn set_updated_at(session: &mut Value) {
    if let Some(obj) = session.as_object_mut() {
        obj.insert("updatedAt".into(), json!(now_ms()));
    }
}

/// `readChatTitle` (`src/main/lib/chatSessionTitle.ts`): a chat's current title straight from its
/// session file, for naming folders after the chat without going through the renderer.
pub fn read_chat_title(user_data_path: &Path, session_id: &str) -> Option<String> {
    if !is_safe_file_id(session_id) {
        return None;
    }
    let file = user_data_path
        .join(SESSIONS_DIR)
        .join(format!("{session_id}.json"));
    let text = fs::read_to_string(file).ok()?;
    let parsed: Value = parse_lenient(&text).ok()?;
    parsed.get("title")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests;
