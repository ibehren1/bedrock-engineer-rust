//! Types for the Code Interpreter. Port of `interpreter/types.ts`.

use serde::{Deserialize, Serialize, Serializer};

/// Supported programming languages (currently Python only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupportedLanguage {
    Python,
}

/// Python execution environments with different library sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PythonEnvironment {
    Basic,
    Datascience,
}

impl PythonEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            PythonEnvironment::Basic => "basic",
            PythonEnvironment::Datascience => "datascience",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerImageConfig {
    pub name: &'static str,
    pub tag: &'static str,
    pub environment: PythonEnvironment,
    pub libraries: Vec<&'static str>,
    pub system_packages: Vec<&'static str>,
    pub environment_variables: Vec<(&'static str, &'static str)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerBuildResult {
    pub success: bool,
    pub image_name: String,
    pub build_time: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFile {
    /// Host-side file path.
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Execute,
    Status,
    Cancel,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn is_final(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }
}

/// Tool input, after [`crate::interpreter::CodeInterpreter::validate_input`] passed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeInterpreterInput {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_files: Option<Vec<InputFile>>,
    #[serde(default, rename = "async", skip_serializing_if = "Option::is_none")]
    pub run_async: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<Operation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_filter: Option<TaskStatus>,
}

/// Code execution result (internal).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    /// Milliseconds.
    pub execution_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeInterpreterResultBody {
    pub code: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    /// Generated files (full host paths).
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeInterpreterResult {
    pub success: bool,
    pub name: String,
    /// The executed code, for UI display.
    pub code: String,
    pub message: String,
    /// Combined stdout + file info for easy consumption.
    pub output: String,
    /// stderr, only when an error occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub execution_time: u64,
    pub result: CodeInterpreterResultBody,
}

/// User-configurable execution configuration. Every field optional, as `Partial<…>` was.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialExecutionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<PythonEnvironment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionConfig {
    /// Seconds.
    pub timeout: f64,
    pub memory_limit: String,
    pub cpu_limit: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<PythonEnvironment>,
}

impl From<&ExecutionConfig> for PartialExecutionConfig {
    fn from(config: &ExecutionConfig) -> Self {
        Self {
            timeout: Some(config.timeout),
            memory_limit: Some(config.memory_limit.clone()),
            cpu_limit: Some(config.cpu_limit),
            environment: config.environment,
        }
    }
}

/// Workspace configuration (mostly internal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub base_path: String,
    pub session_id: String,
    pub max_files: usize,
    pub max_file_size: u64,
    pub cleanup_on_exit: bool,
}

pub(crate) fn ser_iso<S: Serializer>(ms: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&iso_from_ms(*ms))
}

pub(crate) fn ser_iso_opt<S: Serializer>(
    ms: &Option<i64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match ms {
        Some(ms) => serializer.serialize_str(&iso_from_ms(*ms)),
        None => serializer.serialize_none(),
    }
}

/// `new Date(ms).toISOString()`.
pub(crate) fn iso_from_ms(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Task information for async execution. Dates serialize as ISO strings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
    #[serde(
        serialize_with = "ser_iso_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub started_at: Option<i64>,
    #[serde(
        serialize_with = "ser_iso_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub completed_at: Option<i64>,
    pub code: String,
    pub environment: PythonEnvironment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_files: Option<Vec<InputFile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CodeInterpreterResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 0-100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncTaskResultBody {
    pub task_id: String,
    pub status: TaskStatus,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<CodeInterpreterResult>,
}

/// Returned immediately when starting, checking, or cancelling an async task.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncTaskResult {
    pub success: bool,
    pub name: String,
    pub task_id: String,
    pub status: TaskStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    pub result: AsyncTaskResultBody,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub total: usize,
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListResultBody {
    pub tasks: Vec<TaskInfo>,
    pub summary: TaskSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_filter: Option<TaskStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListResult {
    pub success: bool,
    pub name: String,
    pub operation: String,
    pub tasks: Vec<TaskInfo>,
    pub summary: TaskSummary,
    pub message: String,
    pub result: TaskListResultBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskManagerConfig {
    pub max_concurrent_tasks: usize,
    /// Milliseconds.
    pub task_timeout: u64,
    pub max_task_history: usize,
    /// Milliseconds.
    pub cleanup_interval: u64,
}

impl Default for TaskManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 3,
            task_timeout: 300_000,
            max_task_history: 50,
            cleanup_interval: 60_000,
        }
    }
}
