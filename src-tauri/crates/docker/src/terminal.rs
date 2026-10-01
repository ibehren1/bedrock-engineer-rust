//! Interactive shells inside sandbox containers. Port of
//! `src/main/api/docker/sandboxTerminal.ts`.
//!
//! A second, TTY-having exec path that never goes through [`crate::exec`]: its byte stream
//! must not reach the prompt/server-ready matchers, and nothing here enters the exec
//! registry, so the model cannot send stdin to a shell the user is typing into. There is no
//! tool for any of this — the only caller is the chat page.
//!
//! Like the TS, the TTY comes from the Engine API (`POST /containers/{id}/exec` with
//! `Tty: true`, then a hijacked `/exec/{id}/start`), not a local PTY: the daemon owns the
//! pseudo-terminal, which is also the only way `/exec/{id}/resize` works.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, watch, Notify};

use crate::activity::{ActivityLog, ActivityOutcome, RecordSettledInput};
use crate::engine::DockerEngine;
use crate::events::{EventSink, SandboxEvent, TerminalMessage};
use crate::runner::BoxFuture;
use crate::types::WORKSPACE_MOUNT;
use crate::{Error, Result};

/// Output is batched on this cadence — roughly one frame — instead of per chunk.
const FLUSH_INTERVAL: Duration = Duration::from_millis(16);
/// Pending bytes at which the container's stdout is paused.
const HIGH_WATER_BYTES: usize = 256 * 1024;
/// Pending bytes we refuse to exceed even so; the oldest are dropped.
const PENDING_CAP_BYTES: usize = 1024 * 1024;
/// Replayable scrollback held per terminal.
const RING_BYTES: usize = 256 * 1024;
/// Live terminals before the least recently used is closed.
const TERMINAL_LIMIT: usize = 8;

pub fn terminal_channel(terminal_id: &str) -> String {
    format!("docker-sandbox:terminal:{terminal_id}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTarget {
    pub session_id: String,
    pub service: String,
    pub container_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTerminalResult {
    pub terminal_id: String,
    pub channel: String,
    /// Which compose service this shell is inside.
    pub service: String,
    pub cols: u16,
    pub rows: u16,
}

/// Reads the container's output a chunk at a time. `None` means the stream closed.
pub trait ChunkReader: Send {
    fn next_chunk(&mut self) -> BoxFuture<'_, Option<std::io::Result<Vec<u8>>>>;
}

/// [`ChunkReader`] over any async byte stream.
pub struct StreamChunkReader<R> {
    inner: R,
    buffer: Vec<u8>,
}

impl<R: AsyncRead + Unpin + Send> StreamChunkReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: vec![0u8; 64 * 1024],
        }
    }
}

impl<R: AsyncRead + Unpin + Send> ChunkReader for StreamChunkReader<R> {
    fn next_chunk(&mut self) -> BoxFuture<'_, Option<std::io::Result<Vec<u8>>>> {
        Box::pin(async move {
            match self.inner.read(&mut self.buffer).await {
                Ok(0) => None,
                Ok(n) => Some(Ok(self.buffer[..n].to_vec())),
                Err(error) => Some(Err(error)),
            }
        })
    }
}

/// An attached exec: output reader, keystroke writer, and bytes that arrived with the
/// HTTP header.
pub struct TerminalConnection {
    pub reader: Box<dyn ChunkReader>,
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    pub initial: Vec<u8>,
}

/// The pieces of the transport this module needs, behind a trait so tests drive a plain
/// channel instead of Docker (TS `TerminalTransport`).
pub trait TerminalTransport: Send + Sync {
    fn create_exec<'a>(
        &'a self,
        container_name: &'a str,
        cols: u16,
        rows: u16,
    ) -> BoxFuture<'a, Result<String>>;
    fn start_exec<'a>(&'a self, exec_id: &'a str) -> BoxFuture<'a, Result<TerminalConnection>>;
    fn resize_exec<'a>(
        &'a self,
        exec_id: &'a str,
        cols: u16,
        rows: u16,
    ) -> BoxFuture<'a, Result<()>>;
    /// `None` when the code is unknown (or could not be read).
    fn exec_exit_code<'a>(&'a self, exec_id: &'a str) -> BoxFuture<'a, Option<i32>>;
}

const SHELL_COMMAND: [&str; 3] = [
    "/bin/sh",
    "-c",
    // Prefer bash for line editing and history; fall back to sh on minimal images.
    "if command -v bash >/dev/null 2>&1; then exec bash -il; else exec sh -i; fi",
];

/// The real transport, over the Engine API.
pub struct EngineTransport {
    engine: Arc<DockerEngine>,
}

impl EngineTransport {
    pub fn new(engine: Arc<DockerEngine>) -> Self {
        Self { engine }
    }
}

