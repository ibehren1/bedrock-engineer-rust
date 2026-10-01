//! `BackgroundChatSessionManager.ts`: background-agent sessions persisted as JSON files.
//!
//! On-disk format (same as Electron, so existing sessions load):
//!
//! * `<userDataPath>/background-agent-sessions/<sessionId>.json` — one `BackgroundChatSession`
//!   (`JSON.stringify(session, null, 2)`): `sessionId`, `taskId?`, `agentId`, `modelId`,
//!   `projectDirectory?`, `createdAt`, `updatedAt`, `messages`, `executionMetadata?`. Written
//!   atomically through `<file>.tmp-<uuid>` with up to three attempts.
//! * `<store dir>/background-agent-sessions-meta.json` — electron-store file
//!   `{ "metadata": { [sessionId]: BackgroundSessionMetadata } }` (tab-indented).
//!
//! Empty, unparsable or structurally invalid session files are dropped from the metadata when
//! read, as in TS, but the file itself is renamed to `<id>.json.corrupt-<unix-ms>` instead of
//! deleted. Files are parsed like `JSON.parse` (lone surrogate escapes are accepted). A session
//! file that can't be read for another reason (permissions, ...) is left alone. Session ids must
//! be plain file names ([`common::json_file::is_safe_file_id`]); others address no file.
//! Messages are kept as raw JSON, so fields this version does not model survive a rewrite.

use crate::config::JsonFile;
use crate::error::{Error, Result};
use crate::types::de_ms;
use agents::{AgentMessage, SessionMeta, SessionStore};
use common::json_file::{is_safe_file_id, parse_lenient, quarantine, write_atomic};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Sessions directory name under `userDataPath`.
pub const SESSIONS_DIR: &str = "background-agent-sessions";
/// electron-store name of the metadata file.
pub const METADATA_STORE: &str = "background-agent-sessions-meta";

/// `BackgroundSessionMetadata`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSessionMetadata {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
    #[serde(default, deserialize_with = "de_ms")]
    pub created_at: i64,
    #[serde(default, deserialize_with = "de_ms")]
    pub updated_at: i64,
    #[serde(default)]
    pub message_count: usize,
    /// `'scheduled'` when the session has a `taskId`, else `'manual'`.
    #[serde(default)]
    pub execution_type: String,
}

/// `getSessionStats(sessionId)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub exists: bool,
    pub message_count: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BackgroundSessionMetadata>,
}

/// `getAllSessionStats()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllSessionStats {
    pub total_sessions: usize,
    pub total_messages: usize,
    pub average_messages_per_session: f64,
}

