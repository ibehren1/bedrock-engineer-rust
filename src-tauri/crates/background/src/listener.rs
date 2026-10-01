//! The [`agents::SessionListener`] for background runs: `publishSessionUpdate` (pub/sub) and the
//! scheduler's `executionHistoryUpdateCallback` (live message count in the execution history).

use crate::events::SESSION_UPDATE_CHANNEL_PREFIX;
use crate::history::ExecutionHistoryStore;
use crate::pubsub::PubSubManager;
use crate::sessions::BackgroundChatSessionManager;
use agents::{AgentMessage, SessionListener};
use serde_json::json;
use std::sync::Arc;

pub struct BackgroundSessionListener {
    sessions: Arc<BackgroundChatSessionManager>,
    history: Arc<ExecutionHistoryStore>,
    pubsub: Arc<PubSubManager>,
}

impl BackgroundSessionListener {
    pub fn new(
        sessions: Arc<BackgroundChatSessionManager>,
        history: Arc<ExecutionHistoryStore>,
        pubsub: Arc<PubSubManager>,
    ) -> Self {
        BackgroundSessionListener {
            sessions,
            history,
            pubsub,
        }
    }
}

impl SessionListener for BackgroundSessionListener {
    fn message_published(&self, session_id: &str, message: &AgentMessage) {
        self.pubsub.publish(
            &format!("{SESSION_UPDATE_CHANNEL_PREFIX}{session_id}"),
            json!({
                "type": "message-added",
                "sessionId": session_id,
                "message": message,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            }),
        );
    }

    fn history_updated(&self, session_id: &str, message_count: usize) {
        let task_id = self
            .sessions
            .get_session_metadata(session_id)
            .and_then(|m| m.task_id)
            .filter(|t| !t.is_empty());
        if let Some(task_id) = task_id {
            self.history
                .update_message_count(&task_id, session_id, message_count);
        }
    }
}