impl TerminalTransport for EngineTransport {
    fn create_exec<'a>(
        &'a self,
        container_name: &'a str,
        cols: u16,
        rows: u16,
    ) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            // Docker happily execs into a stopped container and then exits 128; check first
            // so the panel gets a message instead of a shell that dies on open.
            let inspected = self
                .engine
                .request("GET", &format!("/containers/{container_name}/json"), None)
                .await?;
            if inspected.status != 200 {
                return Err(Error::EngineUnavailable(format!(
                    "The sandbox container {container_name} could not be found (HTTP {}).",
                    inspected.status
                )));
            }
            let running = inspected
                .body
                .pointer("/State/Running")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !running {
                let status = inspected
                    .body
                    .pointer("/State/Status")
                    .and_then(Value::as_str)
                    .unwrap_or("not running");
                return Err(Error::EngineUnavailable(format!(
                    "The sandbox container is {status}. Start the sandbox before opening a terminal."
                )));
            }

            let response = self
                .engine
                .request(
                    "POST",
                    &format!("/containers/{container_name}/exec"),
                    Some(&json!({
                        "AttachStdin": true,
                        "AttachStdout": true,
                        "AttachStderr": true,
                        "Tty": true,
                        "ConsoleSize": [rows, cols],
                        "WorkingDir": WORKSPACE_MOUNT,
                        "Env": ["TERM=xterm-256color"],
                        "Cmd": SHELL_COMMAND,
                    })),
                )
                .await?;
            let id = response.body.get("Id").and_then(Value::as_str);
            match id {
                Some(id) if response.status == 201 => Ok(id.to_string()),
                _ => Err(Error::EngineUnavailable(
                    response
                        .body
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            format!(
                                "Docker refused to create a shell (HTTP {})",
                                response.status
                            )
                        }),
                )),
            }
        })
    }

    fn start_exec<'a>(&'a self, exec_id: &'a str) -> BoxFuture<'a, Result<TerminalConnection>> {
        Box::pin(async move {
            // Tty: true means one raw stream with no 8-byte frame headers.
            let hijacked = self
                .engine
                .hijack(
                    &format!("/exec/{exec_id}/start"),
                    &json!({ "Detach": false, "Tty": true }),
                )
                .await?;
            let (reader, writer) = tokio::io::split(hijacked.stream);
            Ok(TerminalConnection {
                reader: Box::new(StreamChunkReader::new(reader)),
                writer: Box::new(writer),
                initial: hijacked.initial,
            })
        })
    }

    fn resize_exec<'a>(
        &'a self,
        exec_id: &'a str,
        cols: u16,
        rows: u16,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            // Only valid once the exec is running, and only for Tty execs.
            self.engine
                .request(
                    "POST",
                    &format!("/exec/{exec_id}/resize?h={rows}&w={cols}"),
                    None,
                )
                .await?;
            Ok(())
        })
    }

    fn exec_exit_code<'a>(&'a self, exec_id: &'a str) -> BoxFuture<'a, Option<i32>> {
        Box::pin(async move {
            let response = self
                .engine
                .request("GET", &format!("/exec/{exec_id}/json"), None)
                .await
                .ok()?;
            if response.status != 200 {
                return None;
            }
            response
                .body
                .get("ExitCode")
                .and_then(Value::as_i64)
                .map(|code| code as i32)
        })
    }
}

/// Whether an interactive terminal can be opened at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCapability {
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// A remote Docker context has no local socket to attach to; say so up front.
pub async fn get_terminal_capability(engine: &DockerEngine) -> TerminalCapability {
    let endpoint = engine.resolve_endpoint(false).await;
    let Some(socket_path) = endpoint.socket_path.clone() else {
        return TerminalCapability {
            supported: false,
            endpoint: Some(endpoint.host.clone()),
            reason: Some(format!(
                "Your Docker context points at a remote daemon ({}), so the app cannot attach a shell to the container. A remote daemon also cannot mount your project folder at {WORKSPACE_MOUNT}.",
                endpoint.host
            )),
        };
    };
    match engine.request("GET", "/_ping", None).await {
        Ok(ping) if ping.status == 200 => TerminalCapability {
            supported: true,
            reason: None,
            endpoint: Some(endpoint.host),
        },
        Ok(ping) => TerminalCapability {
            supported: false,
            endpoint: Some(endpoint.host),
            reason: Some(format!(
                "The Docker socket at {socket_path} did not answer (HTTP {}).",
                ping.status
            )),
        },
        Err(error) => TerminalCapability {
            supported: false,
            reason: Some(error.to_string()),
            endpoint: None,
        },
    }
}

struct SessionState {
    cols: u16,
    rows: u16,
    last_used: Instant,
    /// True once the renderer has subscribed and asked for output.
    attached: bool,
    closed: bool,
    paused: bool,
    pending: Vec<Vec<u8>>,
    pending_bytes: usize,
    /// Set when output had to be dropped, so the UI can say so once.
    truncated: bool,
    ring: VecDeque<Vec<u8>>,
    ring_bytes: usize,
}

struct TerminalSession {
    terminal_id: String,
    session_id: String,
    service: String,
    exec_id: String,
    activity_id: String,
    created_at: Instant,
    state: Mutex<SessionState>,
    /// Wakes the reader after a pause.
    resume: Notify,
    /// Flipped to true on close; every task for this terminal watches it.
    closed_tx: watch::Sender<bool>,
    input: mpsc::UnboundedSender<Vec<u8>>,
}

impl TerminalSession {
    fn result(&self) -> OpenTerminalResult {
        let state = self.state.lock().unwrap();
        OpenTerminalResult {
            terminal_id: self.terminal_id.clone(),
            channel: terminal_channel(&self.terminal_id),
            service: self.service.clone(),
            cols: state.cols,
            rows: state.rows,
        }
    }
}

