//! The manager over a fake PTY (channels), plus a few runs against a real shell on Unix.

use super::*;
use crate::pty::Spawned;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

/// What a test can do to one fake shell.
struct FakeShell {
    output: Mutex<Option<Sender<Vec<u8>>>>,
    exit: Sender<Option<i32>>,
    writes: Arc<Mutex<Vec<String>>>,
    resizes: Arc<Mutex<Vec<(u16, u16)>>>,
    signals: Arc<Mutex<Vec<bool>>>,
    cwd: PathBuf,
}

impl FakeShell {
    fn push(&self, data: impl Into<Vec<u8>>) {
        self.output
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(data.into())
            .unwrap();
    }

    /// The shell exits with `code` after its output is closed.
    fn exit(&self, code: i32) {
        self.output.lock().unwrap().take();
        let _ = self.exit.send(Some(code));
    }
}

#[derive(Default)]
struct FakeBackend {
    shells: Mutex<Vec<Arc<FakeShell>>>,
    fail: Mutex<bool>,
}

impl FakeBackend {
    fn shell(&self, index: usize) -> Arc<FakeShell> {
        self.shells.lock().unwrap()[index].clone()
    }
}

struct FakeReader(Receiver<Vec<u8>>);

impl PtyReader for FakeReader {
    fn read(&mut self, timeout: Duration) -> std::io::Result<ReadOutcome> {
        match self.0.recv_timeout(timeout) {
            Ok(bytes) => Ok(ReadOutcome::Data(bytes)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(ReadOutcome::Idle),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(ReadOutcome::Eof),
        }
    }
}

struct FakeWriter(Arc<Mutex<Vec<String>>>);

impl std::io::Write for FakeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(buf).into_owned());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct FakeControl {
    resizes: Arc<Mutex<Vec<(u16, u16)>>>,
    signals: Arc<Mutex<Vec<bool>>>,
    exit: Sender<Option<i32>>,
}

impl PtyControl for FakeControl {
    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.resizes.lock().unwrap().push((cols, rows));
        Ok(())
    }
    fn terminate(&self, force: bool) {
        self.signals.lock().unwrap().push(force);
        // Dies of SIGHUP, like a shell would.
        let _ = self.exit.send(Some(129));
    }
    fn pid(&self) -> Option<u32> {
        None
    }
}

struct FakeWaiter(Receiver<Option<i32>>);

impl ChildWaiter for FakeWaiter {
    fn wait(self: Box<Self>) -> Option<i32> {
        self.0.recv().ok().flatten()
    }
}

