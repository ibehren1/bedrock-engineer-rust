//! `BackgroundAgentScheduler.ts`: cron-scheduled background agent tasks.
//!
//! * Tasks persist in the config key [`SCHEDULED_TASKS_KEY`] (`backgroundAgentScheduledTasks`).
//! * Each enabled task has a cron job (a tokio task) evaluating the expression in the system's
//!   local timezone with node-cron semantics ([`crate::cron`]). Slots missed by more than one
//!   second (system sleep, stalls) are skipped, not caught up.
//! * A scheduled run that fires while the previous run of the same task is still going is
//!   skipped with a `background-agent:task-skipped` event (`reason: 'duplicate_execution'`).
//!   Manual runs ignore that guard and run even when the task is disabled.
//! * Each run: `task-execution-start` event → "running" history entry → new session
//!   (`scheduled-<taskId>-<uuid>`) or the task's last session when `continueSession` is on →
//!   agent chat (tools on, 500 rounds, 10 min first-call timeout) → history entry, task stats,
//!   OS notification and `task-notification` event.
//!
//! Differences from the TS: the cron heartbeat re-checks the wall clock at least every 60 s
//! (node-cron waited up to 24 h on a monotonic timer, which drifts across system sleep), and a
//! run's bookkeeping updates the task as currently stored instead of re-inserting the object the
//! run started with (which in TS undid a concurrent edit and resurrected cancelled tasks).

