//! Types for the per-chat Docker sandbox. Port of `src/main/api/docker/types.ts`.
//!
//! Field names serialize in camelCase so `sandbox.json` files written by the Electron build
//! read back unchanged, and the renderer sees the same shapes.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Package manager family of a sandbox OS; decides the install hint given to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxOsFamily {
    /// apt-get (Debian, Ubuntu).
    Debian,
    /// dnf (Amazon Linux, RHEL and its clones).
    Rhel,
}

/// An operating system the sandbox can be built on. Every image is bare and runs as root;
/// the agent installs what it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxOs {
    /// Stable id stored in settings (`dockerSandboxTool.os`) and accepted by the tool's `os`.
    pub id: &'static str,
    pub label: &'static str,
    pub image: &'static str,
    pub family: SandboxOsFamily,
}

/// Every OS the sandbox offers. Mirrored for the settings UI by `SANDBOX_OS_OPTIONS` in
/// `src/types/sandbox.ts`.
pub const SANDBOX_OS_CATALOG: [SandboxOs; 7] = [
    SandboxOs {
        id: "ubuntu",
        label: "Ubuntu 26.04",
        image: "ubuntu:26.04",
        family: SandboxOsFamily::Debian,
    },
    SandboxOs {
        id: "debian13",
        label: "Debian 13",
        image: "debian:13-slim",
        family: SandboxOsFamily::Debian,
    },
    SandboxOs {
        id: "amazonlinux2023",
        label: "Amazon Linux 2023",
        image: "public.ecr.aws/amazonlinux/amazonlinux:2023",
        family: SandboxOsFamily::Rhel,
    },
    SandboxOs {
        id: "amazonlinux2027",
        label: "Amazon Linux 2027",
        image: "public.ecr.aws/amazonlinux/amazonlinux:2027",
        family: SandboxOsFamily::Rhel,
    },
    SandboxOs {
        id: "almalinux9",
        label: "AlmaLinux 9",
        image: "almalinux:9",
        family: SandboxOsFamily::Rhel,
    },
    SandboxOs {
        id: "rocky9",
        label: "Rocky Linux 9",
        image: "rockylinux/rockylinux:9",
        family: SandboxOsFamily::Rhel,
    },
    SandboxOs {
        id: "ubi9",
        label: "Red Hat UBI 9",
        image: "registry.access.redhat.com/ubi9/ubi:latest",
        family: SandboxOsFamily::Rhel,
    },
];

/// OS used when the settings name none (or one this build doesn't know). Debian slim was the
/// quickest to pull and to install common packages on among images with full package repos.
pub const DEFAULT_SANDBOX_OS_ID: &str = "debian13";

/// Default image for a generated sandbox: the default OS's image.
pub const DEFAULT_SANDBOX_IMAGE: &str = "debian:13-slim";

/// The catalog entry with this id, if any.
pub fn find_sandbox_os(id: &str) -> Option<&'static SandboxOs> {
    SANDBOX_OS_CATALOG.iter().find(|os| os.id == id)
}

/// The default OS's catalog entry.
pub fn default_sandbox_os() -> &'static SandboxOs {
    find_sandbox_os(DEFAULT_SANDBOX_OS_ID).expect("default OS is in the catalog")
}

impl SandboxOsFamily {
    /// What the agent is told about installing packages on this family.
    pub fn package_hint(self) -> &'static str {
        match self {
            Self::Debian => {
                "Run \"apt-get update\" before installing packages with \"apt-get install -y\"."
            }
            Self::Rhel => "Install packages with \"dnf install -y\" (no separate update step).",
        }
    }
}

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
    /// Catalog id of the OS for services that name no image; overrides the user's setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
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

/// User-configurable OS and resource limits, persisted under the `dockerSandboxTool` store key.
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
    /// Catalog id of the OS new sandboxes use. Unset or unknown means the default OS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
}

impl DockerSandboxConfig {
    /// The OS new sandboxes use, falling back to the default for an unset or unknown id.
    pub fn sandbox_os(&self) -> &'static SandboxOs {
        self.os
            .as_deref()
            .and_then(find_sandbox_os)
            .unwrap_or_else(default_sandbox_os)
    }
}

impl Default for DockerSandboxConfig {
    /// `DEFAULT_SANDBOX_CONFIG`.
    fn default() -> Self {
        Self {
            memory_limit: "2g".to_string(),
            cpu_limit: 2.0,
            timeout: 300.0,
            terminal_acknowledged: None,
            os: None,
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
    fn catalog_ids_are_unique_and_default_is_listed() {
        let ids: std::collections::HashSet<_> = SANDBOX_OS_CATALOG.iter().map(|os| os.id).collect();
        assert_eq!(ids.len(), SANDBOX_OS_CATALOG.len());
        assert_eq!(default_sandbox_os().image, DEFAULT_SANDBOX_IMAGE);
    }

    #[test]
    fn config_os_falls_back_to_default() {
        let stored: DockerSandboxConfig =
            serde_json::from_str(r#"{"memoryLimit":"2g","cpuLimit":2,"timeout":300}"#).unwrap();
        assert_eq!(stored.os, None);
        assert_eq!(stored.sandbox_os().id, DEFAULT_SANDBOX_OS_ID);

        let unknown = DockerSandboxConfig {
            os: Some("plan9".into()),
            ..Default::default()
        };
        assert_eq!(unknown.sandbox_os().id, DEFAULT_SANDBOX_OS_ID);

        let chosen = DockerSandboxConfig {
            os: Some("amazonlinux2023".into()),
            ..Default::default()
        };
        assert_eq!(
            chosen.sandbox_os().image,
            "public.ecr.aws/amazonlinux/amazonlinux:2023"
        );
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