fn trim_ring(state: &mut SessionState) {
    while state.ring_bytes > RING_BYTES && !state.ring.is_empty() {
        let first_len = state.ring[0].len();
        if state.ring_bytes - first_len >= RING_BYTES {
            state.ring.pop_front();
            state.ring_bytes -= first_len;
            continue;
        }

        // Cut forward to the next newline so a replay never starts mid-escape-sequence.
        let excess = state.ring_bytes - RING_BYTES;
        let first = &state.ring[0];
        let cut = first[excess.min(first.len())..]
            .iter()
            .position(|&b| b == 0x0a)
            .map(|offset| excess + offset + 1)
            .unwrap_or(first.len());
        let remainder = first[cut..].to_vec();
        state.ring_bytes -= first_len - remainder.len();
        if remainder.is_empty() {
            state.ring.pop_front();
        } else {
            state.ring[0] = remainder;
        }
    }
}

fn ingest(state: &mut SessionState, chunk: Vec<u8>) {
    state.ring_bytes += chunk.len();
    state.ring.push_back(chunk.clone());
    trim_ring(state);

    state.pending_bytes += chunk.len();
    state.pending.push(chunk);

    // Real backpressure: not reading fills the pty's buffer inside the container, which
    // blocks the program writing to it (think `yes`).
    if !state.paused && state.pending_bytes >= HIGH_WATER_BYTES {
        state.paused = true;
    }

    if state.pending_bytes > PENDING_CAP_BYTES {
        let joined = state.pending.concat();
        let keep = joined[joined.len() - PENDING_CAP_BYTES / 2..].to_vec();
        state.pending_bytes = keep.len();
        state.pending = vec![keep];
        state.truncated = true;
    }
}

struct Inner {
    transport: RwLock<Arc<dyn TerminalTransport>>,
    activity: Arc<ActivityLog>,
    sink: Arc<dyn EventSink>,
    terminals: Mutex<Vec<Arc<TerminalSession>>>,
}

/// The live terminals. Cheap to clone.
#[derive(Clone)]
pub struct TerminalManager {
    inner: Arc<Inner>,
}

impl TerminalManager {
    pub fn new(
        transport: Arc<dyn TerminalTransport>,
        activity: Arc<ActivityLog>,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                transport: RwLock::new(transport),
                activity,
                sink,
                terminals: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Test seam: swap the Docker transport for a fake (`setTerminalTransport`).
    pub fn set_transport(&self, transport: Arc<dyn TerminalTransport>) {
        *self.inner.transport.write().unwrap() = transport;
    }

    fn transport(&self) -> Arc<dyn TerminalTransport> {
        self.inner.transport.read().unwrap().clone()
    }

    fn get(&self, terminal_id: &str) -> Option<Arc<TerminalSession>> {
        self.inner
            .terminals
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.terminal_id == terminal_id)
            .cloned()
    }

    fn publish(&self, terminal_id: &str, message: TerminalMessage) {
        self.inner.sink.publish(
            &terminal_channel(terminal_id),
            SandboxEvent::Terminal(message),
        );
    }

    /// Publish pending output if attached, and resume a paused reader.
    fn flush(&self, session: &TerminalSession) {
        let (payload, truncated, resumed) = {
            let mut state = session.state.lock().unwrap();
            if state.pending.is_empty() || !state.attached {
                return;
            }
            let payload = state.pending.concat();
            state.pending.clear();
            state.pending_bytes = 0;
            let truncated = std::mem::take(&mut state.truncated);
            let resumed = std::mem::take(&mut state.paused);
            (payload, truncated, resumed)
        };
        self.publish(
            &session.terminal_id,
            TerminalMessage::Data {
                bytes: payload,
                truncated: truncated.then_some(true),
            },
        );
        if resumed {
            session.resume.notify_one();
        }
    }

    fn finish(&self, session: &Arc<TerminalSession>, exit_code: Option<i32>) {
        {
            let mut state = session.state.lock().unwrap();
            if state.closed {
                return;
            }
            state.closed = true;
        }
        let _ = session.closed_tx.send(true);
        self.flush(session);
        self.inner
            .terminals
            .lock()
            .unwrap()
            .retain(|s| !Arc::ptr_eq(s, session));

        self.publish(&session.terminal_id, TerminalMessage::Exit { exit_code });

        // Update the row this session opened rather than adding a second one.
        self.inner.activity.record_settled(RecordSettledInput {
            session_id: session.session_id.clone(),
            id: session.activity_id.clone(),
            outcome: if matches!(exit_code, None | Some(0)) {
                ActivityOutcome::Completed
            } else {
                ActivityOutcome::Failed
            },
            exit_code,
            duration_ms: Some(session.created_at.elapsed().as_millis() as i64),
        });

        log::debug!(
            target: "docker:sandbox-terminal",
            "Terminal closed terminalId={} exitCode={exit_code:?}",
            session.terminal_id
        );
    }

    fn evict_if_needed(&self) {
        loop {
            let oldest = {
                let terminals = self.inner.terminals.lock().unwrap();
                if terminals.len() < TERMINAL_LIMIT {
                    return;
                }
                terminals
                    .iter()
                    .min_by_key(|s| s.state.lock().unwrap().last_used)
                    .cloned()
            };
            let Some(oldest) = oldest else { return };

            log::info!(
                target: "docker:sandbox-terminal",
                "Closing least recently used terminal to stay under the limit terminalId={}",
                oldest.terminal_id
            );
            if oldest.state.lock().unwrap().attached {
                self.publish(
                    &oldest.terminal_id,
                    TerminalMessage::Data {
                        bytes: b"\r\n[terminal closed: too many open sessions]\r\n".to_vec(),
                        truncated: None,
                    },
                );
            }
            self.finish(&oldest, None);
        }
    }

