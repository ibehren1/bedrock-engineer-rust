//! Command execution (`src/preload/tools/handlers/command` + `src/main/api/command`).

pub mod output_patterns;
mod service;
mod tool;

pub use service::{
    is_command_allowed, CommandConfig, CommandExecutionResult, CommandResult, CommandService,
    DetachedProcessInfo, ProcessInfo, EXECUTE_TIMEOUT, STDIN_TIMEOUT,
};
pub use tool::ExecuteCommandTool;

use async_trait::async_trait;
use serde_json::Value;

/// Options for [`CommandSandbox::exec`] (`docker-sandbox-exec` `options`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SandboxExecOptions {
    pub service: Option<String>,
    /// Only absolute (`/workspace/...`) paths are forwarded.
    pub cwd: Option<String>,
    pub detach: Option<bool>,
}

/// Extension point for Docker sandbox routing (Task 10: `docker-sandbox-exec`,
/// `docker-sandbox-has-pid`, `docker-sandbox-send-input`).
///
/// Results are the JSON objects those handlers return (`stdout`, `stderr`, `exitCode`,
/// `processInfo`, `requiresInput`, `prompt`, `detached`); errors are `Error.message`.
#[async_trait]
pub trait CommandSandbox: Send + Sync {
    async fn has_pid(&self, pid: f64) -> bool;
    async fn send_input(&self, pid: f64, stdin: &str) -> std::result::Result<Value, String>;
    async fn exec(
        &self,
        session_id: &str,
        command: &str,
        options: SandboxExecOptions,
    ) -> std::result::Result<Value, String>;
}
