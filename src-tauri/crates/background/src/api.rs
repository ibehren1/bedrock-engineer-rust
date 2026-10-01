//! The background-agent IPC surface (`src/main/handlers/background-agent-handlers.ts`) and the
//! pub/sub handlers (`pubsub-handlers.ts`), as library functions returning the JSON the Electron
//! handlers returned. The app wraps each in a `#[tauri::command]` named per BRIDGE.md.

use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::events::{EventSink, Notifier, TASK_NOTIFICATION_EVENT};
use crate::history::ExecutionHistoryStore;
use crate::listener::BackgroundSessionListener;
use crate::pubsub::{PubSubManager, PubSubStats};
use crate::scheduler::{BackgroundAgentScheduler, SchedulerDeps, SCHEDULED_TASKS_KEY};
use crate::service::BackgroundAgentService;
use crate::sessions::{BackgroundChatSessionManager, SessionStats};
use crate::types::{BackgroundAgentOptions, ChatParams, ContinueSessionParams, ScheduleConfig};
use agents::{AgentEngine, AgentRunConfig, ChatResult, SessionMeta};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

/// The persistent pieces the engine needs before it is built.
///
/// ```ignore
/// let storage = background::BackgroundStorage::open(&user_data, &store_dir, sink.clone())?;
/// let engine = Arc::new(
///     agents::AgentEngine::new(converse, registry, store_reader)
///         .with_sessions(storage.sessions.clone())
///         .with_listener(storage.listener.clone()),
/// );
/// let bg = background::BackgroundAgents::new(background::BackgroundDeps {
///     engine, storage, config, events: sink, notifier, runtime,
/// });
/// bg.initialize_scheduler(); // at startup
/// ```
#[derive(Clone)]
pub struct BackgroundStorage {
    pub sessions: Arc<BackgroundChatSessionManager>,
    pub history: Arc<ExecutionHistoryStore>,
    pub pubsub: Arc<PubSubManager>,
    pub listener: Arc<BackgroundSessionListener>,
}

impl BackgroundStorage {
    /// Sessions under `<user_data_path>/background-agent-sessions`; the metadata and execution
    /// history electron-store files in `store_dir` (the directory of `config.json`).
    pub fn open(
        user_data_path: impl AsRef<Path>,
        store_dir: impl AsRef<Path>,
        events: Arc<dyn EventSink>,
    ) -> Result<Self> {
        let sessions = Arc::new(BackgroundChatSessionManager::open(
            user_data_path,
            store_dir.as_ref(),
        )?);
        let history = Arc::new(ExecutionHistoryStore::open(store_dir.as_ref()));
        let pubsub = Arc::new(PubSubManager::new(events));
        let listener = Arc::new(BackgroundSessionListener::new(
            sessions.clone(),
            history.clone(),
            pubsub.clone(),
        ));
        Ok(BackgroundStorage {
            sessions,
            history,
            pubsub,
            listener,
        })
    }
}

pub struct BackgroundDeps {
    /// Built with `storage.sessions` and `storage.listener`.
    pub engine: Arc<AgentEngine>,
    pub storage: BackgroundStorage,
    pub config: Arc<dyn ConfigStore>,
    pub events: Arc<dyn EventSink>,
    pub notifier: Arc<dyn Notifier>,
    pub runtime: Handle,
}

/// Background agents: service, lazily created scheduler, pub/sub.
pub struct BackgroundAgents {
    service: Arc<BackgroundAgentService>,
    storage: BackgroundStorage,
    config: Arc<dyn ConfigStore>,
    events: Arc<dyn EventSink>,
    notifier: Arc<dyn Notifier>,
    runtime: Handle,
    scheduler: Mutex<Option<Arc<BackgroundAgentScheduler>>>,
}

impl BackgroundAgents {
    pub fn new(deps: BackgroundDeps) -> Self {
        BackgroundAgents {
            service: Arc::new(BackgroundAgentService::new(
                deps.engine,
                deps.storage.sessions.clone(),
            )),
            storage: deps.storage,
            config: deps.config,
            events: deps.events,
            notifier: deps.notifier,
            runtime: deps.runtime,
            scheduler: Mutex::new(None),
        }
    }

    pub fn service(&self) -> &Arc<BackgroundAgentService> {
        &self.service
    }

    pub fn pubsub(&self) -> &Arc<PubSubManager> {
        &self.storage.pubsub
    }

