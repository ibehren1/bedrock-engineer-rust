//! Port of `src/preload/tools/handlers/docker/DockerSandboxTool.ts`, plus the
//! [`CommandSandbox`] implementation `executeCommand` uses to run inside a chat's sandbox.
//!
//! Both sit on [`docker::SandboxManager`]: the tool through [`SandboxOps`] (the
//! `docker-sandbox-{create,status,start,stop,remove,logs}` handlers), `executeCommand` through
//! `docker-sandbox-{exec,has-pid,send-input}`.

use crate::base::Tool;
use crate::command::{CommandSandbox, SandboxExecOptions};
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use docker::{CreateSandboxOptions, SandboxManager, SandboxRemoveOptions};
use serde_json::{json, Map, Value};
use std::sync::Arc;

const NAME: &str = "dockerSandbox";
const DESCRIPTION: &str = "Manage this chat's isolated Docker sandbox — a long-lived container built on ubuntu:26.04 where you can install anything without touching the user's machine.

The sandbox is created automatically the first time you run executeCommand, so you usually do not need the \"create\" operation. Use \"create\" only to define a multi-service stack, publish ports, or rebuild from scratch.

Key facts:
- The user's project directory ({{projectPath}}) is mounted read-write at /workspace, so files move freely in and out.
- The image is bare Ubuntu: run \"apt-get update\" before installing anything. DEBIAN_FRONTEND=noninteractive is already set.
- The container has network access, so apt/pip/npm work.
- Nothing is preinstalled — not even curl or git.
- Data written to /data persists on the host under the sandbox folder.
- To make a port reachable from the user's browser, declare it with \"create\" before starting the server.

Operations:
- create: create or rebuild the sandbox. Optionally pass \"services\" (structured) or \"composeYaml\" (raw compose). Named volumes are converted to mapped folders; host mounts outside the project directory, \"privileged\", \"cap_add\", and host networking are rejected.
- status: report whether the sandbox exists, whether it is running, and its services and published ports.
- start / stop: bring the sandbox up or halt it without discarding installed packages.
- remove: tear the sandbox down. Pass deleteData: true to also delete its data folder.
- logs: read recent output from a service, which is how you inspect a process started with executeCommand's detach option.";

const OPERATIONS: [&str; 6] = ["create", "status", "start", "stop", "remove", "logs"];

type OpResult<T> = std::result::Result<T, String>;