/// `BackgroundChatSession.executionMetadata`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionMetadata {
    pub executed_at: i64,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn as_map(v: Option<Value>) -> Map<String, Value> {
    match v {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// JS truthiness for the structure check.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn str_field(session: &Map<String, Value>, key: &str) -> Option<String> {
    session.get(key).and_then(Value::as_str).map(str::to_string)
}

fn num_field(session: &Map<String, Value>, key: &str) -> i64 {
    session
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

fn messages_of(session: &Map<String, Value>) -> &[Value] {
    session
        .get("messages")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A stored message as the engine's [`AgentMessage`], tolerating missing fields.
fn to_agent_message(v: &Value) -> AgentMessage {
    serde_json::from_value(v.clone()).unwrap_or_else(|_| AgentMessage {
        id: v
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        role: v
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        content: v
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        timestamp: v.get("timestamp").and_then(Value::as_i64).unwrap_or(0),
        metadata: v.get("metadata").cloned(),
    })
}

fn metadata_of(session_id: &str, session: &Map<String, Value>) -> BackgroundSessionMetadata {
    let task_id = str_field(session, "taskId");
    BackgroundSessionMetadata {
        session_id: str_field(session, "sessionId").unwrap_or_else(|| session_id.to_string()),
        execution_type: if task_id.as_deref().is_some_and(|t| !t.is_empty()) {
            "scheduled"
        } else {
            "manual"
        }
        .to_string(),
        task_id,
        agent_id: str_field(session, "agentId").unwrap_or_default(),
        model_id: str_field(session, "modelId").unwrap_or_default(),
        project_directory: str_field(session, "projectDirectory"),
        created_at: num_field(session, "createdAt"),
        updated_at: num_field(session, "updatedAt"),
        message_count: messages_of(session).len(),
    }
}

/// Persistent session store for background agents; implements [`agents::SessionStore`].
pub struct BackgroundChatSessionManager {
    sessions_dir: PathBuf,
    /// The metadata store; the lock also serializes session file access.
    meta: Mutex<JsonFile>,
}

impl BackgroundChatSessionManager {
    /// Open with sessions under `<user_data_path>/background-agent-sessions` and the metadata
    /// store in `store_dir` (electron-store's `cwd`, the directory of `config.json`). Both are the
    /// Electron `userData` directory in a normal install.
    pub fn open(user_data_path: impl AsRef<Path>, store_dir: impl AsRef<Path>) -> Result<Self> {
        let sessions_dir = user_data_path.as_ref().join(SESSIONS_DIR);
        fs::create_dir_all(&sessions_dir)?;
        let meta = JsonFile::open(store_dir.as_ref(), METADATA_STORE, "metadata", json!({}));
        let manager = BackgroundChatSessionManager {
            sessions_dir,
            meta: Mutex::new(meta),
        };
        manager.initialize_metadata();
        tracing::info!(sessions_dir = %manager.sessions_dir.display(), "BackgroundChatSessionManager initialized");
        Ok(manager)
    }

    /// [`open`](Self::open) with `userDataPath` read from the config store (TS constructor).
    pub fn from_config(
        config: &dyn crate::config::ConfigStore,
        store_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let user_data = config
            .get("userDataPath")
            .and_then(|v| v.as_str().map(str::to_string))
            .filter(|s| !s.is_empty())
            .ok_or(Error::UserDataPathMissing)?;
        Self::open(user_data, store_dir)
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    fn meta_map(meta: &JsonFile) -> Map<String, Value> {
        as_map(meta.get("metadata"))
    }

    /// One read-modify-write of the metadata map; skipped (logged) when the file can't be read.
    fn edit_meta_map(meta: &mut JsonFile, f: impl FnOnce(&mut Map<String, Value>)) {
        let result = meta.update_key("metadata", |current| {
            let mut map = as_map(current);
            f(&mut map);
            (Value::Object(map), ())
        });
        if let Err(e) = result {
            tracing::error!(error = %e, "Failed to write background session metadata");
        }
    }

    fn initialize_metadata(&self) {
        let mut meta = self.meta.lock().unwrap();
        if !Self::meta_map(&meta).is_empty() {
            return;
        }
        let ids = self.list_session_files();
        for id in &ids {
            if let Some(session) = self.read_session_file(&mut meta, id) {
                Self::update_metadata(&mut meta, id, &session);
            }
        }
        tracing::info!(
            session_count = ids.len(),
            "Background session metadata initialized"
        );
    }

    /// `<sessions dir>/<id>.json`, or `None` for an id that isn't a plain file name.
    fn session_file_path(&self, session_id: &str) -> Option<PathBuf> {
        if !is_safe_file_id(session_id) {
            tracing::warn!(session_id, "Rejected invalid background session id");
            return None;
        }
        Some(self.sessions_dir.join(format!("{session_id}.json")))
    }

    fn read_session_file(
        &self,
        meta: &mut JsonFile,
        session_id: &str,
    ) -> Option<Map<String, Value>> {
        let path = self.session_file_path(session_id)?;
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                tracing::warn!(session_id, error = %e, "Background session file is not UTF-8, moving it aside");
                self.remove_corrupted(meta, session_id);
                return None;
            }
            Err(e) => {
                // Unreadable for now (permissions, ...): leave the file and its metadata alone.
                tracing::warn!(session_id, error = %e, "Error reading background session file");
                return None;
            }
        };
        if data.trim().is_empty() {
            tracing::warn!(session_id, "Session file is empty, moving it aside");
            self.remove_corrupted(meta, session_id);
            return None;
        }
        let session = match parse_lenient::<Value>(&data) {
            Ok(Value::Object(m)) => m,
            Ok(_) | Err(_) => {
                tracing::warn!(
                    session_id,
                    "Error reading background session file, moving corrupted file aside"
                );
                self.remove_corrupted(meta, session_id);
                return None;
            }
        };
        if !truthy(session.get("sessionId"))
            || !session.get("messages").is_some_and(Value::is_array)
        {
            tracing::warn!(
                session_id,
                "Session file has invalid structure, moving it aside"
            );
            self.remove_corrupted(meta, session_id);
            return None;
        }
        Some(session)
    }

    fn write_session_file(&self, session_id: &str, session: &Map<String, Value>) -> Result<()> {
        let path = self
            .session_file_path(session_id)
            .ok_or_else(|| Error::Session(format!("Invalid session id: {session_id}")))?;
        let body =
            serde_json::to_string_pretty(session).map_err(|e| Error::Session(e.to_string()))?;
        const MAX_RETRIES: u64 = 3;
        let mut last_error = String::new();
        for attempt in 1..=MAX_RETRIES {
            match write_atomic(&path, body.as_bytes()) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::warn!(session_id, attempt, error = %e, "Error writing background session file");
                    last_error = e.to_string();
                    if attempt < MAX_RETRIES {
                        std::thread::sleep(Duration::from_millis(100 * attempt));
                    }
                }
            }
        }
        Err(Error::Session(format!(
            "Failed to write session file after {MAX_RETRIES} attempts: {last_error}"
        )))
    }

    /// Move a corrupt session file aside (never deleted) and drop its metadata entry.
    fn remove_corrupted(&self, meta: &mut JsonFile, session_id: &str) {
        let Some(path) = self.session_file_path(session_id) else {
            return;
        };
        if let Err(e) = quarantine(&path) {
            tracing::error!(session_id, error = %e, "Failed to move corrupted background session file aside");
            return;
        }
        if Self::meta_map(meta).contains_key(session_id) {
            Self::edit_meta_map(meta, |map| {
                map.remove(session_id);
            });
        }
    }

    fn update_metadata(meta: &mut JsonFile, session_id: &str, session: &Map<String, Value>) {
        let entry = serde_json::to_value(metadata_of(session_id, session)).unwrap_or(Value::Null);
        Self::edit_meta_map(meta, |map| {
            map.insert(session_id.to_string(), entry);
        });
    }

    fn create_locked(
        &self,
        meta: &mut JsonFile,
        session_id: &str,
        options: SessionMeta,
    ) -> Result<()> {
        if self.read_session_file(meta, session_id).is_some() {
            tracing::warn!(session_id, "Background session already exists");
            return Ok(());
        }
        let now = now_ms();
        let mut session = Map::new();
        session.insert("sessionId".into(), json!(session_id));
        if let Some(t) = options.task_id {
            session.insert("taskId".into(), json!(t));
        }
        session.insert(
            "agentId".into(),
            json!(options.agent_id.unwrap_or_default()),
        );
        session.insert(
            "modelId".into(),
            json!(options.model_id.unwrap_or_default()),
        );
        if let Some(p) = options.project_directory {
            session.insert("projectDirectory".into(), json!(p));
        }
        session.insert("createdAt".into(), json!(now));
        session.insert("updatedAt".into(), json!(now));
        session.insert("messages".into(), json!([]));
        self.write_session_file(session_id, &session)?;
        Self::update_metadata(meta, session_id, &session);
        tracing::info!(session_id, "Background session created");
        Ok(())
    }

    /// `createSession(sessionId, options)`; a no-op when a valid session already exists.
    pub fn create_session(&self, session_id: &str, options: SessionMeta) -> Result<()> {
        let mut meta = self.meta.lock().unwrap();
        self.create_locked(&mut meta, session_id, options)
    }

    /// `hasSession` / `hasValidSession`: the file exists and is a valid session.
    pub fn has_session(&self, session_id: &str) -> bool {
        let mut meta = self.meta.lock().unwrap();
        self.read_session_file(&mut meta, session_id).is_some()
    }

    /// `hasValidSession(sessionId)`.
    pub fn has_valid_session(&self, session_id: &str) -> bool {
        self.has_session(session_id)
    }

    /// `addMessage(sessionId, message)`; creates the session when missing.
    pub fn add_message_value(&self, session_id: &str, message: Value) -> Result<()> {
        let mut meta = self.meta.lock().unwrap();
        let mut session = match self.read_session_file(&mut meta, session_id) {
            Some(s) => s,
            None => {
                self.create_locked(&mut meta, session_id, SessionMeta::default())
                    .map_err(|e| {
                        Error::Session(format!("Failed to create session for message: {e}"))
                    })?;
                self.read_session_file(&mut meta, session_id).ok_or_else(|| {
                    Error::Session(format!(
                        "Failed to create session for message: Failed to create session for message: {session_id}"
                    ))
                })?
            }
        };
        if let Some(Value::Array(messages)) = session.get_mut("messages") {
            messages.push(message);
        }
        session.insert("updatedAt".into(), json!(now_ms()));
        self.write_session_file(session_id, &session)
            .map_err(|e| Error::Session(format!("Failed to save message to session: {e}")))?;
        Self::update_metadata(&mut meta, session_id, &session);
        Ok(())
    }

    /// `getHistory(sessionId)` as stored (raw JSON messages); empty for an unknown session.
    pub fn history_values(&self, session_id: &str) -> Vec<Value> {
        let mut meta = self.meta.lock().unwrap();
        self.read_session_file(&mut meta, session_id)
            .map(|s| messages_of(&s).to_vec())
            .unwrap_or_default()
    }

    /// `deleteSession(sessionId)`.
    pub fn delete_session(&self, session_id: &str) -> bool {
        let mut meta = self.meta.lock().unwrap();
        let Some(path) = self.session_file_path(session_id) else {
            return false;
        };
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                tracing::error!(session_id, error = %e, "Error deleting background session");
                return false;
            }
        }
        Self::edit_meta_map(&mut meta, |map| {
            map.remove(session_id);
        });
        tracing::info!(session_id, "Background session deleted");
        true
    }

    fn list_session_files(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.sessions_dir) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.ends_with(".json"))
            .map(|name| name.replacen(".json", "", 1))
            .collect();
        ids.sort();
        ids
    }

    /// `listSessions()`: ids of all `*.json` files in the sessions directory.
    pub fn list_sessions(&self) -> Vec<String> {
        let _guard = self.meta.lock().unwrap();
        self.list_session_files()
    }

    /// `getSessionStats(sessionId)`.
    pub fn get_session_stats(&self, session_id: &str) -> SessionStats {
        let mut meta = self.meta.lock().unwrap();
        let Some(session) = self.read_session_file(&mut meta, session_id) else {
            return SessionStats {
                exists: false,
                message_count: 0,
                user_messages: 0,
                assistant_messages: 0,
                metadata: None,
            };
        };
        let messages = messages_of(&session);
        let count_role = |role: &str| {
            messages
                .iter()
                .filter(|m| m.get("role").and_then(Value::as_str) == Some(role))
                .count()
        };
        SessionStats {
            exists: true,
            message_count: messages.len(),
            user_messages: count_role("user"),
            assistant_messages: count_role("assistant"),
            metadata: Self::meta_map(&meta)
                .get(session_id)
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
        }
    }

    /// `getAllSessionStats()`.
    pub fn get_all_session_stats(&self) -> AllSessionStats {
        let mut meta = self.meta.lock().unwrap();
        let ids = self.list_session_files();
        let total_messages: usize = ids
            .iter()
            .filter_map(|id| self.read_session_file(&mut meta, id))
            .map(|s| messages_of(&s).len())
            .sum();
        let average = if ids.is_empty() {
            0.0
        } else {
            total_messages as f64 / ids.len() as f64
        };
        AllSessionStats {
            total_sessions: ids.len(),
            total_messages,
            average_messages_per_session: (average * 100.0).round() / 100.0,
        }
    }

    /// `getSessionMetadata(sessionId)`.
    pub fn get_session_metadata(&self, session_id: &str) -> Option<BackgroundSessionMetadata> {
        let meta = self.meta.lock().unwrap();
        Self::meta_map(&meta)
            .get(session_id)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// `getAllSessionsMetadata()`: entries whose file exists, most recently updated first.
    pub fn get_all_sessions_metadata(&self) -> Vec<BackgroundSessionMetadata> {
        let meta = self.meta.lock().unwrap();
        let mut all: Vec<BackgroundSessionMetadata> = Self::meta_map(&meta)
            .values()
            .filter_map(|v| serde_json::from_value::<BackgroundSessionMetadata>(v.clone()).ok())
            .filter(|m| {
                self.session_file_path(&m.session_id)
                    .is_some_and(|p| p.exists())
            })
            .collect();
        all.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
        all
    }

    /// `getSessionsByProjectDirectory(projectDirectory)`.
    pub fn get_sessions_by_project_directory(&self, dir: &str) -> Vec<BackgroundSessionMetadata> {
        self.get_all_sessions_metadata()
            .into_iter()
            .filter(|m| m.project_directory.as_deref() == Some(dir))
            .collect()
    }

    /// `getSessionsByAgentId(agentId)`.
    pub fn get_sessions_by_agent_id(&self, agent_id: &str) -> Vec<BackgroundSessionMetadata> {
        self.get_all_sessions_metadata()
            .into_iter()
            .filter(|m| m.agent_id == agent_id)
            .collect()
    }

    /// `getSessionsByTaskId(taskId)`.
    pub fn get_sessions_by_task_id(&self, task_id: &str) -> Vec<BackgroundSessionMetadata> {
        self.get_all_sessions_metadata()
            .into_iter()
            .filter(|m| m.task_id.as_deref() == Some(task_id))
            .collect()
    }

    /// `updateExecutionMetadata(sessionId, executionMetadata)`.
    pub fn update_execution_metadata(
        &self,
        session_id: &str,
        execution: ExecutionMetadata,
    ) -> Result<()> {
        let mut meta = self.meta.lock().unwrap();
        let Some(mut session) = self.read_session_file(&mut meta, session_id) else {
            tracing::warn!(
                session_id,
                "Cannot update execution metadata for non-existent session"
            );
            return Ok(());
        };
        session.insert(
            "executionMetadata".into(),
            serde_json::to_value(execution).unwrap_or(Value::Null),
        );
        session.insert("updatedAt".into(), json!(now_ms()));
        self.write_session_file(session_id, &session)?;
        Self::update_metadata(&mut meta, session_id, &session);
        Ok(())
    }

    /// `cleanupOldSessions(maxAgeMs)` (default 30 days): delete sessions not updated since.
    pub fn cleanup_old_sessions(&self, max_age: Option<Duration>) -> usize {
        let max_age_ms = max_age.map_or(30 * 24 * 60 * 60 * 1000, |d| d.as_millis() as i64);
        let cutoff = now_ms() - max_age_ms;
        let old: Vec<String> = {
            let meta = self.meta.lock().unwrap();
            Self::meta_map(&meta)
                .values()
                .filter_map(|v| serde_json::from_value::<BackgroundSessionMetadata>(v.clone()).ok())
                .filter(|m| m.updated_at < cutoff)
                .map(|m| m.session_id)
                .collect()
        };
        old.iter().filter(|id| self.delete_session(id)).count()
    }
}

impl SessionStore for BackgroundChatSessionManager {
    fn create_session(
        &self,
        session_id: &str,
        meta: SessionMeta,
    ) -> std::result::Result<(), String> {
        BackgroundChatSessionManager::create_session(self, session_id, meta)
            .map_err(|e| e.to_string())
    }

    fn has_session(&self, session_id: &str) -> bool {
        BackgroundChatSessionManager::has_session(self, session_id)
    }

    fn history(&self, session_id: &str) -> Vec<AgentMessage> {
        self.history_values(session_id)
            .iter()
            .map(to_agent_message)
            .collect()
    }

    fn add_message(
        &self,
        session_id: &str,
        message: &AgentMessage,
    ) -> std::result::Result<(), String> {
        let value = serde_json::to_value(message).map_err(|e| e.to_string())?;
        self.add_message_value(session_id, value)
            .map_err(|e| e.to_string())
    }

    fn delete_session(&self, session_id: &str) -> bool {
        BackgroundChatSessionManager::delete_session(self, session_id)
    }
}