    /// `getBackgroundAgentScheduler()`: created on first use.
    pub fn scheduler(&self) -> Arc<BackgroundAgentScheduler> {
        let mut slot = self.scheduler.lock().unwrap();
        slot.get_or_insert_with(|| {
            BackgroundAgentScheduler::new(SchedulerDeps {
                service: self.service.clone(),
                config: self.config.clone(),
                history: self.storage.history.clone(),
                events: self.events.clone(),
                notifier: self.notifier.clone(),
                runtime: self.runtime.clone(),
            })
        })
        .clone()
    }

    /// Whether the scheduler has been created.
    pub fn scheduler_started(&self) -> bool {
        self.scheduler.lock().unwrap().is_some()
    }

    /// `initializeBackgroundAgentScheduler()` (app startup): create the scheduler — restoring
    /// the saved tasks and arming their cron jobs — only when at least one task is saved.
    pub fn initialize_scheduler(&self) {
        match self.config.get(SCHEDULED_TASKS_KEY) {
            Some(Value::Array(a)) if !a.is_empty() => {
                self.scheduler();
            }
            _ => tracing::debug!("No scheduled tasks saved, skipping scheduler startup"),
        }
    }

    /// `shutdownBackgroundAgentScheduler()` (app quit).
    pub fn shutdown_scheduler(&self) {
        if let Some(s) = self.scheduler.lock().unwrap().take() {
            s.shutdown();
        }
    }

    // ---- sessions ----------------------------------------------------------------------------

    /// `background-agent:chat` → `BackgroundChatResult`.
    pub async fn chat(&self, params: ChatParams) -> Result<ChatResult> {
        let config = AgentRunConfig {
            model_id: params.config.model_id,
            system_prompt: params.config.system_prompt,
            agent_id: params.config.agent_id,
            project_directory: params.config.project_directory,
            ..Default::default()
        };
        let options = params.options.unwrap_or_default().to_run_options();
        self.service
            .chat(&params.session_id, &config, &params.user_message, &options)
            .await
    }

    /// `background-agent:create-session` → `{ success: true, sessionId, projectDirectory? }`.
    pub fn create_session(&self, session_id: &str, options: Option<SessionMeta>) -> Result<Value> {
        let options = options.unwrap_or_default();
        let project_directory = options.project_directory.clone();
        self.service.create_session(session_id, options)?;
        let mut out = json!({ "success": true, "sessionId": session_id });
        if let Some(p) = project_directory {
            out["projectDirectory"] = json!(p);
        }
        Ok(out)
    }

    /// `background-agent:delete-session` → `{ success, sessionId }`.
    pub fn delete_session(&self, session_id: &str) -> Value {
        let deleted = self.service.delete_session(session_id);
        json!({ "success": deleted, "sessionId": session_id })
    }

    /// `background-agent:list-sessions` → `{ sessions: string[] }`.
    pub fn list_sessions(&self) -> Value {
        json!({ "sessions": self.service.list_sessions() })
    }

    /// `background-agent:get-session-history` → `{ history: BackgroundMessage[] }`.
    pub fn get_session_history(&self, session_id: &str) -> Value {
        json!({ "history": self.service.get_session_history(session_id) })
    }

    /// `background-agent:get-session-stats` → `{ exists, messageCount, userMessages, assistantMessages, metadata? }`.
    pub fn get_session_stats(&self, session_id: &str) -> SessionStats {
        self.service.get_session_stats(session_id)
    }

    /// `background-agent:get-all-sessions-metadata` → `{ metadata: BackgroundSessionMetadata[] }`.
    pub fn get_all_sessions_metadata(&self) -> Value {
        json!({ "metadata": self.service.get_all_sessions_metadata() })
    }

    /// `background-agent:get-sessions-by-project` → `{ sessions: BackgroundSessionMetadata[] }`.
    pub fn get_sessions_by_project(&self, project_directory: &str) -> Value {
        json!({ "sessions": self.service.get_sessions_by_project_directory(project_directory) })
    }

    /// `background-agent:get-sessions-by-agent` → `{ sessions: BackgroundSessionMetadata[] }`.
    pub fn get_sessions_by_agent(&self, agent_id: &str) -> Value {
        json!({ "sessions": self.service.get_sessions_by_agent_id(agent_id) })
    }

    // ---- scheduler ---------------------------------------------------------------------------

    /// `background-agent:schedule-task` → `{ success: true, taskId }`.
    pub fn schedule_task(&self, config: ScheduleConfig) -> Result<Value> {
        let task_id = self.scheduler().schedule_task(config)?;
        Ok(json!({ "success": true, "taskId": task_id }))
    }

