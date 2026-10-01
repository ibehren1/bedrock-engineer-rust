//! Background agents: persistent sessions, cron-scheduled tasks, execution history,
//! notifications and pub/sub.
//!
//! Port of `src/main/api/bedrock/services/backgroundAgent/**` (`BackgroundAgentService`,
//! `BackgroundAgentScheduler`, `BackgroundChatSessionManager`; `MainToolSpecProvider` lives in
//! the `agents` crate), `src/main/handlers/background-agent-handlers.ts`,
//! `src/main/handlers/pubsub-handlers.ts`, `src/main/lib/pubsub-manager.ts` and the background
//! part of `src/main/api/bedrock/services/NotificationService.ts`. The agent loop is
//! [`agents::AgentEngine::chat`].
//!
//! # Pieces
//!
//! * [`BackgroundChatSessionManager`] — session JSON files + metadata store, byte-compatible
//!   with Electron; implements [`agents::SessionStore`].
//! * [`BackgroundSessionListener`] — [`agents::SessionListener`]: publishes
//!   `session-update:<sessionId>` pub/sub messages and live message counts in the execution
//!   history.
//! * [`BackgroundAgentScheduler`] — tasks in the config key `backgroundAgentScheduledTasks`,
//!   cron jobs ([`cron`], node-cron semantics, local timezone), run-now, execution history
//!   ([`ExecutionHistoryStore`]), OS notifications ([`Notifier`]) and events ([`EventSink`]).
//! * [`BackgroundAgents`] — one method per IPC handler (see [`api`]).
//!
//! # Events (Tauri event names = Electron channels)
//!
//! * [`TASK_NOTIFICATION_EVENT`] `background-agent:task-notification`
//! * [`TASK_EXECUTION_START_EVENT`] `background-agent:task-execution-start`
//! * [`TASK_SKIPPED_EVENT`] `background-agent:task-skipped`
//! * pub/sub channel `c` → event [`pubsub_event_name`]`(c)` sent to each subscribed window;
//!   session updates use channel `session-update:<sessionId>` with
//!   `{ type: 'message-added', sessionId, message, timestamp }`.

pub mod api;
pub mod config;
pub mod cron;
pub mod error;
pub mod events;
pub mod history;
pub mod listener;
pub mod pubsub;
pub mod scheduler;
pub mod service;
pub mod sessions;
pub mod types;

pub use api::{BackgroundAgents, BackgroundDeps, BackgroundStorage};
pub use config::{ConfigStore, MemoryConfigStore};
pub use error::{Error, Result};
pub use events::{
    show_background_agent_notification, EventSink, NoopEventSink, NoopNotifier, Notifier,
    TaskOsNotification, SESSION_UPDATE_CHANNEL_PREFIX, TASK_EXECUTION_START_EVENT,
    TASK_NOTIFICATION_EVENT, TASK_SKIPPED_EVENT,
};
pub use history::ExecutionHistoryStore;
pub use listener::BackgroundSessionListener;
pub use pubsub::{pubsub_event_name, ChannelStats, PubSubManager, PubSubStats};
pub use scheduler::{calculate_next_run, BackgroundAgentScheduler, SCHEDULED_TASKS_KEY};
pub use service::BackgroundAgentService;
pub use sessions::{
    AllSessionStats, BackgroundChatSessionManager, BackgroundSessionMetadata, ExecutionMetadata,
    SessionStats,
};
pub use types::{
    BackgroundAgentOptions, ChatConfigParams, ChatParams, ContinueSessionParams, ExecutionStatus,
    ScheduleAgentConfig, ScheduleConfig, ScheduledTask, SchedulerStats, TaskExecutionResult,
};

#[cfg(test)]
mod tests;
