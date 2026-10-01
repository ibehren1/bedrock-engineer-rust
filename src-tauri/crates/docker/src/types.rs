//! Types for the per-chat Docker sandbox. Port of `src/main/api/docker/types.ts`.
//!
//! Field names serialize in camelCase so `sandbox.json` files written by the Electron build
//! read back unchanged, and the renderer sees the same shapes.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Default image for a generated sandbox. Bare — the agent installs what it needs.
pub const DEFAULT_SANDBOX_IMAGE: &str = "ubuntu:26.04";

/// Service name used by the generated single-service compose file.
pub const DEFAULT_SERVICE_NAME: &str = "main";

/// Mount point of the user's projectPath inside every sandbox container.
pub const WORKSPACE_MOUNT: &str = "/workspace";

/// Folder under projectPath that holds every sandbox's compose/env/data files.
pub const SANDBOX_ROOT_DIRNAME: &str = "docker-sandboxes";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComposeFlavor {
    Plugin,
    Standalone,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerAvailability {
    /// `docker` CLI present and responding.
    pub docker_installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_version: Option<String>,
    /// Daemon reachable (`docker info` succeeded).
    pub daemon_running: bool,
    /// Which compose implementation is usable, if any.
    pub compose: ComposeFlavor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_version: Option<String>,
    /// Human-readable reason the sandbox can't run, when it can't.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Platform-specific install steps, present whenever something is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_guidance: Option<String>,
    /// ISO-8601 timestamp (what `JSON.stringify(new Date())` produces).
    pub last_checked: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPortMapping {
    /// Port published on the host.
    pub host: u16,
    /// Port inside the container.
    pub container: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataVolumeSpec {
    pub name: String,
    pub container_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxServiceSpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Container command. Defaults to `sleep infinity` so the container stays up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<SandboxPortMapping>>,
    /// Environment variables written to the service in the compose file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Map<String, Value>>,
    /// Extra data directories: `<sandboxDir>/data/<name>` → the given container path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_volumes: Option<Vec<DataVolumeSpec>>,
}

impl SandboxServiceSpec {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSandboxOptions {
    /// Structured service definitions. Ignored when `compose_yaml` is supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<SandboxServiceSpec>>,
    /// Raw compose YAML authored by the agent. Validated and rewritten before use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_yaml: Option<String>,
    /// Additional variables written to the sandbox `.env` file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Map<String, Value>>,
    /// Recreate from scratch even if a sandbox already exists for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recreate: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxServiceSummary {
    pub name: String,
    pub image: String,
    pub ports: Vec<SandboxPortMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxMetadata {
    pub session_id: String,
    /// Compose project name, e.g. `bedrock-sandbox-session-1756900000000`.
    pub project_name: String,
    /// Absolute path of the sandbox folder.
    pub directory: String,
    /// Absolute path of the compose file.
    pub compose_file: String,
    /// Absolute path of the projectPath bind-mounted at /workspace.
    pub project_path: String,
    pub services: Vec<SandboxServiceSummary>,
    /// True when the stack is driven by `docker run` because compose is absent.
    pub composeless: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxRunState {
    Running,
    Stopped,
    Partial,
    Missing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRow {
    pub service: String,
    pub name: String,
    pub state: String,
    pub ports: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxStatus {
    pub exists: bool,
    pub state: SandboxRunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SandboxMetadata>,
    pub containers: Vec<ContainerRow>,
}

impl SandboxStatus {
    pub fn missing() -> Self {
        Self {
            exists: false,
            state: SandboxRunState::Missing,
            metadata: None,
            containers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxExecOptions {
    /// Service to exec into. Defaults to the sandbox's first service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Working directory inside the container. Defaults to /workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Start the command in the background and return immediately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detach: Option<bool>,
    /// Seconds before the command is abandoned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub command: String,
    pub detached: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    /// PID of the local `docker` client process, usable for a stdin follow-up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_info: Option<ProcessInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRemoveOptions {
    /// Also delete the sandbox folder, including mapped data volumes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_data: Option<bool>,
}

/// User-configurable resource limits, persisted under the `dockerSandboxTool` store key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerSandboxConfig {
    pub memory_limit: String,
    pub cpu_limit: f64,
    /// Per-command timeout in seconds.
    pub timeout: f64,
    /// Set once the user has acknowledged what the interactive terminal is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_acknowledged: Option<bool>,
}

impl Default for DockerSandboxConfig {
    /// `DEFAULT_SANDBOX_CONFIG`.
    fn default() -> Self {
        Self {
            memory_limit: "2g".to_string(),
            cpu_limit: 2.0,
            timeout: 300.0,
            terminal_acknowledged: None,
        }
    }
}

/// `DEFAULT_SANDBOX_CONFIG` as a function, for call sites that read like the TS.
pub fn default_sandbox_config() -> DockerSandboxConfig {
    DockerSandboxConfig::default()
}

pub const VALID_MEMORY_LIMITS: [&str; 6] = ["256m", "512m", "1g", "2g", "4g", "8g"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_ts() {
        let config = DockerSandboxConfig::default();
        assert_eq!(config.memory_limit, "2g");
        assert_eq!(config.cpu_limit, 2.0);
        assert_eq!(config.timeout, 300.0);
        assert!(VALID_MEMORY_LIMITS.contains(&config.memory_limit.as_str()));
    }

    #[test]
    fn metadata_round_trips_electron_json() {
        let raw = r#"{
  "sessionId": "session_1",
  "projectName": "bedrock-sandbox-session-1",
  "directory": "/p/docker-sandboxes/session-abc123",
  "composeFile": "/p/docker-sandboxes/session-abc123/docker-compose.yml",
  "projectPath": "/p",
  "services": [{ "name": "main", "image": "ubuntu:26.04", "ports": [{ "host": 3000, "container": 3000 }] }],
  "composeless": false,
  "createdAt": "2026-09-03T14:22:15.000Z",
  "updatedAt": "2026-09-03T14:22:15.000Z"
}"#;
        let parsed: SandboxMetadata = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.services[0].ports[0].host, 3000);
        let back = serde_json::to_value(&parsed).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(raw).unwrap());
    }
}
