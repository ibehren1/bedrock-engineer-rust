//! Async Code Interpreter task bookkeeping. Port of `TaskManager.ts`.

use std::sync::{Arc, Mutex, Weak};

use serde_json::json;

use super::logger::ToolLogger;
use super::types::{
    CodeInterpreterResult, InputFile, PythonEnvironment, TaskInfo, TaskManagerConfig, TaskStatus,
    TaskSummary,
};
use crate::util::now_ms;

#[derive(Default)]
struct TaskState {
    /// Insertion-ordered, like the TS `Map`.
    tasks: Vec<TaskInfo>,
    running: Vec<String>,
    timer_armed: bool,
}

struct Inner {
    state: Mutex<TaskState>,
    logger: Arc<dyn ToolLogger>,
    config: TaskManagerConfig,
}

pub struct TaskManager {
    inner: Arc<Inner>,
}

impl TaskManager {
    pub fn new(logger: Arc<dyn ToolLogger>, config: TaskManagerConfig) -> Self {
        logger.info(
            "TaskManager initialized",
            json!({
                "maxConcurrentTasks": config.max_concurrent_tasks,
                "taskTimeout": config.task_timeout,
            }),
        );
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(TaskState::default()),
                logger,
                config,
            }),
        }
    }

    /// `task_<ms>_<8 hex>`.
    fn generate_task_id() -> String {
        let uuid = uuid::Uuid::new_v4().to_string();
        format!("task_{}_{}", now_ms(), &uuid[..8])
    }

    pub fn create_task(
        &self,
        code: &str,
        environment: PythonEnvironment,
        input_files: Option<Vec<InputFile>>,
    ) -> TaskInfo {
        let task = TaskInfo {
            task_id: Self::generate_task_id(),
            status: TaskStatus::Pending,
            created_at: now_ms(),
            started_at: None,
            completed_at: None,
            code: code.to_string(),
            environment,
            input_files: input_files.clone(),
            result: None,
            error: None,
            progress: Some(0),
        };
        self.inner.state.lock().unwrap().tasks.push(task.clone());
        self.start_cleanup_timer();
        self.inner.logger.info(
            "Task created",
            json!({
                "taskId": task.task_id,
                "codeLength": code.len(),
                "environment": environment,
                "inputFileCount": input_files.map(|f| f.len()).unwrap_or(0),
            }),
        );
        task
    }

    pub fn get_task(&self, task_id: &str) -> Option<TaskInfo> {
        self.inner
            .state
            .lock()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task_id == task_id)
            .cloned()
    }

    pub fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        progress: Option<u8>,
    ) -> bool {
        update_status(&self.inner, task_id, status, progress)
    }

    pub fn set_task_result(&self, task_id: &str, result: CodeInterpreterResult) -> bool {
        let (success, execution_time) = (result.success, result.execution_time);
        {
            let mut state = self.inner.state.lock().unwrap();
            let Some(task) = state.tasks.iter_mut().find(|t| t.task_id == task_id) else {
                drop(state);
                self.inner.logger.warn(
                    "Task not found for result setting",
                    json!({ "taskId": task_id }),
                );
                return false;
            };
            task.result = Some(result);
        }
        self.update_task_status(task_id, TaskStatus::Completed, Some(100));
        self.inner.logger.info(
            "Task result set",
            json!({ "taskId": task_id, "success": success, "executionTime": execution_time }),
        );
        true
    }

    pub fn set_task_error(&self, task_id: &str, error: &str) -> bool {
        set_error(&self.inner, task_id, error)
    }

    pub fn cancel_task(&self, task_id: &str) -> bool {
        let status = self.get_task(task_id).map(|t| t.status);
        match status {
            None => {
                self.inner.logger.warn(
                    "Task not found for cancellation",
                    json!({ "taskId": task_id }),
                );
                false
            }
            Some(status) if status.is_final() => {
                self.inner.logger.warn(
                    "Cannot cancel task in final state",
                    json!({ "taskId": task_id, "currentStatus": status }),
                );
                false
            }
            Some(_) => {
                self.update_task_status(task_id, TaskStatus::Cancelled, Some(0));
                self.inner
                    .logger
                    .info("Task cancelled", json!({ "taskId": task_id }));
                true
            }
        }
    }

    pub fn can_start_new_task(&self) -> bool {
        let running = self.inner.state.lock().unwrap().running.len();
        let can_start = running < self.inner.config.max_concurrent_tasks;
        if !can_start {
            self.inner.logger.debug(
                "Cannot start new task - concurrent limit reached",
                json!({ "runningCount": running, "maxConcurrent": self.inner.config.max_concurrent_tasks }),
            );
        }
        can_start
    }

    /// With a filter: matching tasks in creation order. Without: newest first.
    pub fn get_all_tasks(&self, status_filter: Option<TaskStatus>) -> Vec<TaskInfo> {
        let mut tasks = self.inner.state.lock().unwrap().tasks.clone();
        if let Some(filter) = status_filter {
            tasks.retain(|t| t.status == filter);
            return tasks;
        }
        tasks.sort_by_key(|t| std::cmp::Reverse(t.created_at));
        tasks
    }

    pub fn get_running_task_count(&self) -> usize {
        self.inner.state.lock().unwrap().running.len()
    }

    pub fn get_task_stats(&self) -> TaskSummary {
        let state = self.inner.state.lock().unwrap();
        let count = |status| state.tasks.iter().filter(|t| t.status == status).count();
        TaskSummary {
            total: state.tasks.len(),
            pending: count(TaskStatus::Pending),
            running: count(TaskStatus::Running),
            completed: count(TaskStatus::Completed),
            failed: count(TaskStatus::Failed),
            cancelled: count(TaskStatus::Cancelled),
        }
    }

    /// Armed on the first task rather than at construction, so users who never touch the
    /// Code Interpreter never carry a timer.
    fn start_cleanup_timer(&self) {
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.timer_armed {
                return;
            }
            let Ok(handle) = tokio::runtime::Handle::try_current() else {
                return;
            };
            state.timer_armed = true;
            let weak: Weak<Inner> = Arc::downgrade(&self.inner);
            let interval = std::time::Duration::from_millis(self.inner.config.cleanup_interval);
            handle.spawn(async move {
                let mut ticker = tokio::time::interval(interval);
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    let Some(inner) = weak.upgrade() else { return };
                    if !cleanup_old_tasks(&inner) {
                        return;
                    }
                }
            });
        }
    }

    /// Run one cleanup pass now (the timer's body).
    pub fn cleanup_old_tasks(&self) {
        cleanup_old_tasks(&self.inner);
    }

    /// Cancel running tasks and forget everything.
    pub fn dispose(&self) {
        let running = self.inner.state.lock().unwrap().running.clone();
        for task_id in &running {
            self.cancel_task(task_id);
        }
        let mut state = self.inner.state.lock().unwrap();
        self.inner.logger.info(
            "TaskManager disposed",
            json!({ "totalTasks": state.tasks.len(), "cancelledTasks": state.running.len() }),
        );
        state.tasks.clear();
        state.running.clear();
        state.timer_armed = false;
    }
}

