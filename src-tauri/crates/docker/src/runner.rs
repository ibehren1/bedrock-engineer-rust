//! One-shot docker/compose commands. Port of `src/main/api/docker/composeRunner.ts`.
//!
//! Command execution sits behind [`CommandRunner`] so tests inject a fake, the way the TS
//! tests mock `child_process`/`./composeRunner`.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

use crate::types::{ComposeFlavor, ContainerRow};

/// Boxed `Send` future, so the traits here stay object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(120_000);
/// Grace period between SIGTERM and SIGKILL once a timeout fires.
const KILL_ESCALATION: Duration = Duration::from_millis(2000);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    /// `code ?? (timedOut ? 124 : 1)`; 127 when the binary could not be spawned.
    pub exit_code: i32,
    pub timed_out: bool,
    /// Set when the process could not be spawned at all (Node's `'error'` event).
    pub spawn_error: Option<String>,
    /// The exit code as reported by the OS; `None` when the process died to a signal.
    pub raw_exit_code: Option<i32>,
}

impl RunResult {
    /// Convenience for fakes: a completed process with this output.
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            raw_exit_code: Some(0),
            ..Default::default()
        }
    }

    /// Convenience for fakes: a completed process with this exit code and stderr.
    pub fn failed(exit_code: i32, stderr: impl Into<String>) -> Self {
        Self {
            stderr: stderr.into(),
            exit_code,
            raw_exit_code: Some(exit_code),
            ..Default::default()
        }
    }
}

/// Lets a caller terminate a running command with SIGTERM (Node's `child.kill('SIGTERM')`).
pub trait Killable: Send + Sync {
    fn kill(&self) -> std::result::Result<(), String>;
}

/// The real [`Killable`]: fires a notification the runner is waiting on.
#[derive(Default)]
pub struct KillSwitch {
    notify: Notify,
    fired: AtomicBool,
}

impl KillSwitch {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.fired.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}

impl Killable for KillSwitch {
    fn kill(&self) -> std::result::Result<(), String> {
        self.fired.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct RunOptions {
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
    /// Text written to the child's stdin, then closed.
    pub stdin: Option<String>,
    /// Terminates the process (SIGTERM) when fired.
    pub kill: Option<Arc<KillSwitch>>,
}

impl std::fmt::Debug for RunOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunOptions")
            .field("cwd", &self.cwd)
            .field("timeout", &self.timeout)
            .field("stdin", &self.stdin)
            .field("kill", &self.kill.is_some())
            .finish()
    }
}

impl RunOptions {
    pub fn timeout_ms(ms: u64) -> Self {
        Self {
            timeout: Some(Duration::from_millis(ms)),
            ..Default::default()
        }
    }
}

/// Runs a command to completion. Resolves for any exit code — callers decide what a
/// non-zero exit means.
pub trait CommandRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: RunOptions,
    ) -> BoxFuture<'a, RunResult>;
}

/// [`CommandRunner`] backed by `tokio::process`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioRunner;

/// Apply the platform flags every spawned docker process gets (`windowsHide: true`).
pub(crate) fn configure(command: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Send SIGTERM (Unix) or terminate (Windows) without waiting.
pub(crate) fn terminate(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            // SAFETY: kill(2) with a pid we own and a valid signal number has no memory
            // safety requirements.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            return;
        }
    }
    let _ = child.start_kill();
}

