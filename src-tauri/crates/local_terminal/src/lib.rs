//! Interactive shells on the user's own machine, on a real PTY, for the chat page's Terminal tab.
//!
//! Modelled on the Docker sandbox terminal (`docker::terminal`): output is held in a scrollback
//! ring and published in ~16ms batches on the pub/sub channel `local-terminal:<terminalId>`
//! only once the renderer has attached, with backpressure on the PTY while nobody reads it.
//!
//! The model can never reach these shells. There is no tool for any of this and nothing here is
//! in the tool registry: the only callers are the `local_terminal_*` commands, which only the
//! main window's capability grants.
//!
//! Unlike the sandbox terminal, a chat can have several shells, and a shell that exits stays
//! listed (with its scrollback) until it is closed, so the renderer can show what happened.

pub mod pty;
pub mod shell;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::pty::{ChildWaiter, NativePty, PtyBackend, PtyControl, PtyReader, ReadOutcome};
use crate::shell::ShellCommand;

/// Output is batched on this cadence — roughly one frame — instead of per chunk.
const FLUSH_INTERVAL: Duration = Duration::from_millis(16);
/// Pending bytes at which the PTY stops being read.
const HIGH_WATER_BYTES: usize = 256 * 1024;
/// Pending bytes we refuse to exceed even so; the oldest are dropped.
const PENDING_CAP_BYTES: usize = 1024 * 1024;
/// Replayable scrollback held per terminal.
const RING_BYTES: usize = 256 * 1024;
/// Terminals (running or exited) before the least recently used is closed.
pub const TERMINAL_LIMIT: usize = 16;
/// How long the reader may block before checking whether the terminal was closed.
const READ_TIMEOUT: Duration = Duration::from_millis(100);
/// After the shell exits, how long to wait for its last output before reporting the exit.
const EXIT_DRAIN: Duration = Duration::from_millis(300);
/// After `SIGHUP`, how long before `SIGKILL`.
const KILL_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No terminal {0}")]
    NotFound(String),
    #[error("Could not start the shell: {0}")]
    Spawn(String),
    #[error("{0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn terminal_channel(terminal_id: &str) -> String {
    format!("local-terminal:{terminal_id}")
}

/// Messages on `local-terminal:<terminalId>`; the same shape as the sandbox terminal's.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TerminalMessage {
    Data {
        bytes: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        truncated: Option<bool>,
    },
    Exit {
        #[serde(rename = "exitCode")]
        exit_code: Option<i32>,
    },
    Error {
        message: String,
    },
}

/// Where terminal messages go. Must not block: output flushes on a 16ms cadence.
pub trait EventSink: Send + Sync {
    fn publish(&self, channel: &str, message: TerminalMessage);
}

/// Drops everything.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl EventSink for NoopSink {
    fn publish(&self, _channel: &str, _message: TerminalMessage) {}
}

/// Keeps every message, for tests.
#[derive(Debug, Default)]
pub struct RecordingSink {
    messages: Mutex<Vec<(String, TerminalMessage)>>,
}

impl RecordingSink {
    pub fn on_channel(&self, channel: &str) -> Vec<TerminalMessage> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, _)| c == channel)
            .map(|(_, m)| m.clone())
            .collect()
    }

    /// Every `data` frame on `channel`, concatenated.
    pub fn data(&self, channel: &str) -> Vec<u8> {
        self.on_channel(channel)
            .into_iter()
            .filter_map(|m| match m {
                TerminalMessage::Data { bytes, .. } => Some(bytes),
                _ => None,
            })
            .flatten()
            .collect()
    }
}

impl EventSink for RecordingSink {
    fn publish(&self, channel: &str, message: TerminalMessage) {
        self.messages
            .lock()
            .unwrap()
            .push((channel.to_string(), message));
    }
}

/// `local_terminal_open`'s result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTerminalResult {
    pub terminal_id: String,
    pub channel: String,
    pub cols: u16,
    pub rows: u16,
    /// Display name, e.g. `zsh`.
    pub shell: String,
    /// Absolute working directory the shell started in.
    pub cwd: String,
}

/// `local_terminal_attach` / `local_terminal_backlog`'s result. `backlog` is a JSON number
/// array (serde `Vec<u8>`), which the bridge turns into a `Uint8Array`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backlog {
    pub backlog: Vec<u8>,
    /// Whether the shell has already exited (its `exit` message may predate the subscription).
    pub exited: bool,
    pub exit_code: Option<i32>,
}

