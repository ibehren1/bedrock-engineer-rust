//! Commands run inside a sandbox container. Port of `src/main/api/docker/sandboxExec.ts`.
//!
//! Like the host shell path, an exec resolves early when its output looks like an
//! interactive prompt (returning the client PID for a stdin follow-up) or like a dev server
//! that has come up, and the underlying `docker exec` keeps running in both cases.
//!
//! `-t` is deliberately omitted: a TTY injects ANSI escapes the prompt and server-ready
//! matchers would trip on. The user-facing terminal is the separate TTY path in
//! [`crate::terminal`], and nothing it does belongs in the registry here.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::ChildStdin;
use tokio::sync::{broadcast, Notify};

use crate::activity::{
    ActivityLog, ActivityOutcome, RecordExitInput, RecordSettledInput, RecordStartInput,
};
use crate::output_patterns::{detect_errors, detect_server_ready, detect_waiting_for_input};
use crate::runner::{
    compose_argv, configure, terminate, CommandRunner, ComposeContext, RunOptions,
};
use crate::types::{ProcessInfo, SandboxExecOptions, SandboxExecResult, WORKSPACE_MOUNT};
use crate::util::Utf8Accumulator;
use crate::{Error, Result};

const DEFAULT_EXEC_TIMEOUT_SECS: f64 = 300.0;
const DEFAULT_STDIN_TIMEOUT_SECS: f64 = 15.0;

