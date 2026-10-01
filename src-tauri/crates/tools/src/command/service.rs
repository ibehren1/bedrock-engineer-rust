//! Port of `src/main/api/command/commandService.ts`.
//!
//! Each spawned process gets a *pump* task that forwards stdout/stderr chunks and the exit
//! code into a channel, and a *listener* task that consumes those events with the same
//! state machine as the TS event handlers (`completeWithSuccess` / `completeWithError`,
//! timeouts, cleanup). `sendInput` replaces the listener, as the TS
//! `removeAllListeners` + re-subscribe does. Like the TS service, a call resolves early
//! when the output looks like an input prompt or a ready dev server, leaving the process
//! running (and tracked by PID for follow-up `sendInput` calls); timeouts never kill the
//! process.

use super::output_patterns::{detect_errors, detect_server_ready, detect_waiting_for_input};
use crate::context::CommandPatternConfig;
use crate::util::js;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{sleep_until, Instant};

/// `CommandConfig`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandConfig {
    pub allowed_commands: Vec<CommandPatternConfig>,
    pub shell: String,
}

/// `ProcessInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub command: String,
    pub detached: bool,
}

/// `DetachedProcessInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetachedProcessInfo {
    pub pid: u32,
    pub command: String,
    pub timestamp: i64,
}

/// `CommandExecutionResult`. Absent optional fields are omitted, matching the object
/// literals the TS service resolves with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_info: Option<ProcessInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_input: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// Service result: errors are the TS `Error.message`.
pub type CommandResult = std::result::Result<CommandExecutionResult, String>;

/// `executeCommand` timeout (`60000 * 5`).
pub const EXECUTE_TIMEOUT: Duration = Duration::from_secs(300);
/// `sendInput` timeout.
pub const STDIN_TIMEOUT: Duration = Duration::from_secs(5);
/// How long output is still collected after the shell exits before the exit is reported
/// (Node's `exit` can precede the last `data` events; this keeps short commands' output).
const EXIT_DRAIN: Duration = Duration::from_millis(50);

#[derive(Debug)]
enum ProcEvent {
    Stdout(String),
    Stderr(String),
    Exit(Option<i32>),
}

#[derive(Debug, Default)]
struct ProcessState {
    is_running: bool,
    has_error: bool,
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

struct Entry {
    info: DetachedProcessInfo,
    spawnargs: String,
    state: ProcessState,
    stdin: Arc<tokio::sync::Mutex<Option<ChildStdin>>>,
    events: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<ProcEvent>>>,
    generation: watch::Sender<u64>,
}

#[derive(Default)]
struct Inner {
    processes: Mutex<HashMap<u32, Entry>>,
}

impl Inner {
    fn with<R>(&self, pid: u32, f: impl FnOnce(&mut Entry) -> R) -> Option<R> {
        let mut map = self.processes.lock().unwrap_or_else(|e| e.into_inner());
        map.get_mut(&pid).map(f)
    }

    fn has(&self, pid: u32) -> bool {
        self.with(pid, |_| ()).is_some()
    }

    fn remove(&self, pid: u32) {
        let mut map = self.processes.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&pid);
    }
}

/// `CommandPattern`.
struct CommandPattern<'a> {
    command: &'a str,
    args: Vec<&'a str>,
    wildcard: bool,
}

/// `parseCommandPattern`: split on single spaces (empty segments kept, like JS).
fn parse_command_pattern(s: &str) -> CommandPattern<'_> {
    let parts: Vec<&str> = s.split(' ').collect();
    CommandPattern {
        command: parts[0],
        wildcard: parts.contains(&"*"),
        args: parts[1..].to_vec(),
    }
}

/// `isCommandAllowed`.
pub fn is_command_allowed(allowed: &[CommandPatternConfig], command: &str) -> bool {
    let exec = parse_command_pattern(command);
    allowed.iter().any(|a| {
        let p = parse_command_pattern(&a.pattern);
        if p.command != exec.command {
            return false;
        }
        if p.wildcard {
            return true;
        }
        p.args.len() == exec.args.len()
            && p.args
                .iter()
                .zip(&exec.args)
                .all(|(pa, ea)| *pa == "*" || pa == ea)
    })
}

