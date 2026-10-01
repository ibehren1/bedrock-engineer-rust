//! Port of `src/preload/tools/handlers/command/ExecuteCommandTool.ts`.

use super::service::{CommandConfig, CommandExecutionResult, CommandService};
use super::{CommandSandbox, SandboxExecOptions};
use crate::base::{default_handle_error, Tool};
use crate::context::{CommandPatternConfig, ToolContext};
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

const NAME: &str = "executeCommand";
const DESCRIPTION: &str = "Execute a command or send input to a running process. First execute the command to get a PID, then use that PID to send input if needed. Usage: 1) First call with command and cwd to start process, 2) If input is required, call again with pid and stdin.\n\nWhen the dockerSandbox tool is enabled, commands run inside this chat's isolated Docker container by default and any command is permitted there — the sandbox is created automatically on first use. The project directory ({{projectPath}}) is mounted at /workspace, so use /workspace paths for cwd. The sandbox is bare Ubuntu, so run \"apt-get update\" before installing packages.\n\nSet target: \"host\" to run on the user's own machine instead. Host commands require the user to approve them, and only commands from this allowed list may be used: {{allowedCommands}}. Prefer the sandbox unless the task genuinely needs the host (for example git operations on the real repository).\n\nSet detach: true to start a long-running process in the background and return immediately; read its output later with the dockerSandbox logs operation.";

/// `ExecuteCommandTool`. Holds the host [`CommandService`], recreated whenever the
/// resolved config (shell + agent allowlist) changes, like the TS module-level state.
#[derive(Default)]
pub struct ExecuteCommandTool {
    state: Mutex<Option<(CommandConfig, Arc<CommandService>)>>,
}