    /// The live terminal for a chat's service, if any. Without a service: whether the chat
    /// has any shell at all.
    pub fn find_session_terminal(
        &self,
        session_id: &str,
        service: Option<&str>,
    ) -> Option<OpenTerminalResult> {
        let terminals = self.inner.terminals.lock().unwrap();
        terminals
            .iter()
            .find(|s| {
                s.session_id == session_id
                    && !s.state.lock().unwrap().closed
                    && service.is_none_or(|svc| s.service == svc)
            })
            .map(|s| s.result())
    }

    /// Open a shell in a sandbox container. Output is buffered into the scrollback ring
    /// without publishing until [`attach_terminal`](Self::attach_terminal).
    pub async fn open_terminal(
        &self,
        target: &TerminalTarget,
        cols: u16,
        rows: u16,
    ) -> Result<OpenTerminalResult> {
        // Keyed on the service as well: a compose stack gets one shell per container.
        if let Some(existing) =
            self.find_session_terminal(&target.session_id, Some(&target.service))
        {
            return Ok(existing);
        }

        self.evict_if_needed();

        let transport = self.transport();
        let exec_id = transport
            .create_exec(&target.container_name, cols, rows)
            .await?;
        let connection = transport.start_exec(&exec_id).await?;

        let terminal_id = uuid::Uuid::new_v4().to_string();
        let activity_id = self.inner.activity.record_user_event(
            &target.session_id,
            &target.service,
            "Interactive terminal opened",
            ActivityOutcome::Running,
            None,
        );
        let (closed_tx, closed_rx) = watch::channel(false);
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let now = Instant::now();
        let session = Arc::new(TerminalSession {
            terminal_id: terminal_id.clone(),
            session_id: target.session_id.clone(),
            service: target.service.clone(),
            exec_id: exec_id.clone(),
            activity_id,
            created_at: now,
            state: Mutex::new(SessionState {
                cols,
                rows,
                last_used: now,
                attached: false,
                closed: false,
                paused: false,
                pending: Vec::new(),
                pending_bytes: 0,
                truncated: false,
                ring: VecDeque::new(),
                ring_bytes: 0,
            }),
            resume: Notify::new(),
            closed_tx,
            input: input_tx,
        });
        self.inner.terminals.lock().unwrap().push(session.clone());

        if !connection.initial.is_empty() {
            ingest(&mut session.state.lock().unwrap(), connection.initial);
        }

        self.spawn_reader(
            session.clone(),
            connection.reader,
            closed_rx.clone(),
            transport,
        );
        spawn_writer(connection.writer, input_rx, closed_rx.clone());
        self.spawn_flush_timer(session.clone(), closed_rx);

        log::info!(
            target: "docker:sandbox-terminal",
            "Terminal opened terminalId={terminal_id} sessionId={} container={} cols={cols} rows={rows}",
            target.session_id,
            target.container_name
        );

        Ok(OpenTerminalResult {
            terminal_id: terminal_id.clone(),
            channel: terminal_channel(&terminal_id),
            service: target.service.clone(),
            cols,
            rows,
        })
    }