use crate::config::ConfigStore;
use crate::cron::{plan_beat, CronExpression, MISSED_EXECUTION_TOLERANCE_MS};
use crate::error::{Error, Result};
use crate::events::{
    show_background_agent_notification, EventSink, Notifier, TASK_EXECUTION_START_EVENT,
    TASK_NOTIFICATION_EVENT, TASK_SKIPPED_EVENT,
};
use crate::history::ExecutionHistoryStore;
use crate::service::BackgroundAgentService;
use crate::types::{
    ExecutionStatus, ScheduleConfig, ScheduledTask, SchedulerStats, TaskExecutionResult,
};
use agents::{AgentRunOptions, ChatResult, SessionMeta};
use chrono::Local;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// Config store key holding the task list.
pub const SCHEDULED_TASKS_KEY: &str = "backgroundAgentScheduledTasks";
/// Longest wait between two wall-clock checks of a cron job.
pub const HEARTBEAT_MAX_DELAY: Duration = Duration::from_secs(60);
/// Options for scheduled and manual runs.
pub fn scheduled_run_options() -> AgentRunOptions {
    AgentRunOptions {
        enable_tool_execution: true,
        max_tool_executions: 500,
        timeout: Duration::from_millis(600_000),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// `calculateNextRun(cronExpression)` in the local timezone.
pub fn calculate_next_run(cron_expression: &str) -> Option<i64> {
    let expr = CronExpression::parse(cron_expression)
        .inspect_err(|_| {
            tracing::warn!(
                cron_expression,
                "Cannot calculate next run for invalid cron expression"
            )
        })
        .ok()?;
    expr.next_after_ms(&Local, now_ms())
}

/// The first 200 UTF-16 units of the joined text blocks, plus `...` when cut
/// (`textContent.substring(0, 200) + '...'`).
fn ai_message_of(result: &ChatResult) -> String {
    let text = result
        .response
        .content
        .iter()
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() > 200 {
        format!("{}...", String::from_utf16_lossy(&units[..200]))
    } else {
        text
    }
}

#[derive(Default)]
struct State {
    /// In insertion order (a JS `Map`).
    tasks: Vec<ScheduledTask>,
    /// Persisted entries this version can't deserialize, written back unchanged (after the
    /// typed tasks) on every save so they are never dropped.
    unparsed: Vec<Value>,
    jobs: HashMap<String, JoinHandle<()>>,
}

impl State {
    fn get(&self, id: &str) -> Option<&ScheduledTask> {
        self.tasks.iter().find(|t| t.id == id)
    }
    fn get_mut(&mut self, id: &str) -> Option<&mut ScheduledTask> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
    /// `Map.set`: replace in place or append.
    fn set(&mut self, task: ScheduledTask) {
        match self.tasks.iter_mut().find(|t| t.id == task.id) {
            Some(slot) => *slot = task,
            None => self.tasks.push(task),
        }
    }
    fn stop_job(&mut self, id: &str) {
        if let Some(job) = self.jobs.remove(id) {
            job.abort();
        }
    }
}

/// Dependencies of the scheduler.
pub struct SchedulerDeps {
    pub service: Arc<BackgroundAgentService>,
    pub config: Arc<dyn ConfigStore>,
    pub history: Arc<ExecutionHistoryStore>,
    pub events: Arc<dyn EventSink>,
    pub notifier: Arc<dyn Notifier>,
    /// Runtime the cron jobs and runs are spawned on.
    pub runtime: Handle,
}

pub struct BackgroundAgentScheduler {
    weak: Weak<BackgroundAgentScheduler>,
    deps: SchedulerDeps,
    state: Mutex<State>,
}

impl BackgroundAgentScheduler {
    /// The TS constructor: reset the persisted execution state, then restore the tasks and start
    /// the cron jobs of the enabled ones.
    pub fn new(deps: SchedulerDeps) -> Arc<Self> {
        let scheduler = Arc::new_cyclic(|weak| BackgroundAgentScheduler {
            weak: weak.clone(),
            deps,
            state: Mutex::new(State::default()),
        });
        scheduler.reset_all_scheduler_state();
        scheduler.restore_persisted_tasks();
        tracing::info!("BackgroundAgentScheduler initialized");
        scheduler
    }

    /// Clear `isExecuting` / `lastExecutionStarted` (and on Windows `lastError`) of every
    /// persisted task.
    fn reset_all_scheduler_state(&self) {
        {
            let mut st = self.state.lock().unwrap();
            for (_, job) in st.jobs.drain() {
                job.abort();
            }
            st.tasks.clear();
            st.unparsed.clear();
        }
        let Some(Value::Array(tasks)) = self.deps.config.get(SCHEDULED_TASKS_KEY) else {
            return;
        };
        let cleaned: Vec<Value> = tasks
            .into_iter()
            .map(|t| {
                // Anything but an object is left exactly as stored.
                let Value::Object(mut m) = t else {
                    return t;
                };
                m.insert("isExecuting".into(), json!(false));
                m.remove("lastExecutionStarted");
                if cfg!(windows) {
                    m.remove("lastError");
                }
                Value::Object(m)
            })
            .collect();
        if let Err(e) = self
            .deps
            .config
            .set(SCHEDULED_TASKS_KEY, Value::Array(cleaned))
        {
            tracing::error!(error = %e, "Failed to reset scheduler state");
        }
    }

    fn restore_persisted_tasks(&self) {
        let persisted = match self.deps.config.get(SCHEDULED_TASKS_KEY) {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        let mut st = self.state.lock().unwrap();
        for data in persisted {
            let mut task: ScheduledTask = match serde_json::from_value(data.clone()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to restore persisted task; keeping it as stored");
                    st.unparsed.push(data);
                    continue;
                }
            };
            task.next_run = calculate_next_run(&task.cron_expression);
            let enabled = task.enabled;
            st.set(task.clone());
            if enabled {
                if let Err(e) = self.start_cron_job(&mut st, &task) {
                    tracing::error!(task_id = %task.id, error = %e, "Failed to start cron job");
                }
            }
        }
        tracing::info!(count = st.tasks.len(), "Restored persisted scheduled tasks");
    }

    fn persist(&self, st: &State) {
        let mut tasks: Vec<Value> = match st
            .tasks
            .iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<_, _>>()
        {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize tasks; not persisting");
                return;
            }
        };
        tasks.extend(st.unparsed.iter().cloned());
        let tasks = Value::Array(tasks);
        if let Err(e) = self.deps.config.set(SCHEDULED_TASKS_KEY, tasks) {
            tracing::error!(error = %e, "Failed to persist tasks");
        }
    }

    /// `startCronJob(task)`: replace any existing job for the task.
    fn start_cron_job(&self, st: &mut State, task: &ScheduledTask) -> Result<()> {
        st.stop_job(&task.id);
        let expr = CronExpression::parse(&task.cron_expression).map_err(|_| {
            Error::Schedule(format!(
                "Invalid cron expression for task {}: {}",
                task.id, task.cron_expression
            ))
        })?;
        let first = expr.next_after_ms(&Local, now_ms()).ok_or_else(|| {
            Error::Schedule("Could not find next matching date within reasonable time range".into())
        })?;
        let job = self.deps.runtime.spawn(run_cron_job(
            self.weak.clone(),
            task.id.clone(),
            expr,
            first,
        ));
        st.jobs.insert(task.id.clone(), job);
        tracing::info!(task_id = %task.id, cron_expression = %task.cron_expression, "Cron job started successfully");
        Ok(())
    }

    /// `scheduleTask(config)`; returns the task id.
    pub fn schedule_task(&self, config: ScheduleConfig) -> Result<String> {
        if CronExpression::parse(&config.cron_expression).is_err() {
            return Err(Error::InvalidCron(config.cron_expression));
        }
        let task = ScheduledTask {
            id: config
                .task_id
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: config.name,
            next_run: calculate_next_run(&config.cron_expression),
            cron_expression: config.cron_expression,
            agent_id: config.agent_config.agent_id,
            model_id: config.agent_config.model_id,
            project_directory: config.agent_config.project_directory,
            wake_word: config.wake_word,
            enabled: config.enabled,
            created_at: now_ms(),
            run_count: 0,
            inference_config: config.agent_config.inference_config,
            continue_session: config.continue_session,
            continue_session_prompt: config.continue_session_prompt,
            ..Default::default()
        };
        let mut st = self.state.lock().unwrap();
        st.set(task.clone());
        if task.enabled {
            self.start_cron_job(&mut st, &task)?;
        }
        self.persist(&st);
        tracing::info!(task_id = %task.id, name = %task.name, "Scheduled task created");
        Ok(task.id)
    }

    /// `cancelTask(taskId)`: stop, remove, and drop its execution history.
    pub fn cancel_task(&self, task_id: &str) -> bool {
        let mut st = self.state.lock().unwrap();
        let Some(pos) = st.tasks.iter().position(|t| t.id == task_id) else {
            return false;
        };
        st.stop_job(task_id);
        st.tasks.remove(pos);
        self.deps.history.remove(task_id);
        self.persist(&st);
        tracing::info!(task_id, "Scheduled task cancelled");
        true
    }

    /// `updateTask(taskId, config)`; keeps `createdAt`, run statistics and `lastSessionId`,
    /// clears `lastError`. `false` when the task is missing or the expression is invalid.
    pub fn update_task(&self, task_id: &str, config: ScheduleConfig) -> bool {
        let mut st = self.state.lock().unwrap();
        let Some(existing) = st.get(task_id).cloned() else {
            tracing::error!(task_id, "Task not found for update");
            return false;
        };
        if CronExpression::parse(&config.cron_expression).is_err() {
            tracing::error!(task_id, "Failed to update task: invalid cron expression");
            return false;
        }
        st.stop_job(task_id);
        let updated = ScheduledTask {
            name: config.name,
            next_run: calculate_next_run(&config.cron_expression),
            cron_expression: config.cron_expression,
            agent_id: config.agent_config.agent_id,
            model_id: config.agent_config.model_id,
            project_directory: config.agent_config.project_directory,
            wake_word: config.wake_word,
            enabled: config.enabled,
            inference_config: config.agent_config.inference_config,
            continue_session: config.continue_session,
            continue_session_prompt: config.continue_session_prompt,
            last_error: None,
            ..existing
        };
        st.set(updated.clone());
        if updated.enabled {
            if let Err(e) = self.start_cron_job(&mut st, &updated) {
                tracing::error!(task_id, error = %e, "Failed to update task");
                return false;
            }
        }
        self.persist(&st);
        true
    }

    /// `toggleTask(taskId, enabled)`.
    pub fn toggle_task(&self, task_id: &str, enabled: bool) -> bool {
        let mut st = self.state.lock().unwrap();
        let Some(task) = st.get_mut(task_id) else {
            return false;
        };
        task.enabled = enabled;
        let task = task.clone();
        if enabled {
            if let Err(e) = self.start_cron_job(&mut st, &task) {
                tracing::error!(task_id, error = %e, "Failed to toggle task");
                return false;
            }
        } else {
            st.stop_job(task_id);
        }
        self.persist(&st);
        true
    }

    /// `listTasks()`.
    pub fn list_tasks(&self) -> Vec<ScheduledTask> {
        self.state.lock().unwrap().tasks.clone()
    }

    /// `getTask(taskId)`.
    pub fn get_task(&self, task_id: &str) -> Option<ScheduledTask> {
        self.state.lock().unwrap().get(task_id).cloned()
    }

    /// `getTaskExecutionHistory(taskId)`.
    pub fn get_task_execution_history(&self, task_id: &str) -> Vec<TaskExecutionResult> {
        self.deps.history.get(task_id)
    }

    /// `getStats()`.
    pub fn get_stats(&self) -> SchedulerStats {
        let st = self.state.lock().unwrap();
        let enabled = st.tasks.iter().filter(|t| t.enabled).count();
        SchedulerStats {
            total_tasks: st.tasks.len(),
            enabled_tasks: enabled,
            disabled_tasks: st.tasks.len() - enabled,
            total_executions: st.tasks.iter().map(|t| t.run_count).sum(),
            tasks_with_errors: st
                .tasks
                .iter()
                .filter(|t| t.last_error.as_deref().is_some_and(|e| !e.is_empty()))
                .count(),
            active_cron_jobs: st.jobs.len(),
        }
    }

    /// Whether a cron job is armed for the task.
    pub fn has_cron_job(&self, task_id: &str) -> bool {
        self.state.lock().unwrap().jobs.contains_key(task_id)
    }

    /// `shutdown()`: stop every cron job and persist the final task state.
    pub fn shutdown(&self) {
        let mut st = self.state.lock().unwrap();
        for (_, job) in st.jobs.drain() {
            job.abort();
        }
        if cfg!(windows) {
            for t in st.tasks.iter_mut() {
                if t.is_executing() {
                    t.is_executing = Some(false);
                    t.last_execution_started = None;
                }
            }
        }
        self.persist(&st);
        tracing::info!(tasks = st.tasks.len(), "Scheduler shutdown completed");
    }

    /// `executeTaskManually(taskId)`: run now (even when disabled) and return the latest history
    /// entry.
    pub async fn execute_task_manually(&self, task_id: &str) -> Result<TaskExecutionResult> {
        let task = self
            .get_task(task_id)
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?;
        tracing::info!(task_id, "Manual task execution requested");
        self.execute_task_internal(task, true).await;
        self.get_task_execution_history(task_id)
            .pop()
            .ok_or_else(|| Error::NoExecutionResult(task_id.to_string()))
    }

    /// `executeTask(taskId)`: a scheduled run, with the duplicate-execution guard.
    pub async fn execute_task(&self, task_id: &str) {
        let task = {
            let mut st = self.state.lock().unwrap();
            let Some(task) = st.get_mut(task_id).filter(|t| t.enabled) else {
                tracing::warn!(
                    task_id,
                    "Attempted to execute disabled or non-existent task"
                );
                return;
            };
            if task.is_executing() {
                let execution_time = task.last_execution_started.map_or(0, |s| now_ms() - s);
                let name = task.name.clone();
                drop(st);
                tracing::warn!(
                    task_id,
                    "Task is already executing, skipping duplicate execution"
                );
                self.deps.events.emit(
                    TASK_SKIPPED_EVENT,
                    json!({
                        "taskId": task_id,
                        "taskName": name,
                        "reason": "duplicate_execution",
                        "executionTime": execution_time,
                    }),
                );
                return;
            }
            task.is_executing = Some(true);
            task.last_execution_started = Some(now_ms());
            task.clone()
        };
        self.execute_task_internal(task, false).await;
    }

    /// Apply `f` to the stored task (if it still exists), optionally persisting.
    fn update_stored(
        &self,
        task_id: &str,
        persist: bool,
        f: impl FnOnce(&mut ScheduledTask),
    ) -> Option<ScheduledTask> {
        let mut st = self.state.lock().unwrap();
        let task = st.get_mut(task_id)?;
        f(task);
        let snapshot = task.clone();
        if persist {
            self.persist(&st);
        }
        Some(snapshot)
    }

    async fn execute_task_internal(&self, task: ScheduledTask, manual: bool) {
        let task_id = task.id.clone();
        let sessions = self.deps.service.sessions().clone();

        let mut prompt = task.wake_word.clone();
        let mut continuation = false;
        let new_session_id = || format!("scheduled-{task_id}-{}", uuid::Uuid::new_v4());
        let last_session = task
            .last_session_id
            .clone()
            .filter(|s| !s.is_empty() && task.continue_session == Some(true));
        let session_id = match last_session {
            Some(last) if sessions.has_valid_session(&last) => {
                continuation = true;
                if let Some(p) = task
                    .continue_session_prompt
                    .as_deref()
                    .filter(|p| !p.trim().is_empty())
                {
                    prompt = p.to_string();
                }
                tracing::info!(task_id, session_id = %last, "Continuing existing valid session");
                last
            }
            Some(last) => {
                self.update_stored(&task_id, true, |t| t.last_session_id = None);
                let id = new_session_id();
                tracing::warn!(task_id, previous_session_id = %last, new_session_id = %id, "Previous session invalid, creating new session");
                id
            }
            None => new_session_id(),
        };

        tracing::info!(
            task_id,
            session_id,
            continuation,
            "Executing scheduled task"
        );
        self.deps.events.emit(
            TASK_EXECUTION_START_EVENT,
            json!({ "taskId": task_id, "taskName": task.name, "executedAt": now_ms() }),
        );
        self.deps.history.record(
            &task_id,
            TaskExecutionResult {
                task_id: task_id.clone(),
                executed_at: now_ms(),
                status: ExecutionStatus::Running,
                error: None,
                session_id: session_id.clone(),
                message_count: 0,
                extra: Map::new(),
            },
        );

        let outcome: Result<ChatResult> = async {
            if !continuation {
                self.deps.service.create_session(
                    &session_id,
                    SessionMeta {
                        task_id: Some(task.id.clone()),
                        agent_id: Some(task.agent_id.clone()),
                        model_id: Some(task.model_id.clone()),
                        project_directory: task.project_directory.clone(),
                    },
                )?;
            }
            self.deps
                .service
                .chat(
                    &session_id,
                    &task.agent_config(),
                    &prompt,
                    &scheduled_run_options(),
                )
                .await
        }
        .await;

        match outcome {
            Ok(result) => {
                let message_count = self.deps.service.get_session_history(&session_id).len();
                let executed_at = now_ms();
                self.deps.history.record(
                    &task_id,
                    TaskExecutionResult {
                        task_id: task_id.clone(),
                        executed_at,
                        status: ExecutionStatus::Success,
                        error: None,
                        session_id: session_id.clone(),
                        message_count,
                        extra: Map::new(),
                    },
                );
                let continue_session = task.continue_session == Some(true);
                let updated = self.update_stored(&task_id, true, |t| {
                    t.run_count += 1;
                    t.last_run = Some(now_ms());
                    t.next_run = calculate_next_run(&t.cron_expression);
                    t.last_error = None;
                    if continue_session {
                        t.last_session_id = Some(session_id.clone());
                    }
                });
                let ai_message = ai_message_of(&result);
                show_background_agent_notification(
                    self.deps.config.as_ref(),
                    self.deps.notifier.as_ref(),
                    &task_id,
                    &task.name,
                    true,
                    Some(&ai_message),
                    None,
                );
                let now = now_ms();
                let mut payload = json!({
                    "taskId": task_id,
                    "taskName": task.name,
                    "success": true,
                    "aiMessage": ai_message,
                    "executedAt": now,
                    "executionTime": now - executed_at,
                    "sessionId": session_id,
                    "messageCount": message_count,
                    "toolExecutions": result.tool_executions.as_ref().map_or(0, Vec::len),
                    "runCount": updated.as_ref().map_or(task.run_count + 1, |t| t.run_count),
                });
                if let Some(next) = updated.as_ref().and_then(|t| t.next_run) {
                    payload["nextRun"] = json!(next);
                }
                self.deps.events.emit(TASK_NOTIFICATION_EVENT, payload);
                tracing::info!(task_id, session_id, "Scheduled task executed successfully");
            }
            Err(error) => {
                let message = error.to_string();
                tracing::error!(task_id, session_id, error = %message, "Scheduled task execution failed");
                self.deps.history.record(
                    &task_id,
                    TaskExecutionResult {
                        task_id: task_id.clone(),
                        executed_at: now_ms(),
                        status: ExecutionStatus::Failed,
                        error: Some(message.clone()),
                        session_id: session_id.clone(),
                        message_count: 0,
                        extra: Map::new(),
                    },
                );
                self.update_stored(&task_id, true, |t| {
                    t.last_error = Some(message.clone());
                    t.last_run = Some(now_ms());
                    t.next_run = calculate_next_run(&t.cron_expression);
                });
                show_background_agent_notification(
                    self.deps.config.as_ref(),
                    self.deps.notifier.as_ref(),
                    &task_id,
                    &task.name,
                    false,
                    None,
                    Some(&message),
                );
                self.deps.events.emit(
                    TASK_NOTIFICATION_EVENT,
                    json!({
                        "taskId": task_id,
                        "taskName": task.name,
                        "success": false,
                        "error": message,
                        "executedAt": now_ms(),
                    }),
                );
            }
        }

        if !manual {
            self.update_stored(&task_id, false, |t| {
                t.is_executing = Some(false);
                t.last_execution_started = None;
            });
        }
    }
}

/// One cron job: node-cron's `Runner` heartbeat loop. Runs are spawned, not awaited, so a long
/// run does not delay the next slot (the duplicate guard in `execute_task` decides).
async fn run_cron_job(
    scheduler: Weak<BackgroundAgentScheduler>,
    task_id: String,
    expr: CronExpression,
    first: i64,
) {
    let mut expected = first;
    loop {
        let delay = (expected - now_ms()).clamp(0, HEARTBEAT_MAX_DELAY.as_millis() as i64);
        tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        let current = now_ms() / 1000 * 1000;
        let plan = plan_beat(expected, current, MISSED_EXECUTION_TOLERANCE_MS, |t| {
            expr.next_after_ms(&Local, t)
        });
        for slot in &plan.missed {
            tracing::warn!(
                task_id,
                slot,
                "missed execution; skipping (system sleep or blocked process)"
            );
        }
        if plan.run.is_some() {
            let Some(s) = scheduler.upgrade() else { return };
            let runtime = s.deps.runtime.clone();
            let id = task_id.clone();
            runtime.spawn(async move { s.execute_task(&id).await });
        }
        match plan.next {
            Some(n) => expected = n,
            None => return,
        }
    }
}