impl ExecuteCommandTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getCommandService`.
    fn command_service(&self, config: CommandConfig) -> Arc<CommandService> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some((c, s)) if *c == config => s.clone(),
            _ => {
                let s = Arc::new(CommandService::new(config.clone()));
                *guard = Some((config, s.clone()));
                s
            }
        }
    }

    /// The current host service, if one has been created.
    pub fn current_service(&self) -> Option<Arc<CommandService>> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|(_, s)| s.clone())
    }

    /// `getCommandConfig`.
    fn command_config(ctx: &ToolContext) -> CommandConfig {
        let shell = ctx
            .settings
            .shell
            .clone()
            .unwrap_or_else(|| "/bin/bash".to_string());
        let allowed_commands: Vec<CommandPatternConfig> = match ctx.agent_id() {
            Some(id) => ctx
                .find_agent(&id)
                .map(|a| a.allowed_commands)
                .unwrap_or_default(),
            None => {
                tracing::warn!("No agent ID found for command configuration");
                Vec::new()
            }
        };
        CommandConfig {
            allowed_commands,
            shell,
        }
    }

    /// `resolveSandboxSession`.
    fn sandbox_session(input: &Value, ctx: &ToolContext) -> Option<String> {
        if input.get("target").and_then(Value::as_str) == Some("host") {
            return None;
        }
        let agent = ctx.find_agent(&ctx.agent_id()?)?;
        if !agent.tools.iter().any(|t| t == "dockerSandbox") {
            return None;
        }
        ctx.session_id.clone().filter(|s| !s.is_empty())
    }
}

/// `{ success, name, message, result: { target, ...fields }, ...raw }`.
fn build_result(message: String, target: &str, raw: &Value, inner_keys: &[&str]) -> Value {
    let mut inner = Map::new();
    inner.insert("target".into(), json!(target));
    for k in inner_keys {
        if let Some(v) = raw.get(*k).filter(|v| !v.is_null()) {
            inner.insert((*k).into(), v.clone());
        }
    }
    let mut out = Map::new();
    out.insert("success".into(), json!(true));
    out.insert("name".into(), json!(NAME));
    out.insert("message".into(), json!(message));
    out.insert("result".into(), Value::Object(inner));
    if let Some(o) = raw.as_object() {
        for (k, v) in o {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out)
}

const HOST_KEYS: &[&str] = &[
    "stdout",
    "stderr",
    "exitCode",
    "processInfo",
    "requiresInput",
    "prompt",
];
const SANDBOX_KEYS: &[&str] = &[
    "stdout",
    "stderr",
    "exitCode",
    "processInfo",
    "requiresInput",
    "prompt",
    "detached",
];

fn has_key(input: &Value, k: &str) -> bool {
    input.get(k).is_some()
}

async fn sandbox_exec(
    sandbox: &dyn CommandSandbox,
    session_id: &str,
    input: &Value,
) -> std::result::Result<Value, String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let options = SandboxExecOptions {
        service: input
            .get("service")
            .and_then(Value::as_str)
            .map(str::to_string),
        cwd: input
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|c| c.starts_with('/'))
            .map(str::to_string),
        detach: input.get("detach").and_then(Value::as_bool),
    };
    let r = sandbox.exec(session_id, command, options).await?;
    Ok(build_result(
        format!("Command executed in sandbox: {command}"),
        "sandbox",
        &r,
        SANDBOX_KEYS,
    ))
}

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Command
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute (used when starting a new process)" },
                    "cwd": { "type": "string", "description": "The working directory for the command execution (used with command)" },
                    "pid": { "type": "number", "description": "Process ID to send input to (used when sending input to existing process)" },
                    "stdin": { "type": "string", "description": "Standard input to send to the process (used with pid)" },
                    "target": {
                        "type": "string",
                        "enum": ["sandbox", "host"],
                        "description": "Where to run the command. Defaults to the chat's Docker sandbox when that tool is enabled. Use \"host\" only when the task requires the user's own machine; the user must approve each host command."
                    },
                    "service": {
                        "type": "string",
                        "description": "Sandbox service to run the command in. Defaults to the sandbox's first service. Ignored on the host."
                    },
                    "detach": {
                        "type": "boolean",
                        "description": "Start the command in the background and return immediately. Use for dev servers and other long-running processes, then read output with the dockerSandbox logs operation. Sandbox only."
                    }
                }
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        if has_key(input, "pid") && has_key(input, "stdin") {
            if !input["pid"].is_number() {
                errors.push("PID must be a number".to_string());
            }
            if !input["stdin"].is_string() {
                errors.push("Stdin must be a string".to_string());
            }
        } else if has_key(input, "command") && has_key(input, "cwd") {
            if !truthy(input.get("command")) {
                errors.push("Command is required".to_string());
            }
            if !input["command"].is_string() {
                errors.push("Command must be a string".to_string());
            }
            if !truthy(input.get("cwd")) {
                errors.push("Working directory (cwd) is required".to_string());
            }
            if !input["cwd"].is_string() {
                errors.push("Working directory must be a string".to_string());
            }
        } else {
            errors.push(
                "Invalid input format: requires either (command, cwd) or (pid, stdin)".to_string(),
            );
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let is_stdin = has_key(&input, "pid") && has_key(&input, "stdin");
        let pid = input.get("pid").and_then(Value::as_f64).unwrap_or(f64::NAN);
        let stdin = input
            .get("stdin")
            .and_then(Value::as_str)
            .unwrap_or_default();

        // stdin follow-ups go to whichever executor owns the PID.
        if is_stdin {
            if let Some(sandbox) = &ctx.sandbox {
                if sandbox.has_pid(pid).await {
                    let r = sandbox
                        .send_input(pid, stdin)
                        .await
                        .map_err(ToolError::plain)?;
                    return Ok(ToolOutput::Json(build_result(
                        format!("Sent input to sandbox process {pid}"),
                        "sandbox",
                        &r,
                        HOST_KEYS,
                    )));
                }
            }
        } else if let Some(session_id) = Self::sandbox_session(&input, ctx) {
            let extra = || {
                let mut m = Map::new();
                m.insert("input".into(), input.clone());
                Some(m)
            };
            let Some(sandbox) = &ctx.sandbox else {
                return Err(ToolError::execution(
                    "The Docker sandbox is not available in this build",
                    NAME,
                    None,
                    extra(),
                ));
            };
            return sandbox_exec(sandbox.as_ref(), &session_id, &input)
                .await
                .map(ToolOutput::Json)
                .map_err(|m| ToolError::execution(m, NAME, Some(json!({})), extra()));
        }

        let config = Self::command_config(ctx);
        let service = self.command_service(config);
        let result: std::result::Result<CommandExecutionResult, String> = if is_stdin {
            if pid.fract() == 0.0 && pid >= 0.0 && pid <= u32::MAX as f64 {
                service.send_input(pid as u32, stdin).await
            } else {
                Err(format!("No running process found with PID: {pid}"))
            }
        } else {
            let command = input
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let cwd = input.get("cwd").and_then(Value::as_str).unwrap_or_default();
            service.execute_command(command, cwd).await
        };

        match result {
            Ok(r) => {
                let raw = serde_json::to_value(&r).unwrap_or(Value::Null);
                Ok(ToolOutput::Json(build_result(
                    format!("Command executed: {input}"),
                    "host",
                    &raw,
                    HOST_KEYS,
                )))
            }
            Err(message) => {
                if message.contains("not allowed") {
                    let op = if is_stdin {
                        "stdin".to_string()
                    } else {
                        input
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string()
                    };
                    return Err(ToolError::permission_denied(message, NAME, Some(&op)));
                }
                let mut m = Map::new();
                m.insert("input".into(), input.clone());
                Err(ToolError::execution(
                    message,
                    NAME,
                    Some(json!({})),
                    Some(m),
                ))
            }
        }
    }

    /// Rethrows `{ success: false, error: <BaseTool error message> }`.
    fn handle_error(&self, error: ToolError) -> ToolError {
        let base = default_handle_error(error, NAME);
        ToolError::plain(json!({ "success": false, "error": base.message }).to_string())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn ctx(allowed: &[&str], tools: &[&str]) -> ToolContext {
        let store = json!({
            "shell": "/bin/sh",
            "selectedAgentId": "dev",
            "customAgents": [{
                "id": "dev",
                "allowedCommands": allowed.iter().map(|p| json!({"pattern": p})).collect::<Vec<_>>(),
                "tools": tools
            }]
        });
        ToolContext::from_store(&store, Some("session_1".into()))
    }

    #[tokio::test]
    async fn host_result_shape() {
        let tool = ExecuteCommandTool::new();
        let input = json!({"type": "executeCommand", "command": "echo hi", "cwd": "/tmp"});
        let out = run_tool(&tool, input.clone(), &ctx(&["echo *"], &[]))
            .await
            .unwrap();
        let v = out.as_json().unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            vec![
                "success",
                "name",
                "message",
                "result",
                "stdout",
                "stderr",
                "exitCode",
                "processInfo"
            ]
        );
        assert_eq!(v["message"], format!("Command executed: {input}"));
        assert_eq!(v["result"]["target"], "host");
        assert_eq!(v["exitCode"], 0);
        assert!(v["stdout"].as_str().unwrap().contains("hi"));
        assert_eq!(v["result"]["stdout"], v["stdout"]);
    }

    #[tokio::test]
    async fn disallowed_command_is_permission_denied_json() {
        let tool = ExecuteCommandTool::new();
        let input = json!({"type": "executeCommand", "command": "rm -rf /", "cwd": "/tmp"});
        let err = run_tool(&tool, input, &ctx(&["echo *"], &[]))
            .await
            .unwrap_err();
        assert_eq!(err.name, "Error");
        let outer: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(outer["success"], false);
        let inner: Value = serde_json::from_str(outer["error"].as_str().unwrap()).unwrap();
        assert_eq!(inner["error"], "Command not allowed: rm -rf /");
        assert_eq!(inner["type"], "PERMISSION_DENIED");
        assert_eq!(inner["operation"], "rm -rf /");
    }

    #[test]
    fn validation_messages() {
        let tool = ExecuteCommandTool::new();
        assert_eq!(
            tool.validate_input(&json!({"type": "executeCommand"})),
            vec!["Invalid input format: requires either (command, cwd) or (pid, stdin)"]
        );
        assert_eq!(
            tool.validate_input(&json!({"pid": "1", "stdin": 2})),
            vec!["PID must be a number", "Stdin must be a string"]
        );
        assert_eq!(
            tool.validate_input(&json!({"command": "", "cwd": 1})),
            vec!["Command is required", "Working directory must be a string"]
        );
    }

    struct FakeSandbox(AtomicBool);

    #[async_trait]
    impl CommandSandbox for FakeSandbox {
        async fn has_pid(&self, pid: f64) -> bool {
            pid == 42.0
        }
        async fn send_input(&self, _pid: f64, stdin: &str) -> std::result::Result<Value, String> {
            Ok(json!({"stdout": format!("echo {stdin}"), "stderr": "", "exitCode": 0}))
        }
        async fn exec(
            &self,
            session_id: &str,
            command: &str,
            options: SandboxExecOptions,
        ) -> std::result::Result<Value, String> {
            self.0.store(true, Ordering::SeqCst);
            assert_eq!(session_id, "session_1");
            assert_eq!(options.cwd.as_deref(), Some("/workspace"));
            Ok(json!({"stdout": command, "stderr": "", "exitCode": 0, "detached": false}))
        }
    }

    #[tokio::test]
    async fn sandbox_routing() {
        let tool = ExecuteCommandTool::new();
        let sandbox = Arc::new(FakeSandbox(AtomicBool::new(false)));
        let mut c = ctx(&[], &["dockerSandbox"]);
        c.sandbox = Some(sandbox.clone());

        // Any command runs in the sandbox, regardless of the host allowlist.
        let out = run_tool(
            &tool,
            json!({"type": "executeCommand", "command": "apt-get update", "cwd": "/workspace"}),
            &c,
        )
        .await
        .unwrap();
        assert!(sandbox.0.load(Ordering::SeqCst));
        let v = out.as_json().unwrap();
        assert_eq!(v["message"], "Command executed in sandbox: apt-get update");
        assert_eq!(v["result"]["target"], "sandbox");
        assert_eq!(v["result"]["detached"], false);

        // stdin to a sandbox-owned PID.
        let out = run_tool(
            &tool,
            json!({"type": "executeCommand", "pid": 42, "stdin": "y"}),
            &c,
        )
        .await
        .unwrap();
        assert_eq!(
            out.as_json().unwrap()["message"],
            "Sent input to sandbox process 42"
        );

        // target: host bypasses the sandbox and hits the (empty) allowlist.
        let err = run_tool(
            &tool,
            json!({"type": "executeCommand", "command": "ls", "cwd": "/tmp", "target": "host"}),
            &c,
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("Command not allowed: ls"));
    }

    #[tokio::test]
    async fn sandbox_enabled_but_unavailable_does_not_fall_back_to_host() {
        let tool = ExecuteCommandTool::new();
        let err = run_tool(
            &tool,
            json!({"type": "executeCommand", "command": "echo hi", "cwd": "/tmp"}),
            &ctx(&["echo *"], &["dockerSandbox"]),
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("not available"), "{}", err.message);
    }

    fn two_agent_ctx() -> ToolContext {
        let store = json!({
            "shell": "/bin/sh",
            "selectedAgentId": "dev",
            "customAgents": [
                {"id": "dev", "allowedCommands": [{"pattern": "echo *"}], "tools": []},
                {"id": "admin", "allowedCommands": [{"pattern": "id"}], "tools": []}
            ]
        });
        ToolContext::from_store(&store, Some("session_1".into()))
    }

    #[tokio::test]
    async fn model_supplied_agent_id_cannot_pick_another_agents_allowlist() {
        let mut registry = crate::ToolRegistry::new();
        registry.register(Arc::new(ExecuteCommandTool::new()));
        let err = registry
            .execute(
                json!({"type": "executeCommand", "command": "id", "cwd": "/tmp", "_agentId": "admin"}),
                &two_agent_ctx(),
            )
            .await
            .unwrap_err();
        assert!(
            err.message.contains("Command not allowed: id"),
            "{}",
            err.message
        );
    }

    #[tokio::test]
    async fn caller_agent_id_selects_the_allowlist() {
        let tool = ExecuteCommandTool::new();
        let mut c = two_agent_ctx();
        c.caller.agent_id = Some("admin".into());
        let out = run_tool(
            &tool,
            json!({"type": "executeCommand", "command": "id", "cwd": "/tmp"}),
            &c,
        )
        .await
        .unwrap();
        assert_eq!(out.as_json().unwrap()["exitCode"], 0);
    }
}