impl PtyBackend for FakeBackend {
    fn spawn(
        &self,
        _command: &ShellCommand,
        cwd: &Path,
        _cols: u16,
        _rows: u16,
    ) -> Result<Spawned> {
        if *self.fail.lock().unwrap() {
            return Err(Error::Spawn("no such shell".into()));
        }
        let (out_tx, out_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let resizes = Arc::new(Mutex::new(Vec::new()));
        let signals = Arc::new(Mutex::new(Vec::new()));
        self.shells.lock().unwrap().push(Arc::new(FakeShell {
            output: Mutex::new(Some(out_tx)),
            exit: exit_tx.clone(),
            writes: writes.clone(),
            resizes: resizes.clone(),
            signals: signals.clone(),
            cwd: cwd.to_path_buf(),
        }));
        Ok(Spawned {
            reader: Box::new(FakeReader(out_rx)),
            writer: Box::new(FakeWriter(writes)),
            control: Box::new(FakeControl {
                resizes,
                signals,
                exit: exit_tx,
            }),
            waiter: Box::new(FakeWaiter(exit_rx)),
        })
    }
}

struct Env {
    manager: LocalTerminalManager,
    backend: Arc<FakeBackend>,
    sink: Arc<RecordingSink>,
}

impl Drop for Env {
    fn drop(&mut self) {
        self.manager.close_all();
    }
}

fn env() -> Env {
    let backend = Arc::new(FakeBackend::default());
    let sink = Arc::new(RecordingSink::default());
    Env {
        manager: LocalTerminalManager::with_backend(backend.clone(), sink.clone()),
        backend,
        sink,
    }
}

fn sh() -> ShellCommand {
    ShellCommand::new("/bin/zsh", &["-l"])
}

impl Env {
    fn open(&self, session_id: &str) -> OpenTerminalResult {
        self.manager
            .open_with(session_id, &sh(), 80, 24, Path::new("/tmp/project"))
            .unwrap()
    }
}

/// Long enough for delivery plus at least one 16ms flush.
fn settle(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Poll `check` for up to `ms`.
fn eventually(ms: u64, check: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        settle(5);
    }
    check()
}

fn exits(sink: &RecordingSink, channel: &str) -> Vec<Option<i32>> {
    sink.on_channel(channel)
        .into_iter()
        .filter_map(|m| match m {
            TerminalMessage::Exit { exit_code } => Some(exit_code),
            _ => None,
        })
        .collect()
}

#[test]
fn serializes_like_the_sandbox_terminal() {
    let to_json = |m: TerminalMessage| serde_json::to_value(m).unwrap();
    assert_eq!(
        to_json(TerminalMessage::Data {
            bytes: vec![104, 105],
            truncated: None
        }),
        serde_json::json!({ "type": "data", "bytes": [104, 105] })
    );
    assert_eq!(
        to_json(TerminalMessage::Data {
            bytes: vec![],
            truncated: Some(true)
        }),
        serde_json::json!({ "type": "data", "bytes": [], "truncated": true })
    );
    assert_eq!(
        to_json(TerminalMessage::Exit { exit_code: None }),
        serde_json::json!({ "type": "exit", "exitCode": null })
    );
    assert_eq!(
        to_json(TerminalMessage::Error {
            message: "x".into()
        }),
        serde_json::json!({ "type": "error", "message": "x" })
    );
    assert_eq!(
        serde_json::to_value(Backlog {
            backlog: vec![1, 2],
            exited: true,
            exit_code: Some(0)
        })
        .unwrap(),
        serde_json::json!({ "backlog": [1, 2], "exited": true, "exitCode": 0 })
    );
}

#[test]
fn open_returns_the_channel_shell_and_cwd() {
    let env = env();
    let opened = env.open("chat_1");
    assert_eq!(
        opened.channel,
        format!("local-terminal:{}", opened.terminal_id)
    );
    assert_eq!(opened.shell, "zsh");
    assert_eq!(opened.cwd, "/tmp/project");
    assert_eq!((opened.cols, opened.rows), (80, 24));
    assert_eq!(env.backend.shell(0).cwd, PathBuf::from("/tmp/project"));
}

#[test]
fn every_open_starts_a_new_shell() {
    let env = env();
    let a = env.open("chat_1");
    let b = env.open("chat_1");
    assert_ne!(a.terminal_id, b.terminal_id);
    assert_eq!(env.manager.count(), 2);
}

#[test]
fn a_failed_spawn_leaves_nothing_behind() {
    let env = env();
    *env.backend.fail.lock().unwrap() = true;
    let error = env
        .manager
        .open_with("chat_1", &sh(), 80, 24, Path::new("/tmp"))
        .unwrap_err();
    assert!(error.to_string().contains("no such shell"));
    assert_eq!(env.manager.count(), 0);
}

#[test]
fn buffers_output_and_publishes_nothing_until_attached() {
    let env = env();
    let opened = env.open("chat_1");
    env.backend.shell(0).push("prompt$ ");
    env.backend.shell(0).push("ls\r\n");
    settle(60);

    assert!(env.sink.on_channel(&opened.channel).is_empty());
    assert_eq!(
        env.manager.backlog(&opened.terminal_id).backlog,
        b"prompt$ ls\r\n"
    );
}

#[test]
fn attach_hands_back_the_scrollback_then_streams_live_output() {
    let env = env();
    let opened = env.open("chat_1");
    let shell = env.backend.shell(0);
    shell.push("prompt$ ");
    settle(40);

    let attached = env.manager.attach(&opened.terminal_id).unwrap();
    assert_eq!(attached.backlog, b"prompt$ ");
    assert!(!attached.exited);

    shell.push("live");
    assert!(eventually(500, || env.sink.data(&opened.channel) == b"live"));
}

#[test]
fn reattaching_returns_the_whole_backlog_and_keeps_publishing() {
    let env = env();
    let opened = env.open("chat_1");
    let shell = env.backend.shell(0);
    shell.push("one ");
    settle(40);
    env.manager.attach(&opened.terminal_id).unwrap();
    shell.push("two ");
    assert!(eventually(500, || env.sink.data(&opened.channel) == b"two "));

    // The chat page remounted: subscribe again, attach again.
    let again = env.manager.attach(&opened.terminal_id).unwrap();
    assert_eq!(again.backlog, b"one two ");
    shell.push("three");
    assert!(eventually(500, || env.sink.data(&opened.channel) == b"two three"));
}

#[test]
fn coalesces_chunks_into_few_frames() {
    let env = env();
    let opened = env.open("chat_1");
    env.manager.attach(&opened.terminal_id).unwrap();
    let shell = env.backend.shell(0);
    for i in 0..50 {
        shell.push(format!("chunk-{i} "));
    }
    assert!(eventually(500, || String::from_utf8(
        env.sink.data(&opened.channel)
    )
    .unwrap()
    .contains("chunk-49")));
    let frames = env.sink.on_channel(&opened.channel).len();
    assert!(frames < 50, "{frames} frames");
}

#[test]
fn unknown_terminals() {
    let env = env();
    assert!(env
        .manager
        .attach("nope")
        .unwrap_err()
        .to_string()
        .contains("No terminal"));
    assert!(env.manager.backlog("nope").backlog.is_empty());
    env.manager.write("nope", "ls");
    env.manager.resize("nope", 1, 1).unwrap();
    env.manager.close("nope");
}

#[test]
fn pauses_reading_past_the_high_water_mark_until_attached() {
    let env = env();
    let opened = env.open("chat_1");
    env.backend.shell(0).push(vec![b'x'; 300 * 1024]);
    assert!(eventually(500, || env
        .manager
        .is_paused(&opened.terminal_id)
        == Some(true)));

    env.manager.attach(&opened.terminal_id).unwrap();
    assert_eq!(env.manager.is_paused(&opened.terminal_id), Some(false));
}

#[test]
fn flags_truncation_once_when_a_chunk_is_enormous() {
    let env = env();
    let opened = env.open("chat_1");
    env.manager.attach(&opened.terminal_id).unwrap();
    env.backend.shell(0).push(vec![b'y'; 2 * 1024 * 1024]);
    assert!(eventually(500, || !env
        .sink
        .on_channel(&opened.channel)
        .is_empty()));
    let truncated = env
        .sink
        .on_channel(&opened.channel)
        .into_iter()
        .filter(|m| {
            matches!(
                m,
                TerminalMessage::Data {
                    truncated: Some(true),
                    ..
                }
            )
        })
        .count();
    assert_eq!(truncated, 1);
}

#[test]
fn the_backlog_stays_within_its_cap_and_starts_at_a_line() {
    let env = env();
    let opened = env.open("chat_1");
    env.manager.attach(&opened.terminal_id).unwrap();
    let shell = env.backend.shell(0);
    for i in 0..4000 {
        shell.push(format!("line {i} {}\n", ".".repeat(90)));
    }
    assert!(eventually(1000, || String::from_utf8(
        env.manager.backlog(&opened.terminal_id).backlog
    )
    .unwrap()
    .contains("line 3999")));
    let backlog = env.manager.backlog(&opened.terminal_id).backlog;
    assert!(backlog.len() <= RING_BYTES);
    assert!(String::from_utf8(backlog).unwrap().starts_with("line "));
}

#[test]
fn trim_ring_cuts_forward_to_a_newline() {
    let mut state = State::new(80, 24);
    let mut big = vec![b'a'; RING_BYTES];
    big[10] = b'\n';
    ingest(&mut state, big);
    ingest(&mut state, b"zz".to_vec());
    assert!(state.ring_bytes <= RING_BYTES);
    assert_eq!(state.ring[0][0], b'a');
    assert_eq!(state.ring_bytes, RING_BYTES - 11 + 2);
}

#[test]
fn forwards_keystrokes_and_resizes_only_on_change() {
    let env = env();
    let opened = env.open("chat_1");
    let shell = env.backend.shell(0);
    env.manager.write(&opened.terminal_id, "ls -la\r");
    env.manager.write(&opened.terminal_id, "\x03");
    assert!(eventually(500, || shell.writes.lock().unwrap().len() == 2));
    assert_eq!(shell.writes.lock().unwrap().clone(), ["ls -la\r", "\x03"]);

    env.manager.resize(&opened.terminal_id, 80, 24).unwrap();
    assert!(shell.resizes.lock().unwrap().is_empty());
    env.manager.resize(&opened.terminal_id, 100, 30).unwrap();
    env.manager.resize(&opened.terminal_id, 100, 30).unwrap();
    assert_eq!(shell.resizes.lock().unwrap().clone(), [(100, 30)]);
}

#[test]
fn reports_the_exit_and_keeps_the_terminal_listed_with_its_backlog() {
    let env = env();
    let opened = env.open("chat_1");
    env.manager.attach(&opened.terminal_id).unwrap();
    let shell = env.backend.shell(0);
    shell.push("bye\r\n");
    shell.exit(3);

    assert!(eventually(1000, || exits(&env.sink, &opened.channel) == [Some(3)]));
    // The last output went out before the exit.
    let messages = env.sink.on_channel(&opened.channel);
    assert_eq!(
        messages.last(),
        Some(&TerminalMessage::Exit { exit_code: Some(3) })
    );
    assert_eq!(env.sink.data(&opened.channel), b"bye\r\n");

    let listed = env.manager.list("chat_1");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].exited);
    assert_eq!(listed[0].exit_code, Some(3));

    // Re-attaching after the exit still works and says so.
    let again = env.manager.attach(&opened.terminal_id).unwrap();
    assert_eq!(again.backlog, b"bye\r\n");
    assert!(again.exited);
    assert_eq!(again.exit_code, Some(3));

    // Typing into it does nothing.
    env.manager.write(&opened.terminal_id, "ls\r");
    settle(30);
    assert!(shell.writes.lock().unwrap().is_empty());

    env.manager.close(&opened.terminal_id);
    assert_eq!(env.manager.count(), 0);
    // No second exit for one that already exited.
    assert_eq!(exits(&env.sink, &opened.channel), [Some(3)]);
}