/// `getEnhancedEnvironment`: the process PATH (already the login shell's, see
/// `common::shell_env::fix_path`) with any missing common system locations appended.
fn enhanced_path(is_windows: bool) -> String {
    let current = std::env::var("PATH")
        .or_else(|_| std::env::var("Path"))
        .unwrap_or_default();
    with_fallback_dirs(&current, is_windows)
}

/// `current` followed by the well-known directories it lacks, without repeats.
fn with_fallback_dirs(current: &str, is_windows: bool) -> String {
    let (extra, sep): (&[&str], &str) = if is_windows {
        (
            &[
                "C:\\Windows\\System32",
                "C:\\Windows",
                "C:\\Windows\\System32\\Wbem",
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\",
                "C:\\Program Files\\Amazon\\AWSCLIV2",
                "C:\\Program Files (x86)\\Amazon\\AWSCLIV2",
                "C:\\Program Files\\Git\\cmd",
                "C:\\Program Files\\nodejs",
                "C:\\Users\\Public\\chocolatey\\bin",
                "C:\\ProgramData\\chocolatey\\bin",
            ],
            ";",
        )
    } else {
        (
            &[
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
                "/opt/homebrew/bin",
                "/usr/local/aws-cli/v2/current/bin",
            ],
            ":",
        )
    };
    common::shell_env::merge_paths(
        current.split(sep).chain(extra.iter().copied()),
        sep,
        is_windows,
    )
}

/// Build the platform command and its `spawnargs` string.
fn build_command(shell: &str, command: &str, cwd: &str) -> (Command, String) {
    #[cfg(windows)]
    let (mut std_cmd, spawnargs) = {
        use std::os::windows::process::CommandExt;
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
        let _ = shell;
        let mut c = std::process::Command::new(&comspec);
        c.raw_arg(format!("/d /s /c \"{command}\""));
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW (windowsHide)
        for (k, default) in [
            ("COMSPEC", "C:\\Windows\\System32\\cmd.exe"),
            (
                "PATHEXT",
                ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC",
            ),
            ("SYSTEMROOT", "C:\\Windows"),
            ("WINDIR", "C:\\Windows"),
        ] {
            if std::env::var_os(k).is_none() {
                c.env(k, default);
            }
        }
        c.env("PATH", enhanced_path(true));
        (c, format!("{comspec} /d /s /c \"{command}\""))
    };
    #[cfg(not(windows))]
    let (mut std_cmd, spawnargs) = {
        let mut c = std::process::Command::new(shell);
        c.arg("-ic").arg(command);
        c.env("PATH", enhanced_path(false));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // `detached: true`: run in a new session, like Node's setsid() for detached
            // children, so `kill(-pid)` reaches the whole group and an interactive shell
            // doesn't touch the app's controlling terminal.
            // SAFETY: setsid is async-signal-safe and touches no Rust state.
            unsafe {
                c.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
        }
        (c, format!("{shell} -ic {command}"))
    };
    std_cmd
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (Command::from(std_cmd), spawnargs)
}

/// Forward a child's output and exit into `tx`.
async fn pump(mut child: tokio::process::Child, tx: mpsc::UnboundedSender<ProcEvent>) {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out_buf = vec![0u8; 65536];
    let mut err_buf = vec![0u8; 65536];
    let mut exit: Option<Option<i32>> = None;
    let mut exit_sent = false;
    let mut drain_deadline = Instant::now() + Duration::from_secs(86400);
    loop {
        let streams_open = stdout.is_some() || stderr.is_some();
        if let Some(code) = exit {
            if !exit_sent && (!streams_open || Instant::now() >= drain_deadline) {
                let _ = tx.send(ProcEvent::Exit(code));
                exit_sent = true;
            }
            if exit_sent && !streams_open {
                break;
            }
        }
        tokio::select! {
            r = async { stdout.as_mut().expect("guarded").read(&mut out_buf).await }, if stdout.is_some() => {
                match r {
                    Ok(n) if n > 0 => { let _ = tx.send(ProcEvent::Stdout(String::from_utf8_lossy(&out_buf[..n]).into_owned())); }
                    _ => stdout = None,
                }
            }
            r = async { stderr.as_mut().expect("guarded").read(&mut err_buf).await }, if stderr.is_some() => {
                match r {
                    Ok(n) if n > 0 => { let _ = tx.send(ProcEvent::Stderr(String::from_utf8_lossy(&err_buf[..n]).into_owned())); }
                    _ => stderr = None,
                }
            }
            status = child.wait(), if exit.is_none() => {
                exit = Some(status.ok().and_then(|s| s.code()));
                drain_deadline = Instant::now() + EXIT_DRAIN;
            }
            _ = sleep_until(drain_deadline), if exit.is_some() && !exit_sent => {}
        }
    }
}