impl CommandRunner for TokioRunner {
    fn run<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: RunOptions,
    ) -> BoxFuture<'a, RunResult> {
        Box::pin(async move {
            let mut cmd = tokio::process::Command::new(command);
            cmd.args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(false);
            if let Some(cwd) = &options.cwd {
                cmd.current_dir(cwd);
            }
            configure(&mut cmd);

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(error) => {
                    let message = spawn_error_message(command, &error);
                    return RunResult {
                        stderr: message.clone(),
                        exit_code: 127,
                        spawn_error: Some(message),
                        ..Default::default()
                    };
                }
            };

            if let Some(mut stdin) = child.stdin.take() {
                if let Some(text) = &options.stdin {
                    let _ = stdin.write_all(text.as_bytes()).await;
                }
                drop(stdin);
            }

            let mut stdout_pipe = child.stdout.take();
            let mut stderr_pipe = child.stderr.take();
            let stdout_task = tokio::spawn(async move {
                let mut buf = Vec::new();
                if let Some(pipe) = stdout_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut buf).await;
                }
                buf
            });
            let stderr_task = tokio::spawn(async move {
                let mut buf = Vec::new();
                if let Some(pipe) = stderr_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut buf).await;
                }
                buf
            });

            let timeout = options.timeout.unwrap_or(DEFAULT_TIMEOUT);
            let mut timed_out = false;
            let kill = options.kill.clone();
            let kill_wait = async move {
                match kill {
                    Some(kill) => kill.wait().await,
                    None => std::future::pending::<()>().await,
                }
            };

            let status = tokio::select! {
                status = child.wait() => status,
                _ = tokio::time::sleep(timeout) => {
                    timed_out = true;
                    terminate(&mut child);
                    match tokio::time::timeout(KILL_ESCALATION, child.wait()).await {
                        Ok(status) => status,
                        Err(_) => {
                            let _ = child.start_kill();
                            child.wait().await
                        }
                    }
                }
                _ = kill_wait => {
                    terminate(&mut child);
                    child.wait().await
                }
            };

            let stdout =
                String::from_utf8_lossy(&stdout_task.await.unwrap_or_default()).into_owned();
            let mut stderr =
                String::from_utf8_lossy(&stderr_task.await.unwrap_or_default()).into_owned();

            let raw_exit_code = match &status {
                Ok(status) => status.code(),
                Err(error) => {
                    stderr.push_str(&error.to_string());
                    None
                }
            };
            let exit_code = raw_exit_code.unwrap_or(if timed_out { 124 } else { 1 });

            log::debug!(
                target: "docker:compose-runner",
                "{command} finished exit={exit_code} timedOut={timed_out}"
            );

            RunResult {
                stdout,
                stderr,
                exit_code,
                timed_out,
                spawn_error: None,
                raw_exit_code,
            }
        })
    }
}

/// Node formats spawn failures as `spawn <cmd> <CODE>`; keep that shape.
pub(crate) fn spawn_error_message(command: &str, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => format!("spawn {command} ENOENT"),
        std::io::ErrorKind::PermissionDenied => format!("spawn {command} EACCES"),
        _ => format!("spawn {command} {error}"),
    }
}

/// Which compose implementation drives a sandbox (never `none` here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeKind {
    Plugin,
    Standalone,
}

impl ComposeKind {
    pub fn from_flavor(flavor: ComposeFlavor) -> Option<Self> {
        match flavor {
            ComposeFlavor::Plugin => Some(ComposeKind::Plugin),
            ComposeFlavor::Standalone => Some(ComposeKind::Standalone),
            ComposeFlavor::None => None,
        }
    }
}

/// Base argv for a compose invocation against a specific sandbox.
///
/// `docker compose` (v2 plugin) is preferred; `docker-compose` (v1) is the fallback.
pub fn compose_argv(
    kind: ComposeKind,
    compose_file: &str,
    env_file: &str,
) -> (String, Vec<String>) {
    let file_args = vec![
        "-f".to_string(),
        compose_file.to_string(),
        "--env-file".to_string(),
        env_file.to_string(),
    ];
    match kind {
        ComposeKind::Plugin => {
            let mut args = vec!["compose".to_string()];
            args.extend(file_args);
            ("docker".to_string(), args)
        }
        ComposeKind::Standalone => ("docker-compose".to_string(), file_args),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeContext {
    pub kind: ComposeKind,
    pub compose_file: String,
    pub env_file: String,
    pub cwd: String,
}

/// Run a compose subcommand for a sandbox.
pub async fn compose(
    runner: &dyn CommandRunner,
    ctx: &ComposeContext,
    args: &[&str],
    mut options: RunOptions,
) -> RunResult {
    let (command, mut full) = compose_argv(ctx.kind, &ctx.compose_file, &ctx.env_file);
    full.extend(args.iter().map(|arg| arg.to_string()));
    options.cwd = Some(PathBuf::from(&ctx.cwd));
    let result = runner.run(&command, &full, options).await;
    log::debug!(
        target: "docker:compose-runner",
        "compose command finished args={} exitCode={} timedOut={}",
        args.join(" "),
        result.exit_code,
        result.timed_out
    );
    result
}

/// `docker ps` output for a compose project, parsed into rows. Uses `docker ps` rather
/// than `compose ps` so the same path works for the composeless fallback.
pub async fn list_project_containers(
    runner: &dyn CommandRunner,
    project_name: &str,
) -> Vec<ContainerRow> {
    let args: Vec<String> = vec![
        "ps".into(),
        "--all".into(),
        "--filter".into(),
        format!("label=com.docker.compose.project={project_name}"),
        "--format".into(),
        "{{.Label \"com.docker.compose.service\"}}\t{{.Names}}\t{{.State}}\t{{.Ports}}".into(),
    ];
    let result = runner.run("docker", &args, RunOptions::default()).await;

    if result.exit_code != 0 {
        log::warn!(
            target: "docker:compose-runner",
            "Failed to list sandbox containers project={project_name} stderr={}",
            result.stderr.trim()
        );
        return Vec::new();
    }

    result
        .stdout
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split('\t');
            let mut next = || parts.next().unwrap_or("").to_string();
            ContainerRow {
                service: next(),
                name: next(),
                state: next(),
                ports: next(),
            }
        })
        .collect()
}