    fn spawn_reader(
        &self,
        session: Arc<TerminalSession>,
        mut reader: Box<dyn ChunkReader>,
        mut closed: watch::Receiver<bool>,
        transport: Arc<dyn TerminalTransport>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                // Honour backpressure: stop reading while paused.
                loop {
                    let resumed = session.resume.notified();
                    tokio::pin!(resumed);
                    resumed.as_mut().enable();
                    let paused = {
                        let state = session.state.lock().unwrap();
                        if state.closed {
                            return;
                        }
                        state.paused
                    };
                    if !paused {
                        break;
                    }
                    tokio::select! {
                        _ = resumed => {}
                        _ = wait_closed(&mut closed) => return,
                    }
                }

                let chunk = tokio::select! {
                    chunk = reader.next_chunk() => chunk,
                    // Closed locally (close_terminal / eviction): finish already ran.
                    _ = wait_closed(&mut closed) => return,
                };
                match chunk {
                    Some(Ok(bytes)) => ingest(&mut session.state.lock().unwrap(), bytes),
                    Some(Err(error)) => {
                        if session.state.lock().unwrap().attached {
                            manager.publish(
                                &session.terminal_id,
                                TerminalMessage::Error {
                                    message: error.to_string(),
                                },
                            );
                        }
                        break;
                    }
                    None => break,
                }
            }
            // The shell ended (or the stream broke): report Docker's exit code.
            drop(reader);
            let exit_code = transport.exec_exit_code(&session.exec_id).await;
            manager.finish(&session, exit_code);
        });
    }

    fn spawn_flush_timer(&self, session: Arc<TerminalSession>, mut closed: watch::Receiver<bool>) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FLUSH_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = interval.tick() => manager.flush(&session),
                    _ = wait_closed(&mut closed) => return,
                }
            }
        });
    }

    /// Begin publishing. Call only after the renderer has subscribed to the channel.
    /// Returns the scrollback so far, for the renderer to write before any live frame.
    pub fn attach_terminal(&self, terminal_id: &str) -> Result<Vec<u8>> {
        let session = self
            .get(terminal_id)
            .ok_or_else(|| Error::msg(format!("No terminal {terminal_id}")))?;
        let (backlog, resumed) = {
            let mut state = session.state.lock().unwrap();
            state.attached = true;
            state.last_used = Instant::now();
            let backlog: Vec<u8> = state.ring.iter().flatten().copied().collect();
            state.pending.clear();
            state.pending_bytes = 0;
            (backlog, std::mem::take(&mut state.paused))
        };
        if resumed {
            session.resume.notify_one();
        }
        Ok(backlog)
    }

    /// Scrollback for a terminal, used when the panel is reopened.
    pub fn get_terminal_backlog(&self, terminal_id: &str) -> Vec<u8> {
        self.get(terminal_id)
            .map(|s| {
                s.state
                    .lock()
                    .unwrap()
                    .ring
                    .iter()
                    .flatten()
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn write_to_terminal(&self, terminal_id: &str, data: &str) {
        let Some(session) = self.get(terminal_id) else {
            return;
        };
        {
            let mut state = session.state.lock().unwrap();
            if state.closed {
                return;
            }
            state.last_used = Instant::now();
        }
        let _ = session.input.send(data.as_bytes().to_vec());
    }

    pub async fn resize_terminal(&self, terminal_id: &str, cols: u16, rows: u16) -> Result<()> {
        let Some(session) = self.get(terminal_id) else {
            return Ok(());
        };
        {
            let mut state = session.state.lock().unwrap();
            if state.closed || (state.cols == cols && state.rows == rows) {
                return Ok(());
            }
            state.cols = cols;
            state.rows = rows;
            state.last_used = Instant::now();
        }
        self.transport()
            .resize_exec(&session.exec_id, cols, rows)
            .await
    }

    pub fn close_terminal(&self, terminal_id: &str) {
        if let Some(session) = self.get(terminal_id) {
            self.finish(&session, None);
        }
    }

    /// Close every terminal for a chat (sandbox stopped/removed, chat deleted).
    pub fn close_session_terminals(&self, session_id: &str) {
        let ids: Vec<String> = self
            .inner
            .terminals
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.session_id == session_id)
            .map(|s| s.terminal_id.clone())
            .collect();
        for id in ids {
            self.close_terminal(&id);
        }
    }

    /// Close everything. Called on app quit, before the containers are stopped.
    pub fn close_all_terminals(&self) {
        let ids: Vec<String> = self
            .inner
            .terminals
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.terminal_id.clone())
            .collect();
        for id in ids {
            self.close_terminal(&id);
        }
    }

    /// Test seam: number of live terminals.
    pub fn count_terminals(&self) -> usize {
        self.inner.terminals.lock().unwrap().len()
    }

    /// Test seam: whether the reader is holding off for backpressure (`socket.isPaused()`).
    pub fn is_paused(&self, terminal_id: &str) -> Option<bool> {
        self.get(terminal_id)
            .map(|s| s.state.lock().unwrap().paused)
    }
}

/// Resolve once the terminal is closed (dropping the watch guard before returning).
async fn wait_closed(closed: &mut watch::Receiver<bool>) {
    let _ = closed.wait_for(|c| *c).await;
}