fn code_str(code: Option<i32>) -> String {
    code.map_or_else(|| "null".to_string(), |c| c.to_string())
}

/// Listener semantics: `executeCommand` vs `sendInput` handlers differ in cleanup and in
/// when `isCompleted` is set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Execute,
    SendInput,
}

struct Listener {
    inner: Arc<Inner>,
    mode: Mode,
    pid: u32,
    /// Command reported in `processInfo.command`.
    command: String,
    out: String,
    err: String,
    is_completed: bool,
    result_tx: Option<oneshot::Sender<CommandResult>>,
}

impl Listener {
    fn resolve(&mut self, r: CommandResult) {
        if let Some(tx) = self.result_tx.take() {
            let _ = tx.send(r);
        }
    }

    fn process_info(&self) -> Option<ProcessInfo> {
        Some(ProcessInfo {
            pid: self.pid,
            command: self.command.clone(),
            detached: true,
        })
    }

    fn complete_with_error(&mut self, msg: String) {
        if self.is_completed {
            return;
        }
        self.is_completed = true;
        if self.mode == Mode::Execute {
            self.inner.remove(self.pid);
        }
        self.resolve(Err(msg));
    }

    fn complete_with_success(&mut self) {
        if self.is_completed {
            return;
        }
        let waiting = detect_waiting_for_input(&self.out);
        if self.mode == Mode::SendInput {
            self.is_completed = true;
            let r = CommandExecutionResult {
                stdout: self.out.clone(),
                stderr: self.err.clone(),
                exit_code: 0,
                process_info: self.process_info(),
                requires_input: Some(waiting.is_waiting),
                prompt: if waiting.is_waiting {
                    waiting.prompt
                } else {
                    None
                },
            };
            self.resolve(Ok(r));
            return;
        }
        let Some(code) = self.inner.with(self.pid, |e| e.state.code) else {
            self.resolve(Err("Process state not found".to_string()));
            return;
        };
        if waiting.is_waiting {
            let r = CommandExecutionResult {
                stdout: self.out.clone(),
                stderr: self.err.clone(),
                exit_code: 0,
                process_info: self.process_info(),
                requires_input: Some(true),
                prompt: waiting.prompt,
            };
            self.resolve(Ok(r));
            return;
        }
        if detect_server_ready(&self.out) {
            let r = CommandExecutionResult {
                stdout: self.out.clone(),
                stderr: self.err.clone(),
                exit_code: 0,
                process_info: self.process_info(),
                requires_input: None,
                prompt: None,
            };
            self.resolve(Ok(r));
            return;
        }
        self.is_completed = true;
        self.inner.remove(self.pid);
        let r = CommandExecutionResult {
            stdout: self.out.clone(),
            stderr: self.err.clone(),
            exit_code: code.unwrap_or(0),
            process_info: self.process_info(),
            requires_input: None,
            prompt: None,
        };
        self.resolve(Ok(r));
    }

    fn failed_message(&self) -> String {
        format!("Command failed: \n{}\n{}", self.out, self.err)
    }

    fn on_stdout(&mut self, chunk: &str) {
        self.out.push_str(chunk);
        let out = self.out.clone();
        let exists = self
            .inner
            .with(self.pid, |e| e.state.stdout = out)
            .is_some();
        if !exists && self.mode == Mode::Execute {
            return;
        }
        if detect_errors(chunk, "") {
            self.inner.with(self.pid, |e| e.state.has_error = true);
            let m = self.failed_message();
            self.complete_with_error(m);
            return;
        }
        if detect_waiting_for_input(&self.out).is_waiting || detect_server_ready(&self.out) {
            self.complete_with_success();
        }
    }