fn update_status(inner: &Inner, task_id: &str, status: TaskStatus, progress: Option<u8>) -> bool {
    let mut state = inner.state.lock().unwrap();
    let Some(index) = state.tasks.iter().position(|t| t.task_id == task_id) else {
        drop(state);
        inner.logger.warn(
            "Task not found for status update",
            json!({ "taskId": task_id, "status": status }),
        );
        return false;
    };
    let now = now_ms();
    let task = &mut state.tasks[index];
    let old_status = task.status;
    task.status = status;
    if let Some(progress) = progress {
        task.progress = Some(progress.min(100));
    }
    let progress = task.progress;
    match status {
        TaskStatus::Running => {
            task.started_at = Some(now);
            if !state.running.iter().any(|id| id == task_id) {
                state.running.push(task_id.to_string());
            }
        }
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
            task.completed_at = Some(now);
            state.running.retain(|id| id != task_id);
        }
        TaskStatus::Pending => {}
    }
    drop(state);
    inner.logger.debug(
        "Task status updated",
        json!({ "taskId": task_id, "oldStatus": old_status, "newStatus": status, "progress": progress }),
    );
    true
}

fn set_error(inner: &Inner, task_id: &str, error: &str) -> bool {
    {
        let mut state = inner.state.lock().unwrap();
        let Some(task) = state.tasks.iter_mut().find(|t| t.task_id == task_id) else {
            drop(state);
            inner.logger.warn(
                "Task not found for error setting",
                json!({ "taskId": task_id }),
            );
            return false;
        };
        task.error = Some(error.to_string());
    }
    update_status(inner, task_id, TaskStatus::Failed, Some(0));
    inner.logger.error(
        "Task error set",
        json!({ "taskId": task_id, "error": error }),
    );
    true
}