#[test]
fn an_exit_before_attach_still_shows_in_the_backlog() {
    let env = env();
    let opened = env.open("chat_1");
    let shell = env.backend.shell(0);
    shell.push("done\r\n");
    shell.exit(0);
    assert!(eventually(1000, || env.manager.list("chat_1")[0].exited));

    let attached = env.manager.attach(&opened.terminal_id).unwrap();
    assert_eq!(attached.backlog, b"done\r\n");
    assert_eq!((attached.exited, attached.exit_code), (true, Some(0)));
}

#[test]
fn list_is_per_chat_in_creation_order() {
    let env = env();
    let a = env.open("chat_1");
    settle(2);
    let other = env.open("chat_2");
    settle(2);
    let b = env.open("chat_1");
    // Using the older one doesn't reorder the list.
    env.manager.attach(&a.terminal_id).unwrap();

    let listed = env.manager.list("chat_1");
    let ids: Vec<_> = listed.iter().map(|t| t.terminal_id.clone()).collect();
    assert_eq!(ids, [a.terminal_id.clone(), b.terminal_id.clone()]);
    assert!(listed[0].created_at <= listed[1].created_at);
    assert_eq!(listed[0].channel, a.channel);
    assert_eq!(listed[0].shell, "zsh");
    assert_eq!(listed[0].cwd, "/tmp/project");
    assert!(!listed[0].exited);
    assert_eq!(listed[0].exit_code, None);
    assert_eq!(env.manager.list("chat_2")[0].terminal_id, other.terminal_id);
    assert!(env.manager.list("chat_3").is_empty());
}