/// A recording fake runner for tests in this crate and downstream crates.
#[derive(Default)]
pub struct FakeRunner {
    calls: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    responder: std::sync::Mutex<Option<FakeResponder>>,
}

type FakeResponder = Box<dyn FnMut(&str, &[String]) -> RunResult + Send>;

impl FakeRunner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Answer every call with `responder(command, args)`.
    pub fn respond_with(
        &self,
        responder: impl FnMut(&str, &[String]) -> RunResult + Send + 'static,
    ) {
        *self.responder.lock().unwrap() = Some(Box::new(responder));
    }

    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().unwrap().clone()
    }

    pub fn reset(&self) {
        self.calls.lock().unwrap().clear();
        *self.responder.lock().unwrap() = None;
    }
}

impl CommandRunner for FakeRunner {
    fn run<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        _options: RunOptions,
    ) -> BoxFuture<'a, RunResult> {
        self.calls
            .lock()
            .unwrap()
            .push((command.to_string(), args.to_vec()));
        let result = match self.responder.lock().unwrap().as_mut() {
            Some(responder) => responder(command, args),
            None => RunResult::ok(""),
        };
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_argv_prefers_the_plugin() {
        let (cmd, args) = compose_argv(ComposeKind::Plugin, "/s/docker-compose.yml", "/s/.env");
        assert_eq!(cmd, "docker");
        assert_eq!(
            args,
            [
                "compose",
                "-f",
                "/s/docker-compose.yml",
                "--env-file",
                "/s/.env"
            ]
        );
        let (cmd, args) = compose_argv(ComposeKind::Standalone, "c", "e");
        assert_eq!(cmd, "docker-compose");
        assert_eq!(args, ["-f", "c", "--env-file", "e"]);
    }

    #[tokio::test]
    async fn list_project_containers_parses_docker_ps() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| {
            RunResult::ok("main\tbedrock-sandbox-s-main-1\trunning\t0.0.0.0:3000->3000/tcp\n\n")
        });
        let rows = list_project_containers(runner.as_ref(), "bedrock-sandbox-s").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "bedrock-sandbox-s-main-1");
        assert_eq!(rows[0].ports, "0.0.0.0:3000->3000/tcp");
        let calls = runner.calls();
        assert_eq!(
            calls[0].1[3],
            "label=com.docker.compose.project=bedrock-sandbox-s"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tokio_runner_captures_output_and_exit_codes() {
        let runner = TokioRunner;
        let args = vec![
            "-c".to_string(),
            "printf out; printf err >&2; exit 3".to_string(),
        ];
        let result = runner.run("sh", &args, RunOptions::default()).await;
        assert_eq!(result.stdout, "out");
        assert_eq!(result.stderr, "err");
        assert_eq!(result.exit_code, 3);

        let missing = runner
            .run("definitely-not-a-binary-xyz", &[], RunOptions::default())
            .await;
        assert_eq!(missing.exit_code, 127);
        assert_eq!(
            missing.spawn_error.as_deref(),
            Some("spawn definitely-not-a-binary-xyz ENOENT")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tokio_runner_times_out_and_can_be_killed() {
        let runner = TokioRunner;
        let args = vec!["-c".to_string(), "sleep 5".to_string()];
        let result = runner.run("sh", &args, RunOptions::timeout_ms(100)).await;
        assert!(result.timed_out);
        assert_eq!(result.exit_code, 124);

        let kill = KillSwitch::new();
        let options = RunOptions {
            kill: Some(kill.clone()),
            ..Default::default()
        };
        let killer = kill.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            killer.kill().unwrap();
        });
        let result = runner.run("sh", &args, options).await;
        assert!(!result.timed_out);
        assert_eq!(result.raw_exit_code, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tokio_runner_writes_stdin() {
        let runner = TokioRunner;
        let options = RunOptions {
            stdin: Some("hello".into()),
            ..Default::default()
        };
        let result = runner.run("cat", &[], options).await;
        assert_eq!(result.stdout, "hello");
    }
}