/// One row of `local_terminal_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub terminal_id: String,
    pub channel: String,
    pub shell: String,
    pub cwd: String,
    pub exited: bool,
    pub exit_code: Option<i32>,
    /// Unix epoch milliseconds.
    pub created_at: u64,
}

struct State {
    cols: u16,
    rows: u16,
    last_used: Instant,
    /// True once the renderer has subscribed and asked for output.
    attached: bool,
    /// Closed by the app: removed from the list, every thread winds down.
    closed: bool,
    /// The shell exited and that has been reported.
    exited: bool,
    exit_code: Option<i32>,
    /// The shell process is gone (reaped).
    child_exited: bool,
    /// The reader thread has stopped.
    reader_done: bool,
    paused: bool,
    pending: Vec<Vec<u8>>,
    pending_bytes: usize,
    /// Set when output had to be dropped, so the UI can say so once.
    truncated: bool,
    ring: VecDeque<Vec<u8>>,
    ring_bytes: usize,
}

impl State {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            last_used: Instant::now(),
            attached: false,
            closed: false,
            exited: false,
            exit_code: None,
            child_exited: false,
            reader_done: false,
            paused: false,
            pending: Vec::new(),
            pending_bytes: 0,
            truncated: false,
            ring: VecDeque::new(),
            ring_bytes: 0,
        }
    }

    fn backlog(&self) -> Backlog {
        Backlog {
            backlog: self.ring.iter().flatten().copied().collect(),
            exited: self.exited,
            exit_code: self.exit_code,
        }
    }
}

struct Terminal {
    terminal_id: String,
    session_id: String,
    shell: String,
    cwd: String,
    created_at: u64,
    state: Mutex<State>,
    /// Wakes the reader (resume after a pause, close), flusher (output, attach, close) and
    /// waiter (reader done, close).
    changed: Condvar,
    /// Keystrokes for the writer thread; taken (closing the channel) when the terminal ends.
    input: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    /// Resize and signals; dropping it releases the PTY.
    control: Mutex<Option<Box<dyn PtyControl>>>,
}

impl Terminal {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn info(&self) -> TerminalInfo {
        let state = self.lock();
        TerminalInfo {
            terminal_id: self.terminal_id.clone(),
            channel: terminal_channel(&self.terminal_id),
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
            exited: state.exited,
            exit_code: state.exit_code,
            created_at: self.created_at,
        }
    }

    fn take_control(&self) -> Option<Box<dyn PtyControl>> {
        self.control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    fn signal(&self, force: bool) {
        if let Some(control) = self
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            control.terminate(force);
        }
    }
}