/// Trim finished history and fail tasks that ran too long. Returns false once there is
/// nothing left to watch, which stops the timer.
fn cleanup_old_tasks(inner: &Inner) -> bool {
    let timed_out: Vec<String> = {
        let mut state = inner.state.lock().unwrap();
        let mut finished: Vec<(String, i64)> = state
            .tasks
            .iter()
            .filter(|t| t.status.is_final())
            .map(|t| (t.task_id.clone(), t.completed_at.unwrap_or(0)))
            .collect();
        if finished.len() > inner.config.max_task_history {
            finished.sort_by_key(|(_, at)| *at);
            let remove: Vec<String> = finished[..finished.len() - inner.config.max_task_history]
                .iter()
                .map(|(id, _)| id.clone())
                .collect();
            state.tasks.retain(|t| !remove.contains(&t.task_id));
            inner.logger.debug(
                "Cleaned up old tasks",
                json!({ "removedCount": remove.len(), "remainingCount": state.tasks.len() }),
            );
        }
        let now = now_ms();
        state
            .tasks
            .iter()
            .filter(|t| {
                t.status == TaskStatus::Running
                    && t.started_at
                        .is_some_and(|at| (now - at) as u64 > inner.config.task_timeout)
            })
            .map(|t| t.task_id.clone())
            .collect()
    };
    for task_id in timed_out {
        set_error(inner, &task_id, "Task timed out");
        inner.logger.warn(
            "Task timed out and marked as failed",
            json!({ "taskId": task_id }),
        );
    }
    let mut state = inner.state.lock().unwrap();
    if state.tasks.is_empty() {
        state.timer_armed = false;
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::logger::LogFacadeLogger;

    fn manager(config: TaskManagerConfig) -> TaskManager {
        TaskManager::new(Arc::new(LogFacadeLogger), config)
    }

    #[test]
    fn tracks_the_task_lifecycle() {
        let tasks = manager(TaskManagerConfig::default());
        let task = tasks.create_task("print(1)", PythonEnvironment::Basic, None);
        assert!(task.task_id.starts_with("task_"));
        assert_eq!(task.status, TaskStatus::Pending);

        assert!(tasks.update_task_status(&task.task_id, TaskStatus::Running, Some(10)));
        assert_eq!(tasks.get_running_task_count(), 1);
        assert!(tasks.get_task(&task.task_id).unwrap().started_at.is_some());

        assert!(tasks.cancel_task(&task.task_id));
        assert_eq!(
            tasks.get_task(&task.task_id).unwrap().status,
            TaskStatus::Cancelled
        );
        assert!(
            !tasks.cancel_task(&task.task_id),
            "final states cannot be cancelled"
        );
        assert!(!tasks.cancel_task("nope"));
        assert_eq!(tasks.get_running_task_count(), 0);

        let stats = tasks.get_task_stats();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.cancelled, 1);
    }

    #[test]
    fn enforces_the_concurrency_limit_and_errors() {
        let tasks = manager(TaskManagerConfig {
            max_concurrent_tasks: 1,
            ..Default::default()
        });
        let first = tasks.create_task("a", PythonEnvironment::Datascience, None);
        assert!(tasks.can_start_new_task());
        tasks.update_task_status(&first.task_id, TaskStatus::Running, None);
        assert!(!tasks.can_start_new_task());
        assert!(tasks.set_task_error(&first.task_id, "boom"));
        let failed = tasks.get_task(&first.task_id).unwrap();
        assert_eq!(failed.status, TaskStatus::Failed);
        assert_eq!(failed.error.as_deref(), Some("boom"));
        assert!(tasks.can_start_new_task());
    }

    #[test]
    fn lists_newest_first_or_filtered_and_trims_history() {
        let tasks = manager(TaskManagerConfig {
            max_task_history: 1,
            ..Default::default()
        });
        let a = tasks.create_task("a", PythonEnvironment::Basic, None);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = tasks.create_task("b", PythonEnvironment::Basic, None);
        let all: Vec<String> = tasks
            .get_all_tasks(None)
            .into_iter()
            .map(|t| t.task_id)
            .collect();
        assert_eq!(all, [b.task_id.clone(), a.task_id.clone()]);

        tasks.set_task_error(&a.task_id, "x");
        std::thread::sleep(std::time::Duration::from_millis(2));
        tasks.set_task_error(&b.task_id, "y");
        assert_eq!(tasks.get_all_tasks(Some(TaskStatus::Failed)).len(), 2);

        tasks.cleanup_old_tasks();
        let remaining: Vec<String> = tasks
            .get_all_tasks(None)
            .into_iter()
            .map(|t| t.task_id)
            .collect();
        assert_eq!(remaining, [b.task_id]);

        tasks.dispose();
        assert_eq!(tasks.get_task_stats().total, 0);
    }

    #[test]
    fn serializes_dates_as_iso_strings() {
        let tasks = manager(TaskManagerConfig::default());
        let task = tasks.create_task("a", PythonEnvironment::Basic, None);
        let value = serde_json::to_value(&task).unwrap();
        let created = value["createdAt"].as_str().unwrap();
        assert!(created.ends_with('Z') && created.contains('T'));
        assert!(value.get("startedAt").is_none());
        assert_eq!(value["environment"], "basic");
    }
}
