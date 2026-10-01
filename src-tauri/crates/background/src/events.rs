//! Main → renderer events and OS notifications.
//!
//! The Electron main process sent these with `webContents.send(channel, params)` to every
//! window; the Tauri app emits them as events with the same names (BRIDGE.md rule 5).

use crate::config::ConfigStore;
use serde_json::Value;

/// `background-agent:task-notification` — a task run finished (success or failure).
pub const TASK_NOTIFICATION_EVENT: &str = "background-agent:task-notification";
/// `background-agent:task-execution-start` — a task run started.
pub const TASK_EXECUTION_START_EVENT: &str = "background-agent:task-execution-start";
/// `background-agent:task-skipped` — a scheduled run was skipped (`duplicate_execution`).
pub const TASK_SKIPPED_EVENT: &str = "background-agent:task-skipped";
/// Pub/sub channel prefix for session updates: `session-update:<sessionId>`.
pub const SESSION_UPDATE_CHANNEL_PREFIX: &str = "session-update:";

/// Where events go. The app maps this to `AppHandle::emit` / `emit_to`.
pub trait EventSink: Send + Sync {
    /// Broadcast to every window (`BrowserWindow.getAllWindows().forEach(w => w.webContents.send(...))`).
    fn emit(&self, event: &str, payload: Value);

    /// Send to one subscriber (a window label) — used by pub/sub. An error drops the
    /// subscriber. Defaults to [`emit`](Self::emit).
    fn emit_to(&self, target: &str, event: &str, payload: Value) -> Result<(), String> {
        let _ = target;
        self.emit(event, payload);
        Ok(())
    }
}

/// An [`EventSink`] that drops everything.
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: Value) {}
}

/// An OS notification for a finished task (`MainNotificationService.showBackgroundAgentNotification`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOsNotification {
    pub title: String,
    /// `[<taskName>] <message>`.
    pub body: String,
    /// Clicking the notification opens the task history window for this task
    /// (`window:openTaskHistory`).
    pub task_id: String,
    pub task_name: String,
}

/// OS notifications (the app uses `tauri-plugin-notification`).
pub trait Notifier: Send + Sync {
    /// Whether the main window has focus; notifications are skipped while it does.
    fn is_app_focused(&self) -> bool {
        false
    }
    /// Show the notification.
    fn show(&self, notification: TaskOsNotification);
}

/// A [`Notifier`] that shows nothing.
pub struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn show(&self, _notification: TaskOsNotification) {}
}

/// `showBackgroundAgentNotification({ taskId, taskName, success, aiMessage?, error? })`:
/// skipped when the `notification` setting is falsy (unset counts as on) or the app is focused.
/// Returns whether a notification was shown.
pub fn show_background_agent_notification(
    config: &dyn ConfigStore,
    notifier: &dyn Notifier,
    task_id: &str,
    task_name: &str,
    success: bool,
    ai_message: Option<&str>,
    error: Option<&str>,
) -> bool {
    let (title, body) = if success {
        (
            "Background Agent Task Completed",
            ai_message
                .filter(|m| !m.is_empty())
                .unwrap_or("Task completed successfully"),
        )
    } else {
        (
            "Background Agent Task Failed",
            error
                .filter(|m| !m.is_empty())
                .unwrap_or("Task execution failed"),
        )
    };
    let enabled = match config.get("notification") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    };
    if !enabled {
        tracing::debug!(title, "Notification disabled by user settings");
        return false;
    }
    if notifier.is_app_focused() {
        tracing::debug!(title, "App window is focused, skipping notification");
        return false;
    }
    notifier.show(TaskOsNotification {
        title: title.to_string(),
        body: format!("[{task_name}] {body}"),
        task_id: task_id.to_string(),
        task_name: task_name.to_string(),
    });
    true
}
