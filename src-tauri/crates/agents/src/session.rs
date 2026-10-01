//! Agent run sessions: the message type, the session store the engine writes to, and the
//! notification hooks.
//!
//! `BackgroundChatSessionManager` (persistent JSON files, Task 11) implements [`SessionStore`];
//! until then [`InMemorySessionStore`] serves sub-agent runs, whose sessions are deleted as soon
//! as the delegation finishes anyway.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

/// `BackgroundMessage`: a Bedrock `Message` plus id, timestamp and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub id: String,
    /// `user` or `assistant`.
    pub role: String,
    pub content: Vec<Value>,
    /// Milliseconds since the epoch (`Date.now()`).
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

impl AgentMessage {
    /// A new message with a fresh uuid and the current time.
    pub fn new(role: &str, content: Vec<Value>) -> Self {
        AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: role.to_string(),
            content,
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: None,
        }
    }

    /// The `{ role, content }` Bedrock message sent to Converse.
    pub fn to_converse(&self) -> Value {
        json!({ "role": self.role, "content": self.content })
    }
}

/// `CreateSessionOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
}

/// The session operations `BackgroundAgentService` uses on `BackgroundChatSessionManager`.
pub trait SessionStore: Send + Sync {
    /// `createSession(sessionId, options)`.
    fn create_session(&self, session_id: &str, meta: SessionMeta) -> Result<(), String>;
    /// `hasSession(sessionId)`.
    fn has_session(&self, session_id: &str) -> bool;
    /// `getHistory(sessionId)`; empty for an unknown session.
    fn history(&self, session_id: &str) -> Vec<AgentMessage>;
    /// `addMessage(sessionId, message)`; creates the session when it does not exist.
    fn add_message(&self, session_id: &str, message: &AgentMessage) -> Result<(), String>;
    /// `deleteSession(sessionId)`; `false` when there was nothing to delete.
    fn delete_session(&self, session_id: &str) -> bool;
}

/// Process-local [`SessionStore`].
#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<String, (SessionMeta, Vec<AgentMessage>)>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Session ids, in no particular order.
    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.lock().unwrap().keys().cloned().collect()
    }

    /// The metadata a session was created with.
    pub fn meta(&self, session_id: &str) -> Option<SessionMeta> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|(m, _)| m.clone())
    }
}

impl SessionStore for InMemorySessionStore {
    fn create_session(&self, session_id: &str, meta: SessionMeta) -> Result<(), String> {
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), (meta, Vec::new()));
        Ok(())
    }

    fn has_session(&self, session_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(session_id)
    }

    fn history(&self, session_id: &str) -> Vec<AgentMessage> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|(_, h)| h.clone())
            .unwrap_or_default()
    }

    fn add_message(&self, session_id: &str, message: &AgentMessage) -> Result<(), String> {
        self.sessions
            .lock()
            .unwrap()
            .entry(session_id.to_string())
            .or_default()
            .1
            .push(message.clone());
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> bool {
        self.sessions.lock().unwrap().remove(session_id).is_some()
    }
}

/// Notifications a run emits. Both methods default to no-ops.
pub trait SessionListener: Send + Sync {
    /// `publishSessionUpdate(sessionId, message)`: pub/sub channel `session-update:${sessionId}`
    /// with `{ type: 'message-added', sessionId, message, timestamp }`. Called for the user
    /// message, every tool-result message, every follow-up assistant message, and the final
    /// assistant message of a run without tool use (not for the first tool-use response).
    fn message_published(&self, _session_id: &str, _message: &AgentMessage) {}

    /// `executionHistoryUpdateCallback`: called after the user message and after the final
    /// response of a run without tool use, with the session's message count. The TS invoked it
    /// only for sessions whose metadata has a `taskId`; the listener checks that itself.
    fn history_updated(&self, _session_id: &str, _message_count: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_lifecycle() {
        let s = InMemorySessionStore::new();
        assert!(!s.has_session("a"));
        s.create_session(
            "a",
            SessionMeta {
                agent_id: Some("x".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.has_session("a"));
        s.add_message("a", &AgentMessage::new("user", vec![json!({"text": "hi"})]))
            .unwrap();
        assert_eq!(s.history("a").len(), 1);
        assert_eq!(s.meta("a").unwrap().agent_id.as_deref(), Some("x"));
        // addMessage re-creates a missing session
        s.add_message("b", &AgentMessage::new("user", vec![]))
            .unwrap();
        assert!(s.has_session("b"));
        assert!(s.delete_session("a"));
        assert!(!s.delete_session("a"));
        assert!(s.history("a").is_empty());
    }

    #[test]
    fn message_serializes_like_background_message() {
        let mut m = AgentMessage::new("assistant", vec![json!({"text": "x"})]);
        assert_eq!(
            m.to_converse(),
            json!({"role": "assistant", "content": [{"text": "x"}]})
        );
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("metadata").is_none());
        m.metadata = Some(json!({"converseMetadata": {}}));
        let v = serde_json::to_value(&m).unwrap();
        assert!(v["timestamp"].is_i64());
        assert_eq!(v["metadata"]["converseMetadata"], json!({}));
    }
}
