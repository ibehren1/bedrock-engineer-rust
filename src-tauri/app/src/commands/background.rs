//! `window.api.backgroundAgent.*` (port of `src/main/handlers/background-agent-handlers.ts`)
//! and `window.api.pubsub.*` (`pubsub-handlers.ts`) over `background::BackgroundAgents`.
//! Errors reject with the TS error message.

use crate::backend::Backend;
use crate::errors;
use agents::{ChatResult, SessionMeta};
use background::{
    BackgroundAgentOptions, ChatConfigParams, ChatParams, ContinueSessionParams, PubSubStats,
    ScheduleConfig, SessionStats,
};
use serde_json::Value;
use tauri::{State, Window};

#[tauri::command]
pub async fn background_agent_chat(
    backend: State<'_, Backend>,
    session_id: String,
    config: ChatConfigParams,
    user_message: String,
    options: Option<BackgroundAgentOptions>,
) -> Result<ChatResult, String> {
    let params = ChatParams {
        session_id,
        config,
        user_message,
        options,
    };
    backend.background.chat(params).await.map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_create_session(
    backend: State<'_, Backend>,
    session_id: String,
    options: Option<SessionMeta>,
) -> Result<Value, String> {
    backend
        .background
        .create_session(&session_id, options)
        .map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_delete_session(backend: State<'_, Backend>, session_id: String) -> Value {
    backend.background.delete_session(&session_id)
}

#[tauri::command]
pub fn background_agent_list_sessions(backend: State<'_, Backend>) -> Value {
    backend.background.list_sessions()
}

#[tauri::command]
pub fn background_agent_get_session_history(
    backend: State<'_, Backend>,
    session_id: String,
) -> Value {
    backend.background.get_session_history(&session_id)
}

#[tauri::command]
pub fn background_agent_get_session_stats(
    backend: State<'_, Backend>,
    session_id: String,
) -> SessionStats {
    backend.background.get_session_stats(&session_id)
}

#[tauri::command]
pub fn background_agent_get_all_sessions_metadata(backend: State<'_, Backend>) -> Value {
    backend.background.get_all_sessions_metadata()
}

#[tauri::command]
pub fn background_agent_get_sessions_by_project(
    backend: State<'_, Backend>,
    project_directory: String,
) -> Value {
    backend
        .background
        .get_sessions_by_project(&project_directory)
}

#[tauri::command]
pub fn background_agent_get_sessions_by_agent(
    backend: State<'_, Backend>,
    agent_id: String,
) -> Value {
    backend.background.get_sessions_by_agent(&agent_id)
}

#[tauri::command]
pub fn background_agent_schedule_task(
    backend: State<'_, Backend>,
    config: ScheduleConfig,
) -> Result<Value, String> {
    backend
        .background
        .schedule_task(config)
        .map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_update_task(
    backend: State<'_, Backend>,
    task_id: String,
    config: ScheduleConfig,
) -> Result<Value, String> {
    backend
        .background
        .update_task(&task_id, config)
        .map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_cancel_task(backend: State<'_, Backend>, task_id: String) -> Value {
    backend.background.cancel_task(&task_id)
}

#[tauri::command]
pub fn background_agent_toggle_task(
    backend: State<'_, Backend>,
    task_id: String,
    enabled: bool,
) -> Value {
    backend.background.toggle_task(&task_id, enabled)
}

#[tauri::command]
pub fn background_agent_list_tasks(backend: State<'_, Backend>) -> Value {
    backend.background.list_tasks()
}

#[tauri::command]
pub fn background_agent_get_task(backend: State<'_, Backend>, task_id: String) -> Value {
    backend.background.get_task(&task_id)
}

#[tauri::command]
pub fn background_agent_get_task_execution_history(
    backend: State<'_, Backend>,
    task_id: String,
) -> Value {
    backend.background.get_task_execution_history(&task_id)
}

#[tauri::command]
pub async fn background_agent_execute_task_manually(
    backend: State<'_, Backend>,
    task_id: String,
) -> Result<Value, String> {
    backend
        .background
        .execute_task_manually(&task_id)
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_get_scheduler_stats(backend: State<'_, Backend>) -> Value {
    backend.background.get_scheduler_stats()
}

#[tauri::command]
pub async fn background_agent_continue_session(
    backend: State<'_, Backend>,
    session_id: String,
    task_id: String,
    user_message: String,
    options: Option<BackgroundAgentOptions>,
) -> Result<ChatResult, String> {
    let params = ContinueSessionParams {
        session_id,
        task_id,
        user_message,
        options,
    };
    backend
        .background
        .continue_session(params)
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub fn background_agent_get_task_system_prompt(
    backend: State<'_, Backend>,
    task_id: String,
) -> Result<Value, String> {
    backend
        .background
        .get_task_system_prompt(&task_id)
        .map_err(errors::plain)
}

/// `background-agent:task-notification`: re-broadcast to every window.
#[tauri::command]
pub fn background_agent_task_notification(backend: State<'_, Backend>, params: Value) {
    backend.background.task_notification(params)
}

// ---- pub/sub (subscriber = the calling window's label) ----------------------------------------

#[tauri::command]
pub fn pubsub_subscribe(backend: State<'_, Backend>, window: Window, channel: String) {
    backend
        .background
        .pubsub_subscribe(&channel, window.label());
}

#[tauri::command]
pub fn pubsub_unsubscribe(backend: State<'_, Backend>, window: Window, channel: String) {
    backend
        .background
        .pubsub_unsubscribe(&channel, window.label());
}

#[tauri::command]
pub fn pubsub_publish(backend: State<'_, Backend>, channel: String, data: Option<Value>) {
    backend
        .background
        .pubsub_publish(&channel, data.unwrap_or(Value::Null));
}

#[tauri::command]
pub fn pubsub_stats(backend: State<'_, Backend>) -> PubSubStats {
    backend.background.pubsub_stats()
}