fn trim_ring(state: &mut State) {
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

fn ingest(state: &mut State, chunk: Vec<u8>) {
    state.ring_bytes += chunk.len();
    state.ring.push_back(chunk.clone());
    trim_ring(state);

    state.pending_bytes += chunk.len();
    state.pending.push(chunk);

    // Real backpressure: not reading fills the PTY's buffer, which blocks the program writing
    // to it (think `yes`).
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct Inner {
    backend: Arc<dyn PtyBackend>,
    sink: Arc<dyn EventSink>,
    /// In creation order.
    terminals: Mutex<Vec<Arc<Terminal>>>,
}

/// The local terminals. Cheap to clone.
#[derive(Clone)]
pub struct LocalTerminalManager {
    inner: Arc<Inner>,
}

impl LocalTerminalManager {
    /// Shells on the native PTY.
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self::with_backend(Arc::new(NativePty), sink)
    }

    /// Test seam: any [`PtyBackend`].
    pub fn with_backend(backend: Arc<dyn PtyBackend>, sink: Arc<dyn EventSink>) -> Self {
        Self {
            inner: Arc::new(Inner {
                backend,
                sink,
                terminals: Mutex::new(Vec::new()),
            }),
        }
    }

    fn terminals(&self) -> MutexGuard<'_, Vec<Arc<Terminal>>> {
        self.inner
            .terminals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn get(&self, terminal_id: &str) -> Option<Arc<Terminal>> {
        self.terminals()
            .iter()
            .find(|t| t.terminal_id == terminal_id)
            .cloned()
    }

    fn publish(&self, terminal_id: &str, message: TerminalMessage) {
        self.inner
            .sink
            .publish(&terminal_channel(terminal_id), message);
    }

    /// Publish pending output if attached, and resume a paused reader.
    fn flush(&self, terminal: &Terminal) {
        let (payload, truncated) = {
            let mut state = terminal.lock();
            if state.pending.is_empty() || !state.attached {
                return;
            }
            let payload = state.pending.concat();
            state.pending.clear();
            state.pending_bytes = 0;
            state.paused = false;
            (payload, std::mem::take(&mut state.truncated))
        };
        terminal.changed.notify_all();
        self.publish(
            &terminal.terminal_id,
            TerminalMessage::Data {
                bytes: payload,
                truncated: truncated.then_some(true),
            },
        );
    }

    /// Open a new shell (the user's login shell) in `cwd` for chat `session_id`. Output is
    /// buffered into the scrollback without publishing until [`attach`](Self::attach).
    pub fn open(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
        cwd: &Path,
    ) -> Result<OpenTerminalResult> {
        self.open_with(session_id, &shell::default_shell(), cols, rows, cwd)
    }

    /// [`open`](Self::open) with a given program.
    pub fn open_with(
        &self,
        session_id: &str,
        command: &ShellCommand,
        cols: u16,
        rows: u16,
        cwd: &Path,
    ) -> Result<OpenTerminalResult> {
        let (cols, rows) = (cols.max(1), rows.max(1));
        self.evict_if_needed();

        let spawned = self.inner.backend.spawn(command, cwd, cols, rows)?;
        let terminal_id = uuid::Uuid::new_v4().to_string();
        let (input_tx, input_rx) = mpsc::channel();
        let terminal = Arc::new(Terminal {
            terminal_id: terminal_id.clone(),
            session_id: session_id.to_string(),
            shell: command.display_name.clone(),
            cwd: cwd.to_string_lossy().into_owned(),
            created_at: now_ms(),
            state: Mutex::new(State::new(cols, rows)),
            changed: Condvar::new(),
            input: Mutex::new(Some(input_tx)),
            control: Mutex::new(Some(spawned.control)),
        });
        self.terminals().push(terminal.clone());

        let started = self
            .spawn_reader(terminal.clone(), spawned.reader)
            .and_then(|_| spawn_writer(spawned.writer, input_rx))
            .and_then(|_| self.spawn_flusher(terminal.clone()))
            .and_then(|_| self.spawn_waiter(terminal.clone(), spawned.waiter));
        if let Err(error) = started {
            self.close(&terminal_id);
            return Err(Error::Spawn(error.to_string()));
        }

        log::info!(
            target: "local-terminal",
            "Terminal opened terminalId={terminal_id} sessionId={session_id} shell={} cols={cols} rows={rows}",
            terminal.shell
        );

        Ok(OpenTerminalResult {
            channel: terminal_channel(&terminal_id),
            terminal_id,
            cols,
            rows,
            shell: terminal.shell.clone(),
            cwd: terminal.cwd.clone(),
        })
    }

    fn spawn_reader(
        &self,
        terminal: Arc<Terminal>,
        mut reader: Box<dyn PtyReader>,
    ) -> std::io::Result<()> {
        let manager = self.clone();
        std::thread::Builder::new()
            .name("local-terminal-reader".into())
            .spawn(move || {
                loop {
                    {
                        // Honour backpressure: stop reading while paused.
                        let state = terminal.lock();
                        let state = terminal
                            .changed
                            .wait_while(state, |s| s.paused && !s.closed)
                            .unwrap_or_else(|e| e.into_inner());
                        if state.closed {
                            break;
                        }
                    }
                    match reader.read(READ_TIMEOUT) {
                        Ok(ReadOutcome::Data(bytes)) => {
                            ingest(&mut terminal.lock(), bytes);
                            terminal.changed.notify_all();
                        }
                        Ok(ReadOutcome::Idle) => {}
                        Ok(ReadOutcome::Eof) => break,
                        Err(error) => {
                            let report = {
                                let state = terminal.lock();
                                state.attached && !state.closed
                            };
                            if report {
                                manager.publish(
                                    &terminal.terminal_id,
                                    TerminalMessage::Error {
                                        message: error.to_string(),
                                    },
                                );
                            }
                            break;
                        }
                    }
                }
                drop(reader);
                terminal.lock().reader_done = true;
                terminal.changed.notify_all();
            })
            .map(|_| ())
    }

    fn spawn_flusher(&self, terminal: Arc<Terminal>) -> std::io::Result<()> {
        let manager = self.clone();
        std::thread::Builder::new()
            .name("local-terminal-flush".into())
            .spawn(move || loop {
                {
                    // Sleep until there is something to publish, rather than ticking while idle.
                    let state = terminal.lock();
                    let state = terminal
                        .changed
                        .wait_while(state, |s| {
                            !s.closed && !s.exited && (s.pending.is_empty() || !s.attached)
                        })
                        .unwrap_or_else(|e| e.into_inner());
                    if state.closed || state.exited {
                        return;
                    }
                }
                // Let more output arrive so it goes out as one frame.
                std::thread::sleep(FLUSH_INTERVAL);
                manager.flush(&terminal);
            })
            .map(|_| ())
    }

    fn spawn_waiter(
        &self,
        terminal: Arc<Terminal>,
        waiter: Box<dyn ChildWaiter>,
    ) -> std::io::Result<()> {
        let manager = self.clone();
        std::thread::Builder::new()
            .name("local-terminal-wait".into())
            .spawn(move || {
                // Blocks until the shell exits, and reaps it.
                let exit_code = waiter.wait();
                {
                    let mut state = terminal.lock();
                    state.child_exited = true;
                    terminal.changed.notify_all();
                    // Its last output may still be in the PTY.
                    let (state, _) = terminal
                        .changed
                        .wait_timeout_while(state, EXIT_DRAIN, |s| !s.reader_done && !s.closed)
                        .unwrap_or_else(|e| e.into_inner());
                    if state.closed {
                        return;
                    }
                }
                manager.flush(&terminal);
                {
                    let mut state = terminal.lock();
                    if state.closed {
                        return;
                    }
                    state.exited = true;
                    state.exit_code = exit_code;
                }
                terminal.changed.notify_all();
                // Nothing to type into or resize any more; the scrollback stays until closed.
                terminal
                    .input
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                drop(terminal.take_control());
                manager.publish(&terminal.terminal_id, TerminalMessage::Exit { exit_code });
                log::info!(
                    target: "local-terminal",
                    "Shell exited terminalId={} exitCode={exit_code:?}",
                    terminal.terminal_id
                );
            })
            .map(|_| ())
    }

    /// Begin (or keep) publishing. Call after the renderer has subscribed to the channel; may
    /// be called again on a live or exited terminal (the chat page remounted). Returns the
    /// scrollback so far, for the renderer to write before any live frame.
    pub fn attach(&self, terminal_id: &str) -> Result<Backlog> {
        let terminal = self
            .get(terminal_id)
            .ok_or_else(|| Error::NotFound(terminal_id.to_string()))?;
        let backlog = {
            let mut state = terminal.lock();
            state.attached = true;
            state.last_used = Instant::now();
            // Everything pending is already in the ring, so it is in this backlog.
            state.pending.clear();
            state.pending_bytes = 0;
            state.paused = false;
            state.backlog()
        };
        terminal.changed.notify_all();
        Ok(backlog)
    }

    /// Scrollback for a terminal; empty for an unknown one.
    pub fn backlog(&self, terminal_id: &str) -> Backlog {
        self.get(terminal_id)
            .map(|t| t.lock().backlog())
            .unwrap_or(Backlog {
                backlog: Vec::new(),
                exited: false,
                exit_code: None,
            })
    }

    /// Type into a running shell. Ignored for an unknown, exited or closed terminal.
    pub fn write(&self, terminal_id: &str, data: &str) {
        let Some(terminal) = self.get(terminal_id) else {
            return;
        };
        {
            let mut state = terminal.lock();
            if state.closed || state.exited {
                return;
            }
            state.last_used = Instant::now();
        }
        let input = terminal.input.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(input) = input.as_ref() {
            let _ = input.send(data.as_bytes().to_vec());
        }
    }

    /// Resize the PTY when the size actually changed.
    pub fn resize(&self, terminal_id: &str, cols: u16, rows: u16) -> Result<()> {
        let Some(terminal) = self.get(terminal_id) else {
            return Ok(());
        };
        let (cols, rows) = (cols.max(1), rows.max(1));
        {
            let mut state = terminal.lock();
            if state.closed || state.exited || (state.cols == cols && state.rows == rows) {
                return Ok(());
            }
            state.cols = cols;
            state.rows = rows;
            state.last_used = Instant::now();
        }
        let control = terminal.control.lock().unwrap_or_else(|e| e.into_inner());
        match control.as_ref() {
            Some(control) => control.resize(cols, rows),
            None => Ok(()),
        }
    }

    /// Chat `session_id`'s terminals, in the order they were opened.
    pub fn list(&self, session_id: &str) -> Vec<TerminalInfo> {
        let terminals: Vec<Arc<Terminal>> = self
            .terminals()
            .iter()
            .filter(|t| t.session_id == session_id)
            .cloned()
            .collect();
        terminals.iter().map(|t| t.info()).collect()
    }

    /// Mark `terminal` closed (already out of the list) and send the shell `SIGHUP`.
    fn begin_close(&self, terminal: &Terminal) {
        let was_exited = {
            let mut state = terminal.lock();
            if state.closed {
                return;
            }
            state.closed = true;
            state.exited
        };
        terminal.changed.notify_all();
        terminal
            .input
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        terminal.signal(false);
        if !was_exited {
            self.publish(
                &terminal.terminal_id,
                TerminalMessage::Exit { exit_code: None },
            );
        }
        log::debug!(
            target: "local-terminal",
            "Terminal closed terminalId={}",
            terminal.terminal_id
        );
    }

    /// `SIGKILL` whatever ignored the `SIGHUP` within `grace`, then release the PTY.
    fn finish_close(terminal: &Terminal, grace: Duration) {
        let alive = {
            let state = terminal.lock();
            let (state, _) = terminal
                .changed
                .wait_timeout_while(state, grace, |s| !s.child_exited)
                .unwrap_or_else(|e| e.into_inner());
            !state.child_exited
        };
        if alive {
            terminal.signal(true);
        }
        // Closing the master also hangs up anything else still on the terminal.
        drop(terminal.take_control());
    }

    fn remove(&self, terminal_id: &str) -> Option<Arc<Terminal>> {
        let mut terminals = self.terminals();
        let index = terminals
            .iter()
            .position(|t| t.terminal_id == terminal_id)?;
        Some(terminals.remove(index))
    }

    /// End a shell (its whole foreground job too) and forget it. Returns at once; the kill
    /// escalation runs in the background.
    pub fn close(&self, terminal_id: &str) {
        let Some(terminal) = self.remove(terminal_id) else {
            return;
        };
        self.begin_close(&terminal);
        let spawned = std::thread::Builder::new()
            .name("local-terminal-close".into())
            .spawn({
                let terminal = terminal.clone();
                move || Self::finish_close(&terminal, KILL_GRACE)
            });
        if spawned.is_err() {
            terminal.signal(true);
            drop(terminal.take_control());
        }
    }

    /// Close every terminal of a chat (the chat was deleted).
    pub fn close_session(&self, session_id: &str) {
        let ids: Vec<String> = self
            .terminals()
            .iter()
            .filter(|t| t.session_id == session_id)
            .map(|t| t.terminal_id.clone())
            .collect();
        for id in ids {
            self.close(&id);
        }
    }

    /// Close everything and wait (briefly) for the shells to die. Called on app quit.
    pub fn close_all(&self) {
        let terminals: Vec<Arc<Terminal>> = std::mem::take(&mut *self.terminals());
        for terminal in &terminals {
            self.begin_close(terminal);
        }
        let deadline = Instant::now() + KILL_GRACE;
        for terminal in &terminals {
            Self::finish_close(terminal, deadline.saturating_duration_since(Instant::now()));
        }
    }

    fn evict_if_needed(&self) {
        loop {
            let victim = {
                let terminals = self.terminals();
                if terminals.len() < TERMINAL_LIMIT {
                    return;
                }
                // Exited shells go first, then the least recently used.
                terminals
                    .iter()
                    .min_by_key(|t| {
                        let state = t.lock();
                        (!state.exited, state.last_used)
                    })
                    .cloned()
            };
            let Some(victim) = victim else { return };

            log::info!(
                target: "local-terminal",
                "Closing least recently used terminal to stay under the limit terminalId={}",
                victim.terminal_id
            );
            let notify = {
                let state = victim.lock();
                state.attached && !state.exited
            };
            if notify {
                self.flush(&victim);
                self.publish(
                    &victim.terminal_id,
                    TerminalMessage::Data {
                        bytes: b"\r\n[terminal closed: too many open terminals]\r\n".to_vec(),
                        truncated: None,
                    },
                );
            }
            self.close(&victim.terminal_id);
        }
    }

    /// Number of terminals (running or exited).
    pub fn count(&self) -> usize {
        self.terminals().len()
    }

    /// Test seam: whether the reader is holding off for backpressure.
    pub fn is_paused(&self, terminal_id: &str) -> Option<bool> {
        self.get(terminal_id).map(|t| t.lock().paused)
    }

    /// Test seam: the shell's process id.
    pub fn pid(&self, terminal_id: &str) -> Option<u32> {
        self.get(terminal_id)?
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()?
            .pid()
    }
}

fn spawn_writer(
    mut writer: Box<dyn std::io::Write + Send>,
    input: mpsc::Receiver<Vec<u8>>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("local-terminal-writer".into())
        .spawn(move || {
            // Ends when the terminal drops its sender (exit or close).
            for data in input {
                if writer.write_all(&data).is_err() || writer.flush().is_err() {
                    return;
                }
            }
        })
        .map(|_| ())
}

#[cfg(test)]
mod tests;