    fn on_stderr(&mut self, chunk: &str) {
        self.err.push_str(chunk);
        let err = self.err.clone();
        let exists = self
            .inner
            .with(self.pid, |e| e.state.stderr = err)
            .is_some();
        if !exists && self.mode == Mode::Execute {
            return;
        }
        if detect_errors("", chunk) {
            self.inner.with(self.pid, |e| e.state.has_error = true);
            let m = self.failed_message();
            self.complete_with_error(m);
        }
    }

    fn on_exit(&mut self, code: Option<i32>) {
        let exists = self
            .inner
            .with(self.pid, |e| {
                e.state.is_running = false;
                e.state.code = Some(code.unwrap_or(0));
            })
            .is_some();
        if exists || self.mode == Mode::SendInput {
            if !detect_errors(&self.out, &self.err) && code == Some(0) {
                self.complete_with_success();
            } else if !self.is_completed {
                let m = format!(
                    "Process exited with code {}\n{}\n{}",
                    code_str(code),
                    self.out,
                    self.err
                );
                self.complete_with_error(m);
            }
        }
        if self.mode == Mode::SendInput {
            self.inner.remove(self.pid);
        }
    }

    fn on_timeout(&mut self) {
        if self.is_completed {
            return;
        }
        let Some(has_error) = self.inner.with(self.pid, |e| e.state.has_error) else {
            return;
        };
        if has_error {
            let m = match self.mode {
                Mode::Execute => format!("Command failed to start: \n{}", self.err),
                Mode::SendInput => format!("Command failed: \n{}", self.err),
            };
            self.complete_with_error(m);
        } else if detect_server_ready(&self.out) || self.out.contains("waiting for file changes") {
            self.complete_with_success();
        } else {
            self.complete_with_error(match self.mode {
                Mode::Execute => "Command timed out".to_string(),
                Mode::SendInput => "Command timed out waiting for response".to_string(),
            });
        }
    }

    async fn run(
        mut self,
        events: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<ProcEvent>>>,
        mut generation: watch::Receiver<u64>,
        deadline: Instant,
    ) {
        let my_gen = *generation.borrow();
        let mut rx = events.lock().await;
        if self.mode == Mode::SendInput {
            // `currentOutput = state.output.stdout` — taken once the previous listener
            // has let go of the event stream.
            if let Some((o, e)) = self.inner.with(self.pid, |e| {
                (e.state.stdout.clone(), e.state.stderr.clone())
            }) {
                self.out = o;
                self.err = e;
            }
        }
        let mut timer_pending = true;
        let mut gen_alive = true;
        loop {
            if self.is_completed && self.mode == Mode::Execute {
                break;
            }
            tokio::select! {
                ev = rx.recv() => match ev {
                    None => break,
                    Some(ProcEvent::Stdout(c)) => self.on_stdout(&c),
                    Some(ProcEvent::Stderr(c)) => self.on_stderr(&c),
                    Some(ProcEvent::Exit(code)) => {
                        self.on_exit(code);
                        if self.mode == Mode::SendInput { break; }
                    }
                },
                _ = sleep_until(deadline), if timer_pending => {
                    timer_pending = false;
                    self.on_timeout();
                }
                changed = generation.changed(), if gen_alive => {
                    match changed {
                        Ok(()) if *generation.borrow() != my_gen => break,
                        Ok(()) => {}
                        Err(_) => gen_alive = false,
                    }
                }
            }
        }
        self.resolve(Err("Process listener ended without a result".to_string()));
    }
}

/// `CommandService`: host command execution with an allowlist.
pub struct CommandService {
    config: Mutex<CommandConfig>,
    inner: Arc<Inner>,
    execute_timeout: Duration,
    stdin_timeout: Duration,
}

impl CommandService {
    pub fn new(config: CommandConfig) -> Self {
        Self::with_timeouts(config, EXECUTE_TIMEOUT, STDIN_TIMEOUT)
    }

    /// Custom timeouts (tests).
    pub fn with_timeouts(config: CommandConfig, execute: Duration, stdin: Duration) -> Self {
        CommandService {
            config: Mutex::new(config),
            inner: Arc::new(Inner::default()),
            execute_timeout: execute,
            stdin_timeout: stdin,
        }
    }