    /// `background-agent:update-task` → `{ success: true, taskId }`, or
    /// `Failed to update task: <taskId>`.
    pub fn update_task(&self, task_id: &str, config: ScheduleConfig) -> Result<Value> {
        if !self.scheduler().update_task(task_id, config) {
            return Err(Error::UpdateFailed(task_id.to_string()));
        }
        Ok(json!({ "success": true, "taskId": task_id }))
    }

    /// `background-agent:cancel-task` → `{ success }`.
    pub fn cancel_task(&self, task_id: &str) -> Value {
        json!({ "success": self.scheduler().cancel_task(task_id) })
    }

    /// `background-agent:toggle-task` → `{ success }`.
    pub fn toggle_task(&self, task_id: &str, enabled: bool) -> Value {
        json!({ "success": self.scheduler().toggle_task(task_id, enabled) })
    }

    /// `background-agent:list-tasks` → `{ tasks: ScheduledTask[] }`.
    pub fn list_tasks(&self) -> Value {
        json!({ "tasks": self.scheduler().list_tasks() })
    }

    /// `background-agent:get-task` → `{ task }` (`{}` when not found, as `undefined` is dropped
    /// by IPC serialization).
    pub fn get_task(&self, task_id: &str) -> Value {
        match self.scheduler().get_task(task_id) {
            Some(t) => json!({ "task": t }),
            None => json!({}),
        }
    }

    /// `background-agent:get-task-execution-history` → `{ history: TaskExecutionResult[] }`.
    pub fn get_task_execution_history(&self, task_id: &str) -> Value {
        json!({ "history": self.scheduler().get_task_execution_history(task_id) })
    }

    /// `background-agent:execute-task-manually` → `{ result: TaskExecutionResult }` once the run
    /// has finished.
    pub async fn execute_task_manually(&self, task_id: &str) -> Result<Value> {
        let result = self.scheduler().execute_task_manually(task_id).await?;
        Ok(json!({ "result": result }))
    }

    /// `background-agent:get-scheduler-stats` → `{ stats }`.
    pub fn get_scheduler_stats(&self) -> Value {
        json!({ "stats": self.scheduler().get_stats() })
    }

    /// `background-agent:continue-session` → `BackgroundChatResult`: chat in an existing session
    /// with the task's agent and model. Without `options`: tools on, 5 rounds, 5 minutes.
    pub async fn continue_session(&self, params: ContinueSessionParams) -> Result<ChatResult> {
        let task = self
            .scheduler()
            .get_task(&params.task_id)
            .ok_or_else(|| Error::TaskNotFound(params.task_id.clone()))?;
        let config = AgentRunConfig {
            model_id: task.model_id,
            agent_id: task.agent_id,
            project_directory: task.project_directory,
            ..Default::default()
        };
        let options = params
            .options
            .unwrap_or(BackgroundAgentOptions {
                enable_tool_execution: Some(true),
                max_tool_executions: Some(5),
                timeout_ms: Some(300_000),
            })
            .to_run_options();
        self.service
            .chat(&params.session_id, &config, &params.user_message, &options)
            .await
    }

    /// `background-agent:get-task-system-prompt` → `{ systemPrompt }`.
    pub fn get_task_system_prompt(&self, task_id: &str) -> Result<Value> {
        let task = self
            .scheduler()
            .get_task(task_id)
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?;
        let prompt = self
            .service
            .system_prompt(&task.agent_id, task.project_directory.as_deref())?;
        Ok(json!({ "systemPrompt": prompt }))
    }

    /// `background-agent:task-notification`: re-broadcast `params` to every window.
    pub fn task_notification(&self, params: Value) {
        self.events.emit(TASK_NOTIFICATION_EVENT, params);
    }

    // ---- pub/sub -----------------------------------------------------------------------------

    /// `pubsub:subscribe { channel }` from window `subscriber`.
    pub fn pubsub_subscribe(&self, channel: &str, subscriber: &str) {
        self.storage.pubsub.subscribe(channel, subscriber);
    }

    /// `pubsub:unsubscribe { channel }` from window `subscriber`.
    pub fn pubsub_unsubscribe(&self, channel: &str, subscriber: &str) {
        self.storage.pubsub.unsubscribe(channel, subscriber);
    }

    /// `pubsub:publish { channel, data }`.
    pub fn pubsub_publish(&self, channel: &str, data: Value) {
        self.storage.pubsub.publish(channel, data);
    }

    /// `pubsub:stats`.
    pub fn pubsub_stats(&self) -> PubSubStats {
        self.storage.pubsub.stats()
    }
}