/// A timeout in (possibly fractional) seconds, clamped to something `Duration` accepts.
fn seconds(value: f64) -> Duration {
    if value.is_finite() && value > 0.0 {
        Duration::from_secs_f64(value.min(u32::MAX as f64))
    } else {
        Duration::ZERO
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecTarget {
    pub session_id: String,
    pub service: String,
    /// Compose context, or `None` when running composeless via `docker exec`.
    pub compose: Option<ComposeContext>,
    /// Container name, required for the composeless path.
    pub container_name: Option<String>,
}

/// The argv for a single exec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecArgv {
    pub bin: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

/// Build the argv for a single exec (`-i` keeps stdin open for prompts; no `-t`).
pub fn build_exec_argv(
    target: &ExecTarget,
    command: &str,
    options: &SandboxExecOptions,
) -> Result<ExecArgv> {
    let workdir = options
        .cwd
        .clone()
        .unwrap_or_else(|| WORKSPACE_MOUNT.to_string());
    let shell_args = ["sh".to_string(), "-lc".to_string(), command.to_string()];
    let mut exec_args = vec![
        "exec".to_string(),
        "-i".to_string(),
        "--workdir".to_string(),
        workdir,
    ];
    if options.detach == Some(true) {
        exec_args.push("-d".to_string());
    }

    if let Some(compose) = &target.compose {
        let (bin, mut args) = compose_argv(compose.kind, &compose.compose_file, &compose.env_file);
        args.extend(exec_args);
        args.push(target.service.clone());
        args.extend(shell_args);
        return Ok(ExecArgv {
            bin,
            args,
            cwd: Some(compose.cwd.clone()),
        });
    }

    let container = target
        .container_name
        .clone()
        .ok_or_else(|| Error::msg("Composeless exec requires a container name"))?;
    let mut args = exec_args;
    args.push(container);
    args.extend(shell_args);
    Ok(ExecArgv {
        bin: "docker".to_string(),
        args,
        cwd: None,
    })
}

#[derive(Debug, Clone, Copy)]
enum ExecEvent {
    Stdout,
    Stderr,
    Closed(i32),
}

#[derive(Default)]
struct ExecOutput {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    is_running: bool,
}

struct RunningExec {
    session_id: String,
    command: String,
    activity_id: String,
    output: Mutex<ExecOutput>,
    events: broadcast::Sender<ExecEvent>,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    kill: Notify,
}

impl RunningExec {
    fn snapshot(&self) -> (String, String) {
        let output = self.output.lock().unwrap();
        (output.stdout.clone(), output.stderr.clone())
    }
}

/// Live `docker exec` client processes, keyed by the local client PID. A command that
/// stopped for input keeps its entry so a `{ pid, stdin }` follow-up can find it.
pub struct ExecRegistry {
    running: Mutex<HashMap<u32, Arc<RunningExec>>>,
    activity: Arc<ActivityLog>,
}

impl ExecRegistry {
    pub fn new(activity: Arc<ActivityLog>) -> Self {
        Self {
            running: Mutex::new(HashMap::new()),
            activity,
        }
    }

    /// Run a command inside a sandbox container.
    pub async fn exec_in_sandbox(
        self: &Arc<Self>,
        target: &ExecTarget,
        command: &str,
        options: &SandboxExecOptions,
    ) -> Result<SandboxExecResult> {
        let argv = build_exec_argv(target, command, options)?;
        self.spawn_tracked(target, command, options, argv).await
    }

    /// The exec state machine over an arbitrary argv (separated so tests can drive it with
    /// a plain shell instead of Docker).
    pub(crate) async fn spawn_tracked(
        self: &Arc<Self>,
        target: &ExecTarget,
        command: &str,
        options: &SandboxExecOptions,
        argv: ExecArgv,
    ) -> Result<SandboxExecResult> {
        let mut cmd = tokio::process::Command::new(&argv.bin);
        cmd.args(&argv.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &argv.cwd {
            cmd.current_dir(cwd);
        }
        configure(&mut cmd);

        let spawn_failed = || {
            Error::msg(format!(
                "Failed to start docker exec for session {}",
                target.session_id
            ))
        };
        let mut child = cmd.spawn().map_err(|_| spawn_failed())?;
        let pid = child.id().ok_or_else(spawn_failed)?;

        let started = Instant::now();
        let cwd = options
            .cwd
            .clone()
            .unwrap_or_else(|| WORKSPACE_MOUNT.to_string());
        let activity_id = self.activity.record_start(RecordStartInput {
            session_id: target.session_id.clone(),
            service: target.service.clone(),
            command: command.to_string(),
            source: None,
            cwd: Some(cwd),
            pid: Some(pid),
        });

        let (events, mut rx) = broadcast::channel(4096);
        let state = Arc::new(RunningExec {
            session_id: target.session_id.clone(),
            command: command.to_string(),
            activity_id: activity_id.clone(),
            output: Mutex::new(ExecOutput {
                is_running: true,
                ..Default::default()
            }),
            events,
            stdin: tokio::sync::Mutex::new(child.stdin.take()),
            kill: Notify::new(),
        });
        self.running.lock().unwrap().insert(pid, state.clone());

        if options.detach == Some(true) {
            // `exec -d` returns as soon as the command is started; nothing will be written.
            state.stdin.lock().await.take();
        }

        // Pump: read both streams, then wait for exit. Keeps running after this call
        // resolves early, so the activity row is upgraded with the real exit code.
        {
            let registry = self.clone();
            let state = state.clone();
            let mut stdout = child.stdout.take();
            let mut stderr = child.stderr.take();
            tokio::spawn(async move {
                let mut out_acc = Utf8Accumulator::default();
                let mut err_acc = Utf8Accumulator::default();
                let mut out_buf = vec![0u8; 16 * 1024];
                let mut err_buf = vec![0u8; 16 * 1024];
                let mut killed = false;
                while stdout.is_some() || stderr.is_some() {
                    tokio::select! {
                        read = async { stdout.as_mut().unwrap().read(&mut out_buf).await }, if stdout.is_some() => {
                            match read {
                                Ok(n) if n > 0 => {
                                    out_acc.push(&mut state.output.lock().unwrap().stdout, &out_buf[..n]);
                                    let _ = state.events.send(ExecEvent::Stdout);
                                }
                                _ => {
                                    out_acc.finish(&mut state.output.lock().unwrap().stdout);
                                    stdout = None;
                                }
                            }
                        }
                        read = async { stderr.as_mut().unwrap().read(&mut err_buf).await }, if stderr.is_some() => {
                            match read {
                                Ok(n) if n > 0 => {
                                    err_acc.push(&mut state.output.lock().unwrap().stderr, &err_buf[..n]);
                                    let _ = state.events.send(ExecEvent::Stderr);
                                }
                                _ => {
                                    err_acc.finish(&mut state.output.lock().unwrap().stderr);
                                    stderr = None;
                                }
                            }
                        }
                        _ = state.kill.notified(), if !killed => {
                            killed = true;
                            terminate(&mut child);
                        }
                    }
                }
                let status = loop {
                    tokio::select! {
                        status = child.wait() => break status,
                        _ = state.kill.notified(), if !killed => {
                            killed = true;
                            terminate(&mut child);
                        }
                    }
                };
                let code = status.ok().and_then(|s| s.code()).unwrap_or(1);

                let (stdout_bytes, stderr_bytes) = {
                    let mut output = state.output.lock().unwrap();
                    output.is_running = false;
                    output.exit_code = Some(code);
                    (output.stdout.len(), output.stderr.len())
                };
                registry.running.lock().unwrap().remove(&pid);

                // Fires whether or not the caller already resolved, so a row that settled as
                // detached or requires-input is upgraded with the real exit code.
                registry.activity.record_exit(RecordExitInput {
                    session_id: state.session_id.clone(),
                    id: state.activity_id.clone(),
                    exit_code: code,
                    stdout_bytes: Some(stdout_bytes),
                    stderr_bytes: Some(stderr_bytes),
                });
                let _ = state.events.send(ExecEvent::Closed(code));
            });
        }

        let timeout_secs = options.timeout.unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
        let deadline = tokio::time::sleep(seconds(timeout_secs));
        tokio::pin!(deadline);

        let settle = |outcome: ActivityOutcome, exit_code: Option<i32>| {
            self.activity.record_settled(RecordSettledInput {
                session_id: target.session_id.clone(),
                id: activity_id.clone(),
                outcome,
                exit_code,
                duration_ms: Some(started.elapsed().as_millis() as i64),
            });
        };

        let requires_input =
            |stdout: String, stderr: String, prompt: Option<String>| SandboxExecResult {
                stdout,
                stderr,
                exit_code: 0,
                process_info: Some(ProcessInfo {
                    pid,
                    command: command.to_string(),
                    detached: false,
                }),
                requires_input: Some(true),
                prompt,
                detached: None,
            };

        loop {
            let event = tokio::select! {
                _ = &mut deadline => {
                    settle(ActivityOutcome::Timeout, None);
                    let (stdout, stderr) = state.snapshot();
                    // Leave the process alone but stop waiting on it; a long build should
                    // not be killed just because the model's turn needs an answer.
                    return Ok(SandboxExecResult {
                        stdout,
                        stderr,
                        exit_code: 124,
                        process_info: Some(ProcessInfo { pid, command: command.to_string(), detached: true }),
                        prompt: Some(format!(
                            "Command still running after {timeout_secs}s. Use dockerSandbox logs, or send input with pid {pid}."
                        )),
                        ..Default::default()
                    });
                }
                event = rx.recv() => event,
            };

            let (check_stdout, check_stderr, closed) = match event {
                Ok(ExecEvent::Stdout) => (true, false, None),
                Ok(ExecEvent::Stderr) => (false, true, None),
                Ok(ExecEvent::Closed(code)) => (false, false, Some(code)),
                // Missed some events: evaluate everything against the accumulated output.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let exit = state.output.lock().unwrap().exit_code;
                    (true, true, exit)
                }
                Err(broadcast::error::RecvError::Closed) => {
                    let exit = state.output.lock().unwrap().exit_code.unwrap_or(1);
                    (false, false, Some(exit))
                }
            };

            let (stdout, stderr) = state.snapshot();

            if check_stdout {
                let waiting = detect_waiting_for_input(&stdout);
                if waiting.is_waiting {
                    settle(ActivityOutcome::RequiresInput, None);
                    return Ok(requires_input(stdout, stderr, waiting.prompt));
                }
                if detect_server_ready(&stdout) && !detect_errors(&stdout, &stderr) {
                    settle(ActivityOutcome::Detached, None);
                    return Ok(SandboxExecResult {
                        stdout,
                        stderr,
                        exit_code: 0,
                        process_info: Some(ProcessInfo {
                            pid,
                            command: command.to_string(),
                            detached: true,
                        }),
                        ..Default::default()
                    });
                }
            }

            if check_stderr {
                let waiting = detect_waiting_for_input(&stderr);
                if waiting.is_waiting {
                    settle(ActivityOutcome::RequiresInput, None);
                    return Ok(requires_input(stdout, stderr, waiting.prompt));
                }
            }

            if let Some(code) = closed {
                settle(ActivityOutcome::Completed, Some(code));
                return Ok(SandboxExecResult {
                    stdout,
                    stderr,
                    exit_code: code,
                    detached: options.detach,
                    ..Default::default()
                });
            }
        }
    }

    /// Write to the stdin of a still-running sandbox command. The PID is the local docker
    /// client's, which forwards stdin into the container.
    pub async fn send_input(
        &self,
        pid: u32,
        stdin: &str,
        timeout_seconds: Option<f64>,
    ) -> Result<SandboxExecResult> {
        let state = self.running.lock().unwrap().get(&pid).cloned();
        let not_found = || {
            Error::msg(format!(
                "No running sandbox command found with PID {pid}. The container may have been stopped or the command already finished — re-run the command instead of sending input."
            ))
        };
        let state = state.ok_or_else(not_found)?;
        let mut rx = state.events.subscribe();

        let (before_out, before_err, running) = {
            let output = state.output.lock().unwrap();
            (output.stdout.len(), output.stderr.len(), output.is_running)
        };
        if !running {
            return Err(not_found());
        }

        let fresh = |state: &RunningExec| {
            let output = state.output.lock().unwrap();
            (
                output.stdout[before_out..].to_string(),
                output.stderr[before_err..].to_string(),
            )
        };
        let process_info = || ProcessInfo {
            pid,
            command: state.command.clone(),
            detached: false,
        };

        let payload = if stdin.ends_with('\n') {
            stdin.to_string()
        } else {
            format!("{stdin}\n")
        };
        // Length only: a model answering a prompt may be typing a credential.
        self.activity
            .record_stdin(&state.session_id, &state.activity_id, payload.len());
        if let Some(pipe) = state.stdin.lock().await.as_mut() {
            let _ = pipe.write_all(payload.as_bytes()).await;
            let _ = pipe.flush().await;
        }

        let timeout = seconds(timeout_seconds.unwrap_or(DEFAULT_STDIN_TIMEOUT_SECS));
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);

        loop {
            let event = tokio::select! {
                _ = &mut deadline => {
                    let (stdout, stderr) = fresh(&state);
                    return Ok(SandboxExecResult {
                        stdout,
                        stderr,
                        exit_code: 0,
                        process_info: Some(process_info()),
                        ..Default::default()
                    });
                }
                event = rx.recv() => event,
            };
            let closed = match event {
                Ok(ExecEvent::Closed(code)) => Some(code),
                Ok(ExecEvent::Stdout) | Err(broadcast::error::RecvError::Lagged(_)) => {
                    let (stdout, stderr) = fresh(&state);
                    let waiting = detect_waiting_for_input(&stdout);
                    if waiting.is_waiting {
                        return Ok(SandboxExecResult {
                            stdout,
                            stderr,
                            exit_code: 0,
                            process_info: Some(process_info()),
                            requires_input: Some(true),
                            prompt: waiting.prompt,
                            detached: None,
                        });
                    }
                    state.output.lock().unwrap().exit_code
                }
                Ok(ExecEvent::Stderr) => None,
                Err(broadcast::error::RecvError::Closed) => {
                    Some(state.output.lock().unwrap().exit_code.unwrap_or(1))
                }
            };
            if let Some(code) = closed {
                let (stdout, stderr) = fresh(&state);
                return Ok(SandboxExecResult {
                    stdout,
                    stderr,
                    exit_code: code,
                    ..Default::default()
                });
            }
        }
    }

    /// Drop tracked exec processes for a session, signalling each. Called when a sandbox
    /// is stopped or removed so a later stdin follow-up fails with a clear message.
    pub fn invalidate_session_execs(&self, session_id: &str) {
        let mut running = self.running.lock().unwrap();
        running.retain(|_, state| {
            if state.session_id != session_id {
                return true;
            }
            state.kill.notify_one();
            false
        });
    }

    /// True when the PID belongs to a tracked sandbox exec (vs. a host command).
    pub fn is_sandbox_pid(&self, pid: u32) -> bool {
        self.running.lock().unwrap().contains_key(&pid)
    }
}

/// Logs output from a sandbox service. `docker logs` for the composeless path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn read_sandbox_logs(
    runner: &dyn CommandRunner,
    target: &ExecTarget,
    tail: u32,
) -> Result<LogsResult> {
    let result = if let Some(compose) = &target.compose {
        let (command, mut args) =
            compose_argv(compose.kind, &compose.compose_file, &compose.env_file);
        args.extend([
            "logs".to_string(),
            "--no-color".to_string(),
            "--tail".to_string(),
            tail.to_string(),
            target.service.clone(),
        ]);
        let options = RunOptions {
            cwd: Some(compose.cwd.clone().into()),
            ..Default::default()
        };
        runner.run(&command, &args, options).await
    } else {
        let container = target
            .container_name
            .clone()
            .ok_or_else(|| Error::msg("Composeless logs require a container name"))?;
        let args = vec![
            "logs".to_string(),
            "--tail".to_string(),
            tail.to_string(),
            container,
        ];
        runner.run("docker", &args, RunOptions::default()).await
    };
    Ok(LogsResult {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_code: result.exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RecordingSink;
    use crate::runner::{ComposeKind, FakeRunner};

    fn composeless_target() -> ExecTarget {
        ExecTarget {
            session_id: "session_1".into(),
            service: "main".into(),
            compose: None,
            container_name: Some("bedrock-sandbox-session-1-main-1".into()),
        }
    }

    fn compose_target() -> ExecTarget {
        ExecTarget {
            compose: Some(ComposeContext {
                kind: ComposeKind::Plugin,
                compose_file: "/s/docker-compose.yml".into(),
                env_file: "/s/.env".into(),
                cwd: "/s".into(),
            }),
            ..composeless_target()
        }
    }

    #[test]
    fn builds_compose_exec_argv() {
        let argv = build_exec_argv(
            &compose_target(),
            "npm ci",
            &SandboxExecOptions {
                detach: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(argv.bin, "docker");
        assert_eq!(
            argv.args,
            [
                "compose",
                "-f",
                "/s/docker-compose.yml",
                "--env-file",
                "/s/.env",
                "exec",
                "-i",
                "--workdir",
                "/workspace",
                "-d",
                "main",
                "sh",
                "-lc",
                "npm ci"
            ]
        );
        assert_eq!(argv.cwd.as_deref(), Some("/s"));
    }

    #[test]
    fn builds_composeless_exec_argv() {
        let argv = build_exec_argv(
            &composeless_target(),
            "ls",
            &SandboxExecOptions {
                cwd: Some("/data".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            argv.args,
            [
                "exec",
                "-i",
                "--workdir",
                "/data",
                "bedrock-sandbox-session-1-main-1",
                "sh",
                "-lc",
                "ls"
            ]
        );
        let missing = ExecTarget {
            container_name: None,
            ..composeless_target()
        };
        assert!(build_exec_argv(&missing, "ls", &SandboxExecOptions::default()).is_err());
    }

    #[tokio::test]
    async fn reads_logs_through_compose_or_docker() {
        let runner = FakeRunner::new();
        read_sandbox_logs(runner.as_ref(), &compose_target(), 50)
            .await
            .unwrap();
        read_sandbox_logs(runner.as_ref(), &composeless_target(), 20)
            .await
            .unwrap();
        let calls = runner.calls();
        assert_eq!(
            calls[0].1[5..],
            ["logs", "--no-color", "--tail", "50", "main"].map(String::from)
        );
        assert_eq!(calls[1].0, "docker");
        assert_eq!(
            calls[1].1,
            ["logs", "--tail", "20", "bedrock-sandbox-session-1-main-1"].map(String::from)
        );
    }

    #[cfg(unix)]
    fn registry() -> (Arc<ExecRegistry>, Arc<ActivityLog>) {
        let activity = Arc::new(ActivityLog::new(Arc::new(RecordingSink::default())));
        (Arc::new(ExecRegistry::new(activity.clone())), activity)
    }

    #[cfg(unix)]
    fn shell(script: &str) -> ExecArgv {
        ExecArgv {
            bin: "sh".into(),
            args: vec!["-c".into(), script.into()],
            cwd: None,
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn completes_and_records_the_real_exit() {
        let (registry, activity) = registry();
        let result = registry
            .spawn_tracked(
                &composeless_target(),
                "echo hi",
                &Default::default(),
                shell("echo hi; exit 2"),
            )
            .await
            .unwrap();
        assert_eq!(result.stdout, "hi\n");
        assert_eq!(result.exit_code, 2);
        let entry = &activity.get_activity("session_1").await[0];
        assert_eq!(entry.exit_code, Some(2));
        assert!(entry.ended_at.is_some());
        assert!(!registry.is_sandbox_pid(result.process_info.map(|p| p.pid).unwrap_or(0)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn resolves_early_on_a_prompt_and_accepts_a_stdin_follow_up() {
        let (registry, activity) = registry();
        let first = registry
            .spawn_tracked(
                &composeless_target(),
                "ask",
                &Default::default(),
                shell("printf 'Enter name: '; read name; echo \"got $name\""),
            )
            .await
            .unwrap();
        assert_eq!(first.requires_input, Some(true));
        assert_eq!(first.prompt.as_deref(), Some("Enter name: "));
        let pid = first.process_info.unwrap().pid;
        assert!(registry.is_sandbox_pid(pid));

        let second = registry
            .send_input(pid, "hunter2", Some(5.0))
            .await
            .unwrap();
        assert_eq!(second.stdout, "got hunter2\n");
        assert_eq!(second.exit_code, 0);

        let entry = &activity.get_activity("session_1").await[0];
        assert_eq!(entry.stdin_bytes, Some("hunter2\n".len()));
        assert_eq!(entry.outcome, ActivityOutcome::RequiresInput);

        let error = registry.send_input(pid, "Y", Some(1.0)).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("No running sandbox command found"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn resolves_early_for_a_server_and_times_out_long_commands() {
        let (registry, _activity) = registry();
        let server = registry
            .spawn_tracked(
                &composeless_target(),
                "serve",
                &Default::default(),
                shell("echo 'Server listening on 3000'; sleep 5"),
            )
            .await
            .unwrap();
        assert_eq!(server.process_info.as_ref().map(|p| p.detached), Some(true));
        registry.invalidate_session_execs("session_1");
        assert!(!registry.is_sandbox_pid(server.process_info.unwrap().pid));

        let slow = registry
            .spawn_tracked(
                &composeless_target(),
                "sleep",
                &SandboxExecOptions {
                    timeout: Some(0.2),
                    ..Default::default()
                },
                shell("sleep 5"),
            )
            .await
            .unwrap();
        assert_eq!(slow.exit_code, 124);
        assert!(slow
            .prompt
            .unwrap()
            .contains("Command still running after 0.2s"));
        registry.invalidate_session_execs("session_1");
    }
}
