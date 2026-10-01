//! Crate error type. `Display` is the message the TS code threw, which the app returns as the
//! command error string.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `new Error(`Invalid cron expression: ${cronExpression}`)`.
    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),
    /// `new Error(`Task not found: ${taskId}`)`.
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    /// The `background-agent:update-task` handler's `Failed to update task: ${taskId}`.
    #[error("Failed to update task: {0}")]
    UpdateFailed(String),
    /// `new Error(`No execution result found for task: ${taskId}`)`.
    #[error("No execution result found for task: {0}")]
    NoExecutionResult(String),
    /// `new Error('userDataPath is not set in store')`.
    #[error("userDataPath is not set in store")]
    UserDataPathMissing,
    /// A cron job could not be started (e.g. the expression never matches).
    #[error("{0}")]
    Schedule(String),
    /// Session persistence failures (`Failed to write session file after 3 attempts: …`).
    #[error("{0}")]
    Session(String),
    /// Failures from the agent run.
    #[error(transparent)]
    Agent(#[from] agents::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