/// The `docker-sandbox-*` handlers the tool calls. Results are the handlers' JSON.
#[async_trait]
pub trait SandboxOps: Send + Sync {
    /// `{ metadata, warnings }`.
    async fn create(&self, session_id: &str, options: CreateSandboxOptions) -> OpResult<Value>;
    async fn status(&self, session_id: &str) -> OpResult<Value>;
    async fn start(&self, session_id: &str) -> OpResult<Value>;
    async fn stop(&self, session_id: &str) -> OpResult<Value>;
    async fn remove(&self, session_id: &str, delete_data: bool) -> OpResult<Value>;
    async fn logs(
        &self,
        session_id: &str,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> OpResult<Value>;
}

fn to_json<T: serde::Serialize>(r: docker::Result<T>) -> OpResult<Value> {
    r.map_err(|e| e.to_string())
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

#[async_trait]
impl SandboxOps for SandboxManager {
    async fn create(&self, session_id: &str, options: CreateSandboxOptions) -> OpResult<Value> {
        to_json(self.create_sandbox(session_id, options).await)
    }
    async fn status(&self, session_id: &str) -> OpResult<Value> {
        to_json(Ok(self.get_status(session_id).await))
    }
    async fn start(&self, session_id: &str) -> OpResult<Value> {
        to_json(self.start_sandbox(session_id).await)
    }
    async fn stop(&self, session_id: &str) -> OpResult<Value> {
        to_json(self.stop_sandbox(session_id).await)
    }
    async fn remove(&self, session_id: &str, delete_data: bool) -> OpResult<Value> {
        let options = SandboxRemoveOptions {
            delete_data: Some(delete_data),
        };
        to_json(self.remove_sandbox(session_id, options).await)
    }
    async fn logs(
        &self,
        session_id: &str,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> OpResult<Value> {
        to_json(self.get_logs(session_id, service, tail).await)
    }
}

fn pid_u32(pid: f64) -> Option<u32> {
    (pid.fract() == 0.0 && pid >= 0.0 && pid <= u32::MAX as f64).then_some(pid as u32)
}

/// `executeCommand`'s sandbox routing over the manager.
#[async_trait]
impl CommandSandbox for SandboxManager {
    async fn has_pid(&self, pid: f64) -> bool {
        pid_u32(pid).is_some_and(|p| self.is_tracked_sandbox_pid(p))
    }

    async fn send_input(&self, pid: f64, stdin: &str) -> std::result::Result<Value, String> {
        let pid =
            pid_u32(pid).ok_or_else(|| format!("No running process found with PID: {pid}"))?;
        to_json(SandboxManager::send_input(self, pid, stdin).await)
    }

    async fn exec(
        &self,
        session_id: &str,
        command: &str,
        options: SandboxExecOptions,
    ) -> std::result::Result<Value, String> {
        let options = docker::SandboxExecOptions {
            service: options.service,
            cwd: options.cwd,
            detach: options.detach,
            timeout: None,
        };
        to_json(self.exec_command(session_id, command, options).await)
    }
}

/// `resolveSessionId(input, context)`: the caller's session. The TS fell back to the input's
/// `_sessionId`, which the model could set to reach another chat's sandbox; the agent engine
/// now passes its session through [`ToolContext::session_id`] too.
pub fn resolve_session_id(ctx: &ToolContext) -> Option<String> {
    ctx.session_id.clone().filter(|s| !s.is_empty())
}

pub struct DockerSandboxTool {
    ops: Arc<dyn SandboxOps>,
}

impl DockerSandboxTool {
    pub fn new(ops: Arc<dyn SandboxOps>) -> Self {
        Self { ops }
    }

    async fn run(&self, op: &str, session_id: &str, input: &Value) -> OpResult<Value> {
        match op {
            "create" => {
                let services = match input.get("services").filter(|v| !v.is_null()) {
                    None => None,
                    Some(v) => Some(
                        serde_json::from_value(v.clone())
                            .map_err(|e| format!("Invalid services: {e}"))?,
                    ),
                };
                let options = CreateSandboxOptions {
                    services,
                    compose_yaml: input
                        .get("composeYaml")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    env: input.get("env").and_then(Value::as_object).cloned(),
                    recreate: input.get("recreate").and_then(Value::as_bool),
                };
                let created = self.ops.create(session_id, options).await?;
                let metadata = &created["metadata"];
                tracing::info!(session_id, "Sandbox created via tool");
                let mut out = Map::new();
                out.insert("created".into(), json!(true));
                for key in ["projectName", "directory"] {
                    out.insert(
                        key.into(),
                        metadata.get(key).cloned().unwrap_or(Value::Null),
                    );
                }
                out.insert("workspaceMount".into(), json!("/workspace"));
                for key in ["services", "composeless"] {
                    out.insert(
                        key.into(),
                        metadata.get(key).cloned().unwrap_or(Value::Null),
                    );
                }
                out.insert(
                    "warnings".into(),
                    created.get("warnings").cloned().unwrap_or(Value::Null),
                );
                out.insert(
                    "note".into(),
                    json!("The image is bare Ubuntu. Run \"apt-get update\" before installing packages."),
                );
                Ok(Value::Object(out))
            }
            "status" => self.ops.status(session_id).await,
            "start" => self.ops.start(session_id).await,
            "stop" => self.ops.stop(session_id).await,
            "remove" => {
                let delete = input.get("deleteData") == Some(&Value::Bool(true));
                self.ops.remove(session_id, delete).await
            }
            "logs" => {
                let tail = input
                    .get("tail")
                    .and_then(Value::as_f64)
                    .map(|t| t.min(u32::MAX as f64) as u32);
                let service = input.get("service").and_then(Value::as_str);
                self.ops.logs(session_id, service, tail).await
            }
            other => Err(format!("Unsupported operation: {other}")),
        }
    }
}

fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[async_trait]
impl Tool for DockerSandboxTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Docker
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": OPERATIONS,
                        "description": "The sandbox operation to perform"
                    },
                    "services": {
                        "type": "array",
                        "description": "Structured service definitions, used with create. Omit for a single bare ubuntu:26.04 service.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Service name (lowercase letters, digits, dashes, underscores)"
                                },
                                "image": {
                                    "type": "string",
                                    "description": "Container image. Defaults to ubuntu:26.04."
                                },
                                "command": {
                                    "type": "string",
                                    "description": "Container command. Defaults to \"sleep infinity\" so the container stays available for exec."
                                },
                                "ports": {
                                    "type": "array",
                                    "description": "Ports to publish on the host so the user can reach the service.",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "host": { "type": "number", "description": "Port on the host" },
                                            "container": { "type": "number", "description": "Port inside the container" }
                                        },
                                        "required": ["host", "container"]
                                    }
                                },
                                "environment": {
                                    "type": "object",
                                    "description": "Environment variables for the service"
                                },
                                "dataVolumes": {
                                    "type": "array",
                                    "description": "Extra persistent folders. Each maps the sandbox data directory to a container path.",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "name": {
                                                "type": "string",
                                                "description": "Folder name under the sandbox data dir"
                                            },
                                            "containerPath": { "type": "string", "description": "Mount point in the container" }
                                        },
                                        "required": ["name", "containerPath"]
                                    }
                                }
                            },
                            "required": ["name"]
                        }
                    },
                    "composeYaml": {
                        "type": "string",
                        "description": "Raw docker-compose YAML, used with create for stacks the structured form cannot express. Validated and rewritten before use."
                    },
                    "env": {
                        "type": "object",
                        "description": "Extra environment variables set in every sandbox service, used with create. Names must use letters, digits, and underscores."
                    },
                    "recreate": {
                        "type": "boolean",
                        "description": "With create, tear down the existing sandbox first. Installed packages are lost."
                    },
                    "deleteData": {
                        "type": "boolean",
                        "description": "With remove, also delete the sandbox data folder on the host. Defaults to false."
                    },
                    "service": {
                        "type": "string",
                        "description": "With logs, which service to read. Defaults to the first service."
                    },
                    "tail": {
                        "type": "number",
                        "description": "With logs, how many trailing lines to return. Defaults to 200."
                    }
                },
                "required": ["operation"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let list = OPERATIONS.join(", ");
        let op = input.get("operation");
        if !truthy(op) {
            errors.push(format!("Operation is required. One of: {list}"));
        } else if !op
            .and_then(Value::as_str)
            .is_some_and(|o| OPERATIONS.contains(&o))
        {
            errors.push(format!(
                "Unknown operation \"{}\". One of: {list}",
                js_string(op.unwrap_or(&Value::Null))
            ));
        }
        let op = op.and_then(Value::as_str);
        if op == Some("create") {
            if input.get("composeYaml").is_some_and(|v| !v.is_string()) {
                errors.push("composeYaml must be a string".to_string());
            }
            if input.get("services").is_some_and(|v| !v.is_array()) {
                errors.push("services must be an array".to_string());
            }
            if truthy(input.get("composeYaml")) && truthy(input.get("services")) {
                errors.push("Pass either services or composeYaml, not both".to_string());
            }
        }
        if op == Some("logs") {
            if let Some(tail) = input.get("tail") {
                if !tail.as_f64().is_some_and(|t| t > 0.0) {
                    errors.push("tail must be a positive number".to_string());
                }
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let Some(session_id) = resolve_session_id(ctx) else {
            return Err(ToolError::execution(
                "The Docker sandbox is scoped to a chat session, and no session is available in this context. Voice chat does not support sandboxes — run commands with target: \"host\" instead.",
                NAME,
                None,
                None,
            ));
        };
        let op = input
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match self.run(&op, &session_id, &input).await {
            Ok(payload) => Ok(ToolOutput::Json(json!({
                "success": true,
                "name": NAME,
                "message": "Docker sandbox operation completed",
                "result": payload
            }))),
            Err(message) => {
                tracing::error!(session_id = %session_id, operation = %op, error = %message, "Sandbox operation failed");
                let mut extra = Map::new();
                extra.insert("operation".into(), json!(op));
                Err(ToolError::execution(
                    message,
                    NAME,
                    Some(json!({})),
                    Some(extra),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeOps {
        calls: Mutex<Vec<(String, String, Value)>>,
        fail: Mutex<Option<String>>,
    }

    impl FakeOps {
        fn record(&self, op: &str, session: &str, v: Value) -> OpResult<Value> {
            self.calls
                .lock()
                .unwrap()
                .push((op.into(), session.into(), v.clone()));
            match self.fail.lock().unwrap().clone() {
                Some(m) => Err(m),
                None => Ok(v),
            }
        }
    }

    #[async_trait]
    impl SandboxOps for FakeOps {
        async fn create(&self, s: &str, o: CreateSandboxOptions) -> OpResult<Value> {
            self.record("create", s, serde_json::to_value(o).unwrap())?;
            Ok(json!({
                "metadata": {"sessionId": s, "projectName": "bedrock-sandbox-x", "directory": "/p/.sandbox/x",
                             "composeFile": "/p/.sandbox/x/compose.yaml", "projectPath": "/p",
                             "services": [{"name": "main", "image": "ubuntu:26.04", "ports": []}],
                             "composeless": false, "createdAt": "t", "updatedAt": "t"},
                "warnings": ["w1"]
            }))
        }
        async fn status(&self, s: &str) -> OpResult<Value> {
            self.record(
                "status",
                s,
                json!({"exists": false, "state": "missing", "containers": []}),
            )
        }
        async fn start(&self, s: &str) -> OpResult<Value> {
            self.record("start", s, json!({"exists": true}))
        }
        async fn stop(&self, s: &str) -> OpResult<Value> {
            self.record("stop", s, json!({"exists": true}))
        }
        async fn remove(&self, s: &str, d: bool) -> OpResult<Value> {
            self.record("remove", s, json!({"removed": true, "dataDeleted": d}))
        }
        async fn logs(&self, s: &str, svc: Option<&str>, tail: Option<u32>) -> OpResult<Value> {
            self.record("logs", s, json!({"service": svc, "tail": tail}))
        }
    }

    fn ctx(session: Option<&str>) -> ToolContext {
        ToolContext {
            session_id: session.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn validation_messages() {
        let t = DockerSandboxTool::new(Arc::new(FakeOps::default()));
        assert_eq!(
            t.validate_input(&json!({})),
            vec!["Operation is required. One of: create, status, start, stop, remove, logs"]
        );
        assert_eq!(
            t.validate_input(&json!({"operation": "explode"})),
            vec![
                "Unknown operation \"explode\". One of: create, status, start, stop, remove, logs"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"operation": "create", "composeYaml": 1, "services": {}})),
            vec![
                "composeYaml must be a string",
                "services must be an array",
                "Pass either services or composeYaml, not both"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"operation": "logs", "tail": 0})),
            vec!["tail must be a positive number"]
        );
        assert!(t
            .validate_input(&json!({"operation": "logs", "tail": 5}))
            .is_empty());
    }

    #[tokio::test]
    async fn requires_a_session() {
        let t = DockerSandboxTool::new(Arc::new(FakeOps::default()));
        let err = run_tool(&t, json!({"type": NAME, "operation": "status"}), &ctx(None))
            .await
            .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(v["type"], "EXECUTION");
        assert_eq!(v["toolName"], "dockerSandbox");
        assert!(v["error"]
            .as_str()
            .unwrap()
            .starts_with("The Docker sandbox is scoped to a chat session"));
        assert!(v.get("cause").is_none());
    }

    #[tokio::test]
    async fn operations_route_to_the_manager() {
        let ops = Arc::new(FakeOps::default());
        let t = DockerSandboxTool::new(ops.clone());
        let v = run_tool(
            &t,
            json!({"type": NAME, "operation": "create", "services": [{"name": "web", "ports": [{"host": 8080, "container": 80}]}], "recreate": true}),
            &ctx(Some("session_1")),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v,
            json!({
                "success": true, "name": "dockerSandbox", "message": "Docker sandbox operation completed",
                "result": {
                    "created": true, "projectName": "bedrock-sandbox-x", "directory": "/p/.sandbox/x",
                    "workspaceMount": "/workspace",
                    "services": [{"name": "main", "image": "ubuntu:26.04", "ports": []}],
                    "composeless": false, "warnings": ["w1"],
                    "note": "The image is bare Ubuntu. Run \"apt-get update\" before installing packages."
                }
            })
        );
        // The background-agent engine passes its session through the context.
        let v = run_tool(
            &t,
            json!({"type": NAME, "operation": "remove", "deleteData": true}),
            &ctx(Some("bg")),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(v["result"], json!({"removed": true, "dataDeleted": true}));
        run_tool(
            &t,
            json!({"type": NAME, "operation": "logs", "service": "web", "tail": 50}),
            &ctx(Some("session_1")),
        )
        .await
        .unwrap();
        let calls = ops.calls.lock().unwrap().clone();
        assert_eq!(calls[0].0, "create");
        assert_eq!(
            calls[0].2,
            json!({"services": [{"name": "web", "ports": [{"host": 8080, "container": 80}]}], "recreate": true})
        );
        assert_eq!(calls[1].1, "bg");
        assert_eq!(calls[2].2, json!({"service": "web", "tail": 50}));
    }

    #[tokio::test]
    async fn model_supplied_session_id_is_ignored() {
        let ops = Arc::new(FakeOps::default());
        let mut registry = crate::ToolRegistry::new();
        registry.register(Arc::new(DockerSandboxTool::new(ops.clone())));
        // No session in the context: the input's `_sessionId` must not stand in for one.
        let err = registry
            .execute(
                json!({"type": NAME, "operation": "remove", "deleteData": true, "_sessionId": "other"}),
                &ctx(None),
            )
            .await
            .unwrap_err();
        assert!(
            err.message.contains("scoped to a chat session"),
            "{}",
            err.message
        );
        // With a session, the input's `_sessionId` cannot redirect to another chat's sandbox.
        registry
            .execute(
                json!({"type": NAME, "operation": "status", "_sessionId": "other"}),
                &ctx(Some("mine")),
            )
            .await
            .unwrap();
        let calls = ops.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "mine");
    }

    #[tokio::test]
    async fn failures_carry_the_operation() {
        let ops = Arc::new(FakeOps::default());
        *ops.fail.lock().unwrap() = Some("No sandbox exists for session s.".into());
        let t = DockerSandboxTool::new(ops);
        let err = run_tool(
            &t,
            json!({"type": NAME, "operation": "start"}),
            &ctx(Some("s")),
        )
        .await
        .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(
            v,
            json!({"success": false, "error": "No sandbox exists for session s.", "type": "EXECUTION",
                   "toolName": "dockerSandbox", "cause": {}, "operation": "start"})
        );
    }

    #[tokio::test]
    async fn manager_as_command_sandbox_without_docker() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().to_string_lossy().into_owned();
        let settings: Arc<dyn docker::Settings> =
            Arc::new(move |k: &str| (k == "projectPath").then(|| json!(project.clone())));
        let manager = SandboxManager::new(settings, Arc::new(docker::NoopSink));
        assert!(!CommandSandbox::has_pid(&manager, 1.5).await);
        assert!(!CommandSandbox::has_pid(&manager, 999_999.0).await);
        assert_eq!(
            CommandSandbox::send_input(&manager, -1.0, "x")
                .await
                .unwrap_err(),
            "No running process found with PID: -1"
        );
        // No sandbox on disk: status is "missing" and stop is a no-op.
        let status = SandboxOps::status(&manager, "session_x").await.unwrap();
        assert_eq!(status["exists"], false);
        assert_eq!(status["state"], "missing");
        let removed = SandboxOps::remove(&manager, "session_x", false)
            .await
            .unwrap();
        assert_eq!(removed, json!({"removed": false, "dataDeleted": false}));
    }
}
