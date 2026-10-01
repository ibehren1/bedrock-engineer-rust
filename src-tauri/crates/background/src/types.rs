//! `src/main/api/bedrock/services/backgroundAgent/types.ts` and the IPC parameter shapes.
//!
//! Stored shapes keep unknown keys (`extra`) so a round trip through Rust does not drop fields
//! written by another version of the app.

use agents::{AgentRunConfig, AgentRunOptions};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::time::Duration;

/// Timestamps are `Date.now()` numbers; accept any JSON number.
pub(crate) fn de_ms<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .unwrap_or(0))
}

/// `null` (which TS writes for an unset field) as the type's default.
pub(crate) fn de_null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

pub(crate) fn de_opt_ms<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    Ok(v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))))
}

/// `ScheduleConfig.agentConfig` (a `BackgroundAgentConfig`). `inferenceConfig` is kept as JSON so
/// it is stored exactly as sent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScheduleAgentConfig {
    pub model_id: String,
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<Value>,
}

/// `ScheduleConfig` (`background-agent:schedule-task` / `update-task`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScheduleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub name: String,
    pub cron_expression: String,
    pub agent_config: ScheduleAgentConfig,
    pub wake_word: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_session: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_session_prompt: Option<String>,
}

/// `ScheduledTask`, as stored in the config key `backgroundAgentScheduledTasks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    #[serde(default, deserialize_with = "de_null_default")]
    pub id: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub cron_expression: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub agent_id: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub wake_word: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub enabled: bool,
    #[serde(default, deserialize_with = "de_ms")]
    pub created_at: i64,
    #[serde(
        default,
        deserialize_with = "de_opt_ms",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_run: Option<i64>,
    #[serde(
        default,
        deserialize_with = "de_opt_ms",
        skip_serializing_if = "Option::is_none"
    )]
    pub next_run: Option<i64>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub run_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_session: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_session_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_executing: Option<bool>,
    #[serde(
        default,
        deserialize_with = "de_opt_ms",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_execution_started: Option<i64>,
    /// Keys this version does not know about.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ScheduledTask {
    /// The `BackgroundAgentConfig` a scheduled run uses.
    pub fn agent_config(&self) -> AgentRunConfig {
        AgentRunConfig {
            model_id: self.model_id.clone(),
            agent_id: self.agent_id.clone(),
            project_directory: self.project_directory.clone(),
            inference_config: self
                .inference_config
                .clone()
                .and_then(|v| serde_json::from_value(v).ok()),
            ..Default::default()
        }
    }

    pub(crate) fn is_executing(&self) -> bool {
        self.is_executing == Some(true)
    }
}

/// `TaskExecutionResult.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
}

/// `TaskExecutionResult` (execution history entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionResult {
    #[serde(default, deserialize_with = "de_null_default")]
    pub task_id: String,
    #[serde(default, deserialize_with = "de_ms")]
    pub executed_at: i64,
    pub status: ExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub session_id: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub message_count: usize,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `BackgroundAgentOptions`; unset fields take the `chat()` defaults (tools on, 500 rounds,
/// three hours).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackgroundAgentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_tool_execution: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_executions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

impl BackgroundAgentOptions {
    pub fn to_run_options(&self) -> AgentRunOptions {
        let d = AgentRunOptions::default();
        AgentRunOptions {
            enable_tool_execution: self
                .enable_tool_execution
                .unwrap_or(d.enable_tool_execution),
            max_tool_executions: self.max_tool_executions.unwrap_or(d.max_tool_executions),
            timeout: self
                .timeout_ms
                .map(Duration::from_millis)
                .unwrap_or(d.timeout),
        }
    }
}

/// `params.config` of `background-agent:chat`. Only these four fields are passed on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatConfigParams {
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
}

/// `background-agent:chat` params (`{ sessionId, config, userMessage, options? }`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatParams {
    pub session_id: String,
    pub config: ChatConfigParams,
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<BackgroundAgentOptions>,
}

/// `background-agent:continue-session` params (`{ sessionId, taskId, userMessage, options? }`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContinueSessionParams {
    pub session_id: String,
    pub task_id: String,
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<BackgroundAgentOptions>,
}

/// `BackgroundAgentScheduler.getStats()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStats {
    pub total_tasks: usize,
    pub enabled_tasks: usize,
    pub disabled_tasks: usize,
    pub total_executions: u64,
    pub tasks_with_errors: usize,
    pub active_cron_jobs: usize,
}