#[test]
fn close_signals_the_shell_publishes_an_exit_and_forgets_it() {
    let env = env();
    let opened = env.open("chat_1");
    env.manager.attach(&opened.terminal_id).unwrap();
    let shell = env.backend.shell(0);

    env.manager.close(&opened.terminal_id);
    assert_eq!(env.manager.count(), 0);
    assert_eq!(exits(&env.sink, &opened.channel), [None]);
    // SIGHUP, which the fake obeys, so no SIGKILL follows.
    settle(KILL_GRACE.as_millis() as u64 + 100);
    assert_eq!(shell.signals.lock().unwrap().clone(), [false]);
    env.manager.write(&opened.terminal_id, "ls");
    assert!(shell.writes.lock().unwrap().is_empty());
}

#[test]
fn close_session_closes_only_that_chats_terminals() {
    let env = env();
    env.open("chat_1");
    env.open("chat_1");
    let kept = env.open("chat_2");
    env.manager.close_session("chat_1");
    assert!(env.manager.list("chat_1").is_empty());
    assert_eq!(env.manager.list("chat_2")[0].terminal_id, kept.terminal_id);
}

#[test]
fn close_all_leaves_nothing_behind() {
    let env = env();
    for i in 0..3 {
        env.open(&format!("chat_{i}"));
    }
    env.manager.close_all();
    assert_eq!(env.manager.count(), 0);
    for i in 0..3 {
        assert_eq!(
            env.backend.shell(i).signals.lock().unwrap().clone(),
            [false]
        );
    }
}

#[test]
fn evicts_the_least_recently_used_past_the_limit() {
    let env = env();
    let mut opened = Vec::new();
    for i in 0..TERMINAL_LIMIT {
        opened.push(env.open(&format!("chat_{i}")));
        settle(2);
    }
    // Touch the oldest, so the second-oldest is now least recently used.
    env.manager.attach(&opened[0].terminal_id).unwrap();

    env.open("chat_new");
    assert_eq!(env.manager.count(), TERMINAL_LIMIT);
    assert_eq!(env.manager.list("chat_0").len(), 1);
    assert!(env.manager.list("chat_1").is_empty());
    assert_eq!(env.manager.list("chat_new").len(), 1);
}

