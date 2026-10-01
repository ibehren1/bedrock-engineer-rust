//! Push notifications to the renderer. Replaces `pubSubManager.publish(channel, data)`; the
//! app crate implements [`EventSink`] with Tauri events on the same channel names.

use std::sync::Mutex;

use serde::Serialize;

use crate::activity::SandboxActivityEntry;
use crate::types::SandboxStatus;

/// `{ type: 'start' | 'settled' | 'exit', entry }` on `docker-sandbox:activity:<sessionId>`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ActivityEvent {
    Start { entry: SandboxActivityEntry },
    Settled { entry: SandboxActivityEntry },
    Exit { entry: SandboxActivityEntry },
}

impl ActivityEvent {
    pub fn entry(&self) -> &SandboxActivityEntry {
        match self {
            ActivityEvent::Start { entry }
            | ActivityEvent::Settled { entry }
            | ActivityEvent::Exit { entry } => entry,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ActivityEvent::Start { .. } => "start",
            ActivityEvent::Settled { .. } => "settled",
            ActivityEvent::Exit { .. } => "exit",
        }
    }
}

/// `{ type: 'state', status }` on `docker-sandbox:state:<sessionId>`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StateEvent {
    State { status: SandboxStatus },
}

/// Messages on `docker-sandbox:terminal:<terminalId>`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TerminalMessage {
    Data {
        bytes: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        truncated: Option<bool>,
    },
    Exit {
        #[serde(rename = "exitCode")]
        exit_code: Option<i32>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SandboxEvent {
    Activity(ActivityEvent),
    State(StateEvent),
    Terminal(TerminalMessage),
}

/// Where sandbox events go. Must not block: the terminal flushes on a 16ms cadence.
pub trait EventSink: Send + Sync {
    fn publish(&self, channel: &str, event: SandboxEvent);
}

/// Drops everything.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl EventSink for NoopSink {
    fn publish(&self, _channel: &str, _event: SandboxEvent) {}
}

/// Keeps every event, for tests (the TS tests spy on `pubSubManager.publish`).
#[derive(Debug, Default)]
pub struct RecordingSink {
    events: Mutex<Vec<(String, SandboxEvent)>>,
}

impl RecordingSink {
    pub fn events(&self) -> Vec<(String, SandboxEvent)> {
        self.events.lock().unwrap().clone()
    }

    pub fn on_channel(&self, channel: &str) -> Vec<SandboxEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(ch, _)| ch == channel)
            .map(|(_, event)| event.clone())
            .collect()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl EventSink for RecordingSink {
    fn publish(&self, channel: &str, event: SandboxEvent) {
        self.events
            .lock()
            .unwrap()
            .push((channel.to_string(), event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_like_the_ts_messages() {
        let exit = SandboxEvent::Terminal(TerminalMessage::Exit { exit_code: Some(3) });
        assert_eq!(
            serde_json::to_value(&exit).unwrap(),
            serde_json::json!({ "type": "exit", "exitCode": 3 })
        );
        let data = SandboxEvent::Terminal(TerminalMessage::Data {
            bytes: vec![104, 105],
            truncated: None,
        });
        assert_eq!(
            serde_json::to_value(&data).unwrap(),
            serde_json::json!({ "type": "data", "bytes": [104, 105] })
        );
        let state = SandboxEvent::State(StateEvent::State {
            status: SandboxStatus::missing(),
        });
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            serde_json::json!({ "type": "state", "status": { "exists": false, "state": "missing", "containers": [] } })
        );
    }
}