fn spawn_writer(
    mut writer: Box<dyn AsyncWrite + Send + Unpin>,
    mut input: mpsc::UnboundedReceiver<Vec<u8>>,
    mut closed: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                data = input.recv() => match data {
                    Some(data) => {
                        if writer.write_all(&data).await.is_err() || writer.flush().await.is_err() {
                            return;
                        }
                    }
                    None => return,
                },
                _ = wait_closed(&mut closed) => {
                    let _ = writer.shutdown().await;
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    //! Port of `sandboxTerminal.test.ts`. The PassThrough "socket" becomes a channel-fed
    //! [`ChunkReader`] plus a capturing writer; the `sandboxActivity` mocks become
    //! assertions on a real [`ActivityLog`].
    use super::*;
    use crate::activity::ActivitySource;
    use crate::events::RecordingSink;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct ChannelReader(mpsc::UnboundedReceiver<Vec<u8>>);

    impl ChunkReader for ChannelReader {
        fn next_chunk(&mut self) -> BoxFuture<'_, Option<std::io::Result<Vec<u8>>>> {
            Box::pin(async move { self.0.recv().await.map(Ok) })
        }
    }

    struct CaptureWriter(Arc<Mutex<Vec<String>>>);

    impl AsyncWrite for CaptureWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.0
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(buf).into_owned());
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    struct FakeTransport {
        initial: Vec<u8>,
        /// Handed out once, on `start_exec`.
        reader: Mutex<Option<ChannelReader>>,
        writes: Arc<Mutex<Vec<String>>>,
        resizes: Arc<Mutex<Vec<(u16, u16)>>>,
        created: Arc<Mutex<Vec<(String, u16, u16)>>>,
        exit_code: Arc<Mutex<Option<i32>>>,
        fail_create: bool,
        /// The output side of the fake socket. Held here so the stream stays open for as
        /// long as the transport is referenced, like the TS PassThrough.
        sender: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    }

    impl TerminalTransport for FakeTransport {
        fn create_exec<'a>(
            &'a self,
            container_name: &'a str,
            cols: u16,
            rows: u16,
        ) -> BoxFuture<'a, Result<String>> {
            Box::pin(async move {
                if self.fail_create {
                    return Err(Error::msg(
                        "The sandbox container is exited. Start the sandbox before opening a terminal.",
                    ));
                }
                self.created
                    .lock()
                    .unwrap()
                    .push((container_name.to_string(), cols, rows));
                Ok("exec-1".to_string())
            })
        }
        fn start_exec<'a>(
            &'a self,
            _exec_id: &'a str,
        ) -> BoxFuture<'a, Result<TerminalConnection>> {
            Box::pin(async move {
                let reader = self.reader.lock().unwrap().take().expect("single start");
                Ok(TerminalConnection {
                    reader: Box::new(reader),
                    writer: Box::new(CaptureWriter(self.writes.clone())),
                    initial: self.initial.clone(),
                })
            })
        }
        fn resize_exec<'a>(
            &'a self,
            _exec_id: &'a str,
            cols: u16,
            rows: u16,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async move {
                self.resizes.lock().unwrap().push((cols, rows));
                Ok(())
            })
        }
        fn exec_exit_code<'a>(&'a self, _exec_id: &'a str) -> BoxFuture<'a, Option<i32>> {
            Box::pin(async move { *self.exit_code.lock().unwrap() })
        }
    }

    struct Harness {
        transport: Arc<FakeTransport>,
        writes: Arc<Mutex<Vec<String>>>,
        resizes: Arc<Mutex<Vec<(u16, u16)>>>,
        created: Arc<Mutex<Vec<(String, u16, u16)>>>,
        exit_code: Arc<Mutex<Option<i32>>>,
    }

    impl Harness {
        /// Push container output.
        fn push(&self, data: impl Into<Vec<u8>>) {
            let sender = self.transport.sender.lock().unwrap();
            sender.as_ref().unwrap().send(data.into()).unwrap();
        }

        /// Close the stream from the container side (`stream.destroy()`).
        fn destroy(&self) {
            self.transport.sender.lock().unwrap().take();
        }
    }

    fn make_harness(initial: &str) -> Harness {
        make_harness_with(initial, false)
    }

    fn make_harness_with(initial: &str, fail_create: bool) -> Harness {
        let (tx, rx) = mpsc::unbounded_channel();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let resizes = Arc::new(Mutex::new(Vec::new()));
        let created = Arc::new(Mutex::new(Vec::new()));
        let exit_code = Arc::new(Mutex::new(Some(0)));
        Harness {
            transport: Arc::new(FakeTransport {
                initial: initial.as_bytes().to_vec(),
                reader: Mutex::new(Some(ChannelReader(rx))),
                writes: writes.clone(),
                resizes: resizes.clone(),
                created: created.clone(),
                exit_code: exit_code.clone(),
                fail_create,
                sender: Mutex::new(Some(tx)),
            }),
            writes,
            resizes,
            created,
            exit_code,
        }
    }

    struct Env {
        manager: TerminalManager,
        sink: Arc<RecordingSink>,
        activity: Arc<ActivityLog>,
    }

    impl Drop for Env {
        fn drop(&mut self) {
            self.manager.close_all_terminals();
        }
    }

    fn env() -> Env {
        let sink = Arc::new(RecordingSink::default());
        let activity = Arc::new(ActivityLog::new(Arc::new(RecordingSink::default())));
        let manager =
            TerminalManager::new(make_harness("").transport, activity.clone(), sink.clone());
        Env {
            manager,
            sink,
            activity,
        }
    }

    fn target() -> TerminalTarget {
        TerminalTarget {
            session_id: "session_1".into(),
            service: "main".into(),
            container_name: "sandbox-main-1".into(),
        }
    }

    fn target_with(session_id: &str, service: &str, container_name: &str) -> TerminalTarget {
        TerminalTarget {
            session_id: session_id.into(),
            service: service.into(),
            container_name: container_name.into(),
        }
    }

    /// Long enough for delivery plus at least one 16ms flush tick.
    async fn settle(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn data_frames(sink: &RecordingSink, channel: &str) -> Vec<(Vec<u8>, Option<bool>)> {
        sink.on_channel(channel)
            .into_iter()
            .filter_map(|event| match event {
                SandboxEvent::Terminal(TerminalMessage::Data { bytes, truncated }) => {
                    Some((bytes, truncated))
                }
                _ => None,
            })
            .collect()
    }

    // describe('openTerminal')
    #[tokio::test]
    async fn creates_the_exec_with_the_requested_size_and_returns_a_channel() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());

        let result = env.manager.open_terminal(&target(), 120, 40).await.unwrap();

        assert_eq!(
            harness.created.lock().unwrap().clone(),
            vec![("sandbox-main-1".to_string(), 120, 40)]
        );
        assert_eq!(result.channel, terminal_channel(&result.terminal_id));
        assert_eq!(result.cols, 120);
    }

    #[tokio::test]
    async fn reuses_the_existing_terminal_for_a_chat_rather_than_opening_a_second_shell() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);

        let first = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        let second = env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        assert_eq!(second.terminal_id, first.terminal_id);
        assert_eq!(env.manager.count_terminals(), 1);
        assert_eq!(
            env.manager
                .find_session_terminal("session_1", None)
                .map(|t| t.terminal_id),
            Some(first.terminal_id)
        );
    }

    #[tokio::test]
    async fn gives_each_compose_service_its_own_shell() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);
        let main = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.set_transport(make_harness("").transport);
        let db = env
            .manager
            .open_terminal(&target_with("session_1", "db", "sandbox-db-1"), 80, 24)
            .await
            .unwrap();

        assert_ne!(db.terminal_id, main.terminal_id);
        assert_eq!(env.manager.count_terminals(), 2);
        assert_eq!(
            env.manager
                .find_session_terminal("session_1", Some("main"))
                .map(|t| t.terminal_id),
            Some(main.terminal_id)
        );
        assert_eq!(
            env.manager
                .find_session_terminal("session_1", Some("db"))
                .map(|t| t.terminal_id),
            Some(db.terminal_id.clone())
        );
        assert_eq!(db.service, "db");
    }

    #[tokio::test]
    async fn reuses_the_shell_for_the_same_service_and_closes_both_with_the_chat() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);
        let first = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        let again = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        assert_eq!(again.terminal_id, first.terminal_id);

        env.manager.set_transport(make_harness("").transport);
        env.manager
            .open_terminal(&target_with("session_1", "db", "sandbox-db-1"), 80, 24)
            .await
            .unwrap();
        assert_eq!(env.manager.count_terminals(), 2);

        env.manager.close_session_terminals("session_1");
        assert_eq!(env.manager.count_terminals(), 0);
    }

    #[tokio::test]
    async fn records_the_session_in_the_activity_log() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);
        env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        let entries = env.activity.get_activity("session_1").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].service, "main");
        assert_eq!(entries[0].command, "Interactive terminal opened");
        assert_eq!(entries[0].source, ActivitySource::User);
        assert_eq!(entries[0].outcome, ActivityOutcome::Running);
    }

    #[tokio::test]
    async fn records_no_activity_row_when_the_shell_could_not_be_started() {
        let env = env();
        env.manager
            .set_transport(make_harness_with("", true).transport);

        let error = env
            .manager
            .open_terminal(&target(), 80, 24)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Start the sandbox"));
        assert!(env.activity.get_activity("session_1").await.is_empty());
        assert_eq!(env.manager.count_terminals(), 0);
    }

    #[tokio::test]
    async fn buffers_output_and_publishes_nothing_until_the_renderer_attaches() {
        let env = env();
        let harness = make_harness("root@abc:/# ");
        env.manager.set_transport(harness.transport.clone());

        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        harness.push("ls\r\n");
        settle(40).await;

        // No replay on the event bus, so publishing before the subscription would lose it.
        assert!(data_frames(&env.sink, &opened.channel).is_empty());
        assert_eq!(
            String::from_utf8(env.manager.get_terminal_backlog(&opened.terminal_id)).unwrap(),
            "root@abc:/# ls\r\n"
        );
    }

    // describe('attachTerminal')
    #[tokio::test]
    async fn hands_back_the_scrollback_so_far_then_streams_live_output() {
        let env = env();
        let harness = make_harness("prompt> ");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        settle(40).await;

        let backlog = env.manager.attach_terminal(&opened.terminal_id).unwrap();
        assert_eq!(String::from_utf8(backlog).unwrap(), "prompt> ");

        harness.push("live output");
        settle(40).await;

        let joined: Vec<u8> = data_frames(&env.sink, &opened.channel)
            .into_iter()
            .flat_map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(String::from_utf8(joined).unwrap(), "live output");
    }

    #[tokio::test]
    async fn coalesces_several_chunks_into_one_frame_per_flush() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.attach_terminal(&opened.terminal_id).unwrap();

        for i in 0..20 {
            harness.push(format!("chunk-{i} "));
        }
        settle(40).await;

        let frames = data_frames(&env.sink, &opened.channel);
        assert!(frames.len() < 20, "{} frames", frames.len());
        let joined: Vec<u8> = frames.into_iter().flat_map(|(bytes, _)| bytes).collect();
        assert!(String::from_utf8(joined).unwrap().contains("chunk-19"));
    }

    #[tokio::test]
    async fn throws_for_an_unknown_terminal_rather_than_silently_doing_nothing() {
        let env = env();
        let error = env.manager.attach_terminal("nope").unwrap_err();
        assert!(error.to_string().contains("No terminal"));
    }

    // describe('backpressure')
    #[tokio::test]
    async fn pauses_the_container_stream_past_the_high_water_mark_and_resumes_after_a_flush() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        // 300 KB with no subscriber: more than the 256 KB high-water mark.
        harness.push(vec![b'x'; 300 * 1024]);
        settle(40).await;
        assert_eq!(env.manager.is_paused(&opened.terminal_id), Some(true));

        env.manager.attach_terminal(&opened.terminal_id).unwrap();
        settle(40).await;
        assert_eq!(env.manager.is_paused(&opened.terminal_id), Some(false));
    }

    #[tokio::test]
    async fn drops_the_oldest_output_and_flags_truncation_when_a_single_chunk_is_enormous() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        // The cap is a backstop for one oversized chunk; chunked output pauses first.
        harness.push(vec![b'y'; 2 * 1024 * 1024]);
        settle(80).await;

        env.manager.attach_terminal(&opened.terminal_id).unwrap();
        harness.push("tail");
        settle(40).await;

        let truncated = data_frames(&env.sink, &opened.channel)
            .into_iter()
            .filter(|(_, truncated)| *truncated == Some(true))
            .count();
        assert_eq!(truncated, 1);
    }

    // describe('scrollback ring')
    #[tokio::test]
    async fn stays_within_its_cap_and_resumes_at_a_line_boundary() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.attach_terminal(&opened.terminal_id).unwrap();

        // 400 KB of numbered lines, past the 256 KB ring.
        for i in 0..4000 {
            harness.push(format!("line {i} {}\n", ".".repeat(90)));
        }
        settle(150).await;

        let backlog = env.manager.get_terminal_backlog(&opened.terminal_id);
        assert!(backlog.len() <= 256 * 1024);
        let text = String::from_utf8(backlog).unwrap();
        // Trimming cuts forward to a newline, so a replay never starts mid-sequence.
        assert!(text.starts_with("line "));
        assert!(text.contains("line 3999"));
    }

    // describe('input and resize')
    #[tokio::test]
    async fn forwards_keystrokes_to_the_container() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        env.manager
            .write_to_terminal(&opened.terminal_id, "ls -la\r");
        env.manager.write_to_terminal(&opened.terminal_id, "\x03");
        settle(20).await;

        assert_eq!(harness.writes.lock().unwrap().clone(), ["ls -la\r", "\x03"]);
    }

    #[tokio::test]
    async fn ignores_writes_to_a_closed_terminal_instead_of_throwing() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.close_terminal(&opened.terminal_id);
        env.manager.write_to_terminal(&opened.terminal_id, "ls");
    }

    #[tokio::test]
    async fn only_calls_docker_when_the_size_actually_changed() {
        let env = env();
        let harness = make_harness("");
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();

        env.manager
            .resize_terminal(&opened.terminal_id, 80, 24)
            .await
            .unwrap();
        assert!(harness.resizes.lock().unwrap().is_empty());

        env.manager
            .resize_terminal(&opened.terminal_id, 100, 30)
            .await
            .unwrap();
        env.manager
            .resize_terminal(&opened.terminal_id, 100, 30)
            .await
            .unwrap();
        assert_eq!(harness.resizes.lock().unwrap().clone(), [(100, 30)]);
    }

    // describe('lifecycle')
    #[tokio::test]
    async fn publishes_an_exit_with_the_code_docker_reports_when_the_shell_ends() {
        let env = env();
        let harness = make_harness("");
        *harness.exit_code.lock().unwrap() = Some(3);
        env.manager.set_transport(harness.transport.clone());
        let opened = env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.attach_terminal(&opened.terminal_id).unwrap();

        harness.destroy();
        settle(40).await;

        assert!(env
            .sink
            .on_channel(&opened.channel)
            .contains(&SandboxEvent::Terminal(TerminalMessage::Exit {
                exit_code: Some(3)
            })));
        assert_eq!(env.manager.count_terminals(), 0);
        // The row this session opened is closed out, rather than a second row being added.
        let entries = env.activity.get_activity("session_1").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, ActivityOutcome::Failed);
        assert_eq!(entries[0].exit_code, Some(3));
    }

    #[tokio::test]
    async fn closes_every_terminal_belonging_to_one_chat_leaving_others_alone() {
        let env = env();
        env.manager.set_transport(make_harness("").transport);
        env.manager.open_terminal(&target(), 80, 24).await.unwrap();
        env.manager.set_transport(make_harness("").transport);
        env.manager
            .open_terminal(&target_with("session_2", "main", "sandbox-main-1"), 80, 24)
            .await
            .unwrap();
        assert_eq!(env.manager.count_terminals(), 2);

        env.manager.close_session_terminals("session_1");

        assert!(env
            .manager
            .find_session_terminal("session_1", None)
            .is_none());
        assert!(env
            .manager
            .find_session_terminal("session_2", None)
            .is_some());
        assert_eq!(env.manager.count_terminals(), 1);
    }

    #[tokio::test]
    async fn evicts_the_least_recently_used_terminal_past_the_cap_of_8() {
        let env = env();
        for i in 0..9 {
            env.manager.set_transport(make_harness("").transport);
            env.manager
                .open_terminal(&target_with(&format!("session_{i}"), "main", "c"), 80, 24)
                .await
                .unwrap();
            // Space the timestamps so "least recently used" is well defined.
            settle(2).await;
        }

        assert_eq!(env.manager.count_terminals(), 8);
        assert!(env
            .manager
            .find_session_terminal("session_0", None)
            .is_none());
        assert!(env
            .manager
            .find_session_terminal("session_8", None)
            .is_some());
    }

    #[tokio::test]
    async fn close_all_terminals_leaves_nothing_behind() {
        let env = env();
        for i in 0..3 {
            env.manager.set_transport(make_harness("").transport);
            env.manager
                .open_terminal(&target_with(&format!("chat_{i}"), "main", "c"), 80, 24)
                .await
                .unwrap();
        }

        env.manager.close_all_terminals();
        assert_eq!(env.manager.count_terminals(), 0);
    }

    #[test]
    fn trim_ring_cuts_forward_to_a_newline() {
        let mut state = SessionState {
            cols: 80,
            rows: 24,
            last_used: Instant::now(),
            attached: false,
            closed: false,
            paused: false,
            pending: Vec::new(),
            pending_bytes: 0,
            truncated: false,
            ring: VecDeque::new(),
            ring_bytes: 0,
        };
        let mut big = vec![b'a'; RING_BYTES];
        big[10] = b'\n';
        ingest(&mut state, big);
        ingest(&mut state, b"zz".to_vec());
        assert!(state.ring_bytes <= RING_BYTES);
        assert_eq!(state.ring[0][0], b'a');
        assert_eq!(state.ring_bytes, RING_BYTES - 11 + 2);
    }
}