    fn config(&self) -> CommandConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// `executeCommand`.
    pub async fn execute_command(&self, command: &str, cwd: &str) -> CommandResult {
        if command.trim().is_empty() {
            return Err("Invalid command: Command cannot be empty".to_string());
        }
        if cwd.is_empty() {
            return Err("Invalid working directory: cwd must be a valid string".to_string());
        }
        let config = self.config();
        if config.shell.is_empty() {
            return Err("Shell configuration is missing".to_string());
        }
        if !is_command_allowed(&config.allowed_commands, command) {
            return Err(format!("Command not allowed: {command}"));
        }

        let is_windows = cfg!(windows);
        let (mut cmd, spawnargs) = build_command(&config.shell, command, cwd);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, shell = %config.shell, command, cwd, "Process spawn failed");
                return Err(format!(
                    "Failed to start process: PID is undefined\nPlatform: {}\nShell: {}\nCommand: {}\nWorking Directory: {}\nSpawn Method: {}",
                    js::platform(),
                    config.shell,
                    command,
                    cwd,
                    if is_windows { "shell=true" } else { "shell+args" }
                ));
            }
        };
        let pid = child.id().unwrap_or_default();
        let stdin = child.stdin.take();
        let (tx, rx) = mpsc::unbounded_channel();
        let events = Arc::new(tokio::sync::Mutex::new(rx));
        let (gen_tx, gen_rx) = watch::channel(0u64);
        {
            let mut map = self
                .inner
                .processes
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            map.insert(
                pid,
                Entry {
                    info: DetachedProcessInfo {
                        pid,
                        command: command.to_string(),
                        timestamp: js::now_millis(),
                    },
                    spawnargs,
                    state: ProcessState {
                        is_running: true,
                        ..Default::default()
                    },
                    stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
                    events: events.clone(),
                    generation: gen_tx,
                },
            );
        }
        tokio::spawn(pump(child, tx));

        let (result_tx, result_rx) = oneshot::channel();
        let listener = Listener {
            inner: self.inner.clone(),
            mode: Mode::Execute,
            pid,
            command: command.to_string(),
            out: String::new(),
            err: String::new(),
            is_completed: false,
            result_tx: Some(result_tx),
        };
        let deadline = Instant::now() + self.execute_timeout;
        tokio::spawn(listener.run(events, gen_rx, deadline));
        result_rx
            .await
            .unwrap_or_else(|_| Err("Process listener ended without a result".to_string()))
    }

    /// `sendInput`: write `stdin + "\n"` to a tracked process and collect its response.
    pub async fn send_input(&self, pid: u32, stdin: &str) -> CommandResult {
        let deadline = Instant::now() + self.stdin_timeout;
        let taken = self.inner.with(pid, |e| {
            let next = *e.generation.borrow() + 1;
            e.generation.send_replace(next);
            (
                e.events.clone(),
                e.stdin.clone(),
                e.generation.subscribe(),
                e.spawnargs.clone(),
            )
        });
        let Some((events, stdin_handle, gen_rx, spawnargs)) = taken else {
            return Err(format!("No running process found with PID: {pid}"));
        };
        let (result_tx, result_rx) = oneshot::channel();
        let listener = Listener {
            inner: self.inner.clone(),
            mode: Mode::SendInput,
            pid,
            command: spawnargs,
            out: String::new(),
            err: String::new(),
            is_completed: false,
            result_tx: Some(result_tx),
        };
        tokio::spawn(listener.run(events, gen_rx, deadline));
        {
            let mut guard = stdin_handle.lock().await;
            if let Some(s) = guard.as_mut() {
                let _ = s.write_all(format!("{stdin}\n").as_bytes()).await;
                let _ = s.flush().await;
            }
        }
        result_rx
            .await
            .unwrap_or_else(|_| Err("Process listener ended without a result".to_string()))
    }

    /// `stopProcess`: terminate the process group.
    pub fn stop_process(&self, pid: u32) -> std::result::Result<(), String> {
        if !self.inner.has(pid) {
            return Ok(());
        }
        #[cfg(unix)]
        {
            // SAFETY: plain syscall with integer arguments.
            let rc = unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGTERM) };
            if rc != 0 {
                let e = std::io::Error::last_os_error();
                return Err(format!("Failed to stop process {pid}: {e}"));
            }
        }
        #[cfg(windows)]
        {
            let status = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status()
                .map_err(|e| format!("Failed to stop process {pid}: {e}"))?;
            if !status.success() {
                return Err(format!("Failed to stop process {pid}: taskkill failed"));
            }
        }
        self.inner.remove(pid);
        Ok(())
    }

    /// `getRunningProcesses`.
    pub fn get_running_processes(&self) -> Vec<DetachedProcessInfo> {
        let map = self
            .inner
            .processes
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.values().map(|e| e.info.clone()).collect()
    }

    /// Whether a PID is tracked.
    pub fn has_process(&self, pid: u32) -> bool {
        self.inner.has(pid)
    }

    /// `getAllowedCommands`.
    pub fn get_allowed_commands(&self) -> Vec<CommandPatternConfig> {
        self.config().allowed_commands
    }

    /// `updateConfig`.
    pub fn update_config(&self, config: CommandConfig) {
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = config;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn allow(patterns: &[&str]) -> Vec<CommandPatternConfig> {
        patterns
            .iter()
            .map(|p| CommandPatternConfig {
                pattern: p.to_string(),
                description: String::new(),
            })
            .collect()
    }

    fn service(patterns: &[&str], exec_timeout: Duration) -> CommandService {
        CommandService::with_timeouts(
            CommandConfig {
                allowed_commands: allow(patterns),
                shell: "/bin/sh".to_string(),
            },
            exec_timeout,
            Duration::from_secs(5),
        )
    }

    fn cwd() -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }

    #[test]
    fn fallback_dirs_follow_the_process_path_without_repeats() {
        assert_eq!(
            with_fallback_dirs("/home/me/.nvm/bin:/usr/bin:/opt/homebrew/bin", false),
            "/home/me/.nvm/bin:/usr/bin:/opt/homebrew/bin:/usr/local/bin:/bin:/usr/sbin:/sbin:/usr/local/aws-cli/v2/current/bin"
        );
        assert_eq!(
            with_fallback_dirs("", false),
            "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/aws-cli/v2/current/bin"
        );
        let win = with_fallback_dirs("c:\\windows\\system32;C:\\Tools", true);
        assert!(win.starts_with("c:\\windows\\system32;C:\\Tools;C:\\Windows;"));
        assert_eq!(win.matches("ystem32;").count(), 1);
    }

    #[test]
    fn allowlist_semantics() {
        let a = allow(&["ls", "npm run *", "git status", "cat * *"]);
        assert!(is_command_allowed(&a, "ls"));
        assert!(!is_command_allowed(&a, "ls -la"));
        assert!(is_command_allowed(&a, "npm run dev -- --port 3000"));
        // Any `*` in a pattern allows every invocation of that command (TS behavior).
        assert!(is_command_allowed(&a, "npm install"));
        assert!(is_command_allowed(&a, "cat a"));
        assert!(is_command_allowed(&a, "git status"));
        assert!(!is_command_allowed(&a, "git push"));
        assert!(!is_command_allowed(&[], "ls"));
        // Double spaces produce empty args, as JS split(' ') does.
        assert!(!is_command_allowed(&allow(&["git status"]), "git  status"));
    }

    #[tokio::test]
    async fn rejects_disallowed_and_invalid_commands() {
        let s = service(&["echo *"], EXECUTE_TIMEOUT);
        assert_eq!(
            s.execute_command("rm -rf /", &cwd()).await.unwrap_err(),
            "Command not allowed: rm -rf /"
        );
        assert_eq!(
            s.execute_command("  ", &cwd()).await.unwrap_err(),
            "Invalid command: Command cannot be empty"
        );
        assert_eq!(
            s.execute_command("echo hi", "").await.unwrap_err(),
            "Invalid working directory: cwd must be a valid string"
        );
        let no_shell = CommandService::new(CommandConfig {
            allowed_commands: allow(&["echo *"]),
            shell: String::new(),
        });
        assert_eq!(
            no_shell
                .execute_command("echo hi", &cwd())
                .await
                .unwrap_err(),
            "Shell configuration is missing"
        );
    }

    #[tokio::test]
    async fn runs_a_command_to_completion() {
        let s = service(&["echo *"], EXECUTE_TIMEOUT);
        let r = s.execute_command("echo hello", &cwd()).await.unwrap();
        assert!(r.stdout.contains("hello"), "{r:?}");
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.requires_input, None);
        let info = r.process_info.unwrap();
        assert_eq!(info.command, "echo hello");
        assert!(info.detached);
        assert!(
            !s.has_process(info.pid),
            "completed processes are cleaned up"
        );
    }

    #[tokio::test]
    async fn nonzero_exit_is_an_error() {
        let s = service(&["exit *"], EXECUTE_TIMEOUT);
        let e = s.execute_command("exit 3", &cwd()).await.unwrap_err();
        assert!(e.starts_with("Process exited with code 3\n"), "{e}");
    }

    #[tokio::test]
    async fn error_output_fails_fast() {
        let s = service(&["echo *"], EXECUTE_TIMEOUT);
        let e = s
            .execute_command("echo 'Error: boom'", &cwd())
            .await
            .unwrap_err();
        assert!(e.starts_with("Command failed: \nError: boom"), "{e}");
    }

    #[tokio::test]
    async fn bad_cwd_fails_to_start() {
        let s = service(&["echo *"], EXECUTE_TIMEOUT);
        let e = s
            .execute_command("echo hi", "/definitely/not/a/dir")
            .await
            .unwrap_err();
        assert!(
            e.starts_with("Failed to start process: PID is undefined\nPlatform: "),
            "{e}"
        );
        assert!(e.ends_with("Spawn Method: shell+args"), "{e}");
    }

    #[tokio::test]
    async fn prompt_then_stdin_round_trip() {
        let s = service(&["printf *"], EXECUTE_TIMEOUT);
        let r = s
            .execute_command("printf 'Enter name: '; read x; echo \"got $x\"", &cwd())
            .await
            .unwrap();
        assert_eq!(r.requires_input, Some(true));
        assert_eq!(r.prompt.as_deref(), Some("Enter name: "));
        let pid = r.process_info.unwrap().pid;
        assert!(s.has_process(pid));
        assert_eq!(s.get_running_processes().len(), 1);

        let r2 = s.send_input(pid, "bob").await.unwrap();
        assert!(r2.stdout.starts_with("Enter name: "), "{r2:?}");
        assert!(r2.stdout.contains("got bob"), "{r2:?}");
        assert_eq!(r2.requires_input, Some(false));
        assert_eq!(r2.prompt, None);
        assert!(r2
            .process_info
            .unwrap()
            .command
            .ends_with("-ic printf 'Enter name: '; read x; echo \"got $x\""));
        // The process exits after answering; its state is dropped.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!s.has_process(pid));
        assert_eq!(
            s.send_input(pid, "x").await.unwrap_err(),
            format!("No running process found with PID: {pid}")
        );
    }

    #[tokio::test]
    async fn server_ready_resolves_while_running() {
        let s = service(&["echo *"], EXECUTE_TIMEOUT);
        let start = std::time::Instant::now();
        let r = s
            .execute_command("echo 'Server listening on 3000'; sleep 5", &cwd())
            .await
            .unwrap();
        assert!(start.elapsed() < Duration::from_secs(4));
        assert_eq!(r.exit_code, 0);
        let pid = r.process_info.unwrap().pid;
        assert!(s.has_process(pid));
        s.stop_process(pid).unwrap();
        assert!(!s.has_process(pid));
    }

    #[tokio::test]
    async fn times_out_without_killing() {
        let s = service(&["sleep *"], Duration::from_millis(300));
        let e = s.execute_command("sleep 2", &cwd()).await.unwrap_err();
        assert_eq!(e, "Command timed out");
        assert!(s.get_running_processes().is_empty());
    }

    #[test]
    fn config_accessors() {
        let s = service(&["ls"], EXECUTE_TIMEOUT);
        assert_eq!(s.get_allowed_commands().len(), 1);
        s.update_config(CommandConfig::default());
        assert!(s.get_allowed_commands().is_empty());
    }
}
