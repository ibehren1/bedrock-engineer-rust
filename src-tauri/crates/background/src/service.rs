//! `BackgroundAgentService.ts`: the session-facing half. The agent loop itself is
//! [`agents::AgentEngine::chat`]; this wraps it with the persistent session manager.

use crate::error::Result;
use crate::sessions::{
    AllSessionStats, BackgroundChatSessionManager, BackgroundSessionMetadata, SessionStats,
};
use agents::{AgentEngine, AgentRunConfig, AgentRunOptions, ChatResult, SessionMeta};
use serde_json::Value;
use std::sync::Arc;

/// Engine + session manager. The engine must have been built with
/// `.with_sessions(sessions.clone())` and a [`crate::BackgroundSessionListener`] (see
/// [`crate::BackgroundStorage`]), so runs persist to and notify from the same sessions.
pub struct BackgroundAgentService {
    engine: Arc<AgentEngine>,
    sessions: Arc<BackgroundChatSessionManager>,
}

impl BackgroundAgentService {
    pub fn new(engine: Arc<AgentEngine>, sessions: Arc<BackgroundChatSessionManager>) -> Self {
        BackgroundAgentService { engine, sessions }
    }

    pub fn engine(&self) -> &Arc<AgentEngine> {
        &self.engine
    }

    pub fn sessions(&self) -> &Arc<BackgroundChatSessionManager> {
        &self.sessions
    }

    /// `chat(sessionId, config, userMessage, options)`.
    pub async fn chat(
        &self,
        session_id: &str,
        config: &AgentRunConfig,
        user_message: &str,
        options: &AgentRunOptions,
    ) -> Result<ChatResult> {
        Ok(self
            .engine
            .chat(session_id, config, user_message, options)
            .await?)
    }

    /// `createSession(sessionId, options)`.
    pub fn create_session(&self, session_id: &str, options: SessionMeta) -> Result<()> {
        self.sessions.create_session(session_id, options)
    }

    /// `deleteSession(sessionId)` (also drops the engine's prompt-cache point).
    pub fn delete_session(&self, session_id: &str) -> bool {
        let _ = self.engine.delete_session(session_id);
        self.sessions.delete_session(session_id)
    }

    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.list_sessions()
    }

    pub fn get_session_stats(&self, session_id: &str) -> SessionStats {
        self.sessions.get_session_stats(session_id)
    }

    pub fn get_all_session_stats(&self) -> AllSessionStats {
        self.sessions.get_all_session_stats()
    }

    /// `getSessionHistory(sessionId)`: the stored messages as JSON.
    pub fn get_session_history(&self, session_id: &str) -> Vec<Value> {
        self.sessions.history_values(session_id)
    }

    pub fn get_all_sessions_metadata(&self) -> Vec<BackgroundSessionMetadata> {
        self.sessions.get_all_sessions_metadata()
    }

    pub fn get_sessions_by_project_directory(&self, dir: &str) -> Vec<BackgroundSessionMetadata> {
        self.sessions.get_sessions_by_project_directory(dir)
    }

    pub fn get_sessions_by_agent_id(&self, agent_id: &str) -> Vec<BackgroundSessionMetadata> {
        self.sessions.get_sessions_by_agent_id(agent_id)
    }

    pub fn get_session_metadata(&self, session_id: &str) -> Option<BackgroundSessionMetadata> {
        self.sessions.get_session_metadata(session_id)
    }

    /// The agent's system prompt with environment context and placeholders (the core of
    /// `getTaskSystemPrompt`).
    pub fn system_prompt(&self, agent_id: &str, project_directory: Option<&str>) -> Result<String> {
        Ok(self.engine.system_prompt_for(agent_id, project_directory)?)
    }
}