#[test]
fn eviction_prefers_exited_terminals_and_tells_an_attached_one() {
    let env = env();
    let mut opened = Vec::new();
    for i in 0..TERMINAL_LIMIT {
        opened.push(env.open(&format!("chat_{i}")));
        settle(2);
    }
    env.backend.shell(5).exit(0);
    assert!(eventually(1000, || env.manager.list("chat_5")[0].exited));

    let newest = env.open("chat_new");
    assert!(env.manager.list("chat_5").is_empty());
    assert_eq!(env.manager.list("chat_0").len(), 1);

    // Next the least recently used live one goes, with a note if someone is watching.
    env.manager.attach(&opened[0].terminal_id).unwrap();
    settle(2);
    env.manager.attach(&newest.terminal_id).unwrap();
    for (i, terminal) in opened.iter().enumerate().skip(1) {
        if i != 5 {
            env.manager.attach(&terminal.terminal_id).unwrap();
        }
    }
    env.open("chat_newer");
    assert!(env.manager.list("chat_0").is_empty());
    let note = String::from_utf8(env.sink.data(&opened[0].channel)).unwrap();
    assert!(note.contains("too many open terminals"), "{note}");
}

/// Against a real shell. `/bin/sh` without `-l`, so profile scripts can't slow it down.
#[cfg(unix)]
mod native {
    use super::*;

    fn manager() -> (LocalTerminalManager, Arc<RecordingSink>) {
        let sink = Arc::new(RecordingSink::default());
        (LocalTerminalManager::new(sink.clone()), sink)
    }

    fn plain_sh() -> ShellCommand {
        let mut command = ShellCommand::new("/bin/sh", &[]);
        // A predictable prompt, whatever the user's environment says.
        command.env.push(("PS1".into(), "$ ".into()));
        command.env.push(("ENV".into(), "/dev/null".into()));
        command
    }

    #[test]
    fn runs_a_command_and_reports_the_exit_code() {
        let (manager, sink) = manager();
        let cwd = std::env::temp_dir().canonicalize().unwrap();
        let opened = manager
            .open_with("chat_1", &plain_sh(), 80, 24, &cwd)
            .unwrap();
        manager.attach(&opened.terminal_id).unwrap();

        manager.write(&opened.terminal_id, "echo hi-$((40+2)); pwd; echo $TERM\n");
        manager.write(&opened.terminal_id, "exit 7\n");

        assert!(
            eventually(5000, || exits(&sink, &opened.channel) == [Some(7)]),
            "{:?}",
            sink.on_channel(&opened.channel)
        );
        let output = String::from_utf8_lossy(&sink.data(&opened.channel)).into_owned();
        assert!(output.contains("hi-42"), "{output}");
        assert!(output.contains(&*cwd.to_string_lossy()), "{output}");
        assert!(output.contains("xterm-256color"), "{output}");

        let listed = manager.list("chat_1");
        assert!(listed[0].exited);
        assert_eq!(listed[0].exit_code, Some(7));
        manager.close(&opened.terminal_id);
    }

    #[test]
    fn close_kills_the_foreground_job_and_reaps_the_shell() {
        let (manager, _sink) = manager();
        let opened = manager
            .open_with("chat_1", &plain_sh(), 80, 24, &std::env::temp_dir())
            .unwrap();
        let pid = manager.pid(&opened.terminal_id).unwrap() as libc::pid_t;
        manager.write(&opened.terminal_id, "sleep 1000\n");
        settle(200);

        manager.close(&opened.terminal_id);
        // ESRCH only once the shell is dead *and* reaped (a zombie still accepts signal 0).
        assert!(eventually(3000, || unsafe { libc::kill(pid, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error()
                == Some(libc::ESRCH)));
    }

    #[test]
    fn resizes_the_pty() {
        let (manager, sink) = manager();
        let opened = manager
            .open_with("chat_1", &plain_sh(), 80, 24, &std::env::temp_dir())
            .unwrap();
        manager.attach(&opened.terminal_id).unwrap();
        manager.resize(&opened.terminal_id, 101, 33).unwrap();
        manager.write(&opened.terminal_id, "stty size; exit\n");
        assert!(eventually(5000, || !exits(&sink, &opened.channel).is_empty()));
        let output = String::from_utf8_lossy(&sink.data(&opened.channel)).into_owned();
        assert!(output.contains("33 101"), "{output}");
        manager.close_all();
    }
}
