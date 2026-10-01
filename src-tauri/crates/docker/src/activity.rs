//! Record of what has happened inside a chat's sandbox: every command the agent ran, and
//! every time the user opened an interactive terminal. Port of
//! `src/main/api/docker/sandboxActivity.ts`.
//!
//! Persistence is best-effort and runs on a dedicated writer thread fed in order, which is
//! the TS per-session promise queue: appends never interleave a rotation, and a failed
//! write never becomes an error for the caller.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::events::{ActivityEvent, EventSink, SandboxEvent};
use crate::util::{now_iso, parse_iso_ms};

/// Filename inside the sandbox folder, alongside sandbox.json.
pub const ACTIVITY_FILENAME: &str = "activity.jsonl";

/// Entries held per session for the UI.
const MEMORY_LIMIT: usize = 200;
/// Sessions tracked before the least recently touched is dropped.
const SESSION_LIMIT: usize = 20;
/// Lines kept on disk before the file is rewritten without its oldest half.
const FILE_LINE_LIMIT: usize = 500;
/// Commands longer than this are stored truncated.
const COMMAND_LIMIT: usize = 2048;

pub fn activity_channel(session_id: &str) -> String {
    format!("docker-sandbox:activity:{session_id}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivityOutcome {
    Running,
    Completed,
    RequiresInput,
    Detached,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivitySource {
    Agent,
    User,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxActivityEntry {
    pub id: String,
    pub session_id: String,
    pub service: String,
    /// The command, or a description for user events like opening a terminal.
    pub command: String,
    pub source: ActivitySource,
    pub outcome: ActivityOutcome,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_bytes: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_bytes: Option<usize>,
    /// Bytes of stdin sent in a follow-up. The content is deliberately not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct RecordStartInput {
    pub session_id: String,
    pub service: String,
    pub command: String,
    pub source: Option<ActivitySource>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RecordSettledInput {
    pub session_id: String,
    pub id: String,
    pub outcome: ActivityOutcome,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct RecordExitInput {
    pub session_id: String,
    pub id: String,
    pub exit_code: i32,
    pub stdout_bytes: Option<usize>,
    pub stderr_bytes: Option<usize>,
}

/// Resolves a session's sandbox folder. Injected so this module does not depend on the
/// sandbox manager.
pub type DirectoryResolver = Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

enum Job {
    Persist(Box<SandboxActivityEntry>),
    Clear(String, tokio::sync::oneshot::Sender<()>),
    Flush(tokio::sync::oneshot::Sender<()>),
}

/// The per-session activity log.
pub struct ActivityLog {
    /// Insertion-ordered: the first entry is the least recently touched session.
    sessions: Mutex<Vec<(String, Vec<SandboxActivityEntry>)>>,
    resolver: Arc<RwLock<DirectoryResolver>>,
    sink: Arc<dyn EventSink>,
    writer: Mutex<mpsc::Sender<Job>>,
}

impl ActivityLog {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        let resolver: Arc<RwLock<DirectoryResolver>> = Arc::new(RwLock::new(Arc::new(|_| None)));
        let (tx, rx) = mpsc::channel::<Job>();
        let worker_resolver = resolver.clone();
        std::thread::Builder::new()
            .name("sandbox-activity-writer".into())
            .spawn(move || {
                for job in rx {
                    let resolve = worker_resolver.read().unwrap().clone();
                    match job {
                        Job::Persist(entry) => {
                            if let Err(error) = persist(&resolve, &entry) {
                                // Best-effort: a sandbox whose folder was just deleted must
                                // not turn a command into a tool error.
                                log::debug!(
                                    target: "docker:sandbox-activity",
                                    "Could not persist sandbox activity session={} error={error}",
                                    entry.session_id
                                );
                            }
                        }
                        Job::Clear(session_id, done) => {
                            if let Some(directory) = resolve(&session_id) {
                                // Usually already gone with the whole folder.
                                let _ = std::fs::remove_file(directory.join(ACTIVITY_FILENAME));
                            }
                            let _ = done.send(());
                        }
                        Job::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("spawn activity writer");

        Self {
            sessions: Mutex::new(Vec::new()),
            resolver,
            sink,
            writer: Mutex::new(tx),
        }
    }

    /// `setSandboxDirectoryResolver`.
    pub fn set_directory_resolver(&self, resolver: DirectoryResolver) {
        *self.resolver.write().unwrap() = resolver;
    }

    fn resolve(&self, session_id: &str) -> Option<PathBuf> {
        let resolver = self.resolver.read().unwrap().clone();
        resolver(session_id)
    }

    fn enqueue(&self, job: Job) {
        let _ = self.writer.lock().unwrap().send(job);
    }

    fn publish(&self, event: ActivityEvent) {
        let channel = activity_channel(&event.entry().session_id);
        self.sink.publish(&channel, SandboxEvent::Activity(event));
    }

    /// Mutate an entry in place, then publish and persist a snapshot of it.
    fn update(
        &self,
        session_id: &str,
        id: &str,
        apply: impl FnOnce(&mut SandboxActivityEntry),
        event: fn(SandboxActivityEntry) -> ActivityEvent,
    ) {
        let snapshot = {
            let mut sessions = self.sessions.lock().unwrap();
            let Some((_, entries)) = sessions.iter_mut().find(|(sid, _)| sid == session_id) else {
                return;
            };
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
                return;
            };
            apply(entry);
            entry.clone()
        };
        self.publish(event(snapshot.clone()));
        self.enqueue(Job::Persist(Box::new(snapshot)));
    }

    /// Record a command as started. Returns the entry id for `record_settled`/`record_exit`.
    pub fn record_start(&self, input: RecordStartInput) -> String {
        let entry = SandboxActivityEntry {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: input.session_id.clone(),
            service: input.service,
            command: truncate_command(&input.command),
            source: input.source.unwrap_or(ActivitySource::Agent),
            outcome: ActivityOutcome::Running,
            started_at: now_iso(),
            ended_at: None,
            duration_ms: None,
            exit_code: None,
            cwd: input.cwd,
            pid: input.pid,
            stdout_bytes: None,
            stderr_bytes: None,
            stdin_bytes: None,
        };

        {
            let mut sessions = self.sessions.lock().unwrap();
            let entries = touch(&mut sessions, &input.session_id);
            entries.push(entry.clone());
            if entries.len() > MEMORY_LIMIT {
                let excess = entries.len() - MEMORY_LIMIT;
                entries.drain(..excess);
            }
        }

        let id = entry.id.clone();
        self.publish(ActivityEvent::Start {
            entry: entry.clone(),
        });
        self.enqueue(Job::Persist(Box::new(entry)));
        id
    }

    /// Record how a command resolved for the caller — distinct from exiting, since an exec
    /// can resolve early and leave its process running.
    pub fn record_settled(&self, input: RecordSettledInput) {
        self.update(
            &input.session_id,
            &input.id,
            |entry| {
                entry.outcome = input.outcome;
                if let Some(code) = input.exit_code {
                    entry.exit_code = Some(code);
                }
                if let Some(duration) = input.duration_ms {
                    entry.duration_ms = Some(duration);
                }
            },
            |entry| ActivityEvent::Settled { entry },
        );
    }

    /// Record the process actually ending. Upgrades the row in place.
    pub fn record_exit(&self, input: RecordExitInput) {
        self.update(
            &input.session_id,
            &input.id,
            |entry| {
                let ended_at = now_iso();
                entry.exit_code = Some(input.exit_code);
                entry.duration_ms = match (parse_iso_ms(&ended_at), parse_iso_ms(&entry.started_at))
                {
                    (Some(end), Some(start)) => Some(end - start),
                    _ => None,
                };
                entry.ended_at = Some(ended_at);
                entry.stdout_bytes = input.stdout_bytes;
                entry.stderr_bytes = input.stderr_bytes;
                // A process that had resolved as detached or requires-input has now really
                // finished; those outcomes are kept.
                if matches!(
                    entry.outcome,
                    ActivityOutcome::Running | ActivityOutcome::Completed
                ) {
                    entry.outcome = if input.exit_code == 0 {
                        ActivityOutcome::Completed
                    } else {
                        ActivityOutcome::Failed
                    };
                }
            },
            |entry| ActivityEvent::Exit { entry },
        );
    }

    /// Record stdin sent to a waiting command. Length only, never the text.
    pub fn record_stdin(&self, session_id: &str, id: &str, bytes: usize) {
        self.update(
            session_id,
            id,
            |entry| entry.stdin_bytes = Some(entry.stdin_bytes.unwrap_or(0) + bytes),
            |entry| ActivityEvent::Settled { entry },
        );
    }

    /// Record something the user did rather than the agent. Terminal sessions are logged
    /// as single open/close events on purpose.
    pub fn record_user_event(
        &self,
        session_id: &str,
        service: &str,
        description: &str,
        outcome: ActivityOutcome,
        exit_code: Option<i32>,
    ) -> String {
        let id = self.record_start(RecordStartInput {
            session_id: session_id.to_string(),
            service: service.to_string(),
            command: description.to_string(),
            source: Some(ActivitySource::User),
            ..Default::default()
        });
        if outcome != ActivityOutcome::Running {
            self.record_settled(RecordSettledInput {
                session_id: session_id.to_string(),
                id: id.clone(),
                outcome,
                exit_code,
                duration_ms: None,
            });
        }
        id
    }

    /// Entries for a session, newest last. Reads the persisted file when nothing is in
    /// memory yet, which is what makes the list survive an app restart.
    pub async fn get_activity(&self, session_id: &str) -> Vec<SandboxActivityEntry> {
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some((_, entries)) = sessions.iter().find(|(sid, _)| sid == session_id) {
                if !entries.is_empty() {
                    return entries.clone();
                }
            }
        }

        let Some(directory) = self.resolve(session_id) else {
            return Vec::new();
        };
        let Ok(contents) = tokio::fs::read_to_string(directory.join(ACTIVITY_FILENAME)).await
        else {
            return Vec::new();
        };

        let mut parsed: Vec<SandboxActivityEntry> = Vec::new();
        let mut seen: std::collections::HashMap<String, usize> = Default::default();
        for line in contents.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            // A half-written final line is expected after a crash. Skip it.
            let Ok(entry) = serde_json::from_str::<SandboxActivityEntry>(line) else {
                continue;
            };
            // Each entry is appended again as it progresses; keep the latest version.
            match seen.get(&entry.id) {
                Some(&at) => parsed[at] = entry,
                None => {
                    seen.insert(entry.id.clone(), parsed.len());
                    parsed.push(entry);
                }
            }
        }

        let start = parsed.len().saturating_sub(MEMORY_LIMIT);
        let recent = parsed.split_off(start);
        let mut sessions = self.sessions.lock().unwrap();
        *touch(&mut sessions, session_id) = recent.clone();
        recent
    }

    /// Forget a session's activity, in memory and on disk. Used when a sandbox is removed.
    pub async fn clear_activity(&self, session_id: &str) {
        self.sessions
            .lock()
            .unwrap()
            .retain(|(sid, _)| sid != session_id);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue(Job::Clear(session_id.to_string(), tx));
        let _ = rx.await;
    }

    /// Wait for queued writes to settle (`flushActivityWrites`).
    pub async fn flush(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue(Job::Flush(tx));
        let _ = rx.await;
    }
}

fn touch<'a>(
    sessions: &'a mut Vec<(String, Vec<SandboxActivityEntry>)>,
    session_id: &str,
) -> &'a mut Vec<SandboxActivityEntry> {
    if let Some(index) = sessions.iter().position(|(sid, _)| sid == session_id) {
        // Move this session to the back of the eviction order.
        let existing = sessions.remove(index);
        sessions.push(existing);
    } else {
        sessions.push((session_id.to_string(), Vec::new()));
        while sessions.len() > SESSION_LIMIT {
            sessions.remove(0);
        }
    }
    &mut sessions.last_mut().expect("just pushed").1
}

fn truncate_command(command: &str) -> String {
    if command.chars().count() > COMMAND_LIMIT {
        let mut truncated: String = command.chars().take(COMMAND_LIMIT).collect();
        truncated.push('…');
        truncated
    } else {
        command.to_string()
    }
}

fn persist(resolve: &DirectoryResolver, entry: &SandboxActivityEntry) -> std::io::Result<()> {
    // Resolved on every append: renaming a chat moves the folder while containers run.
    let Some(directory) = resolve(&entry.session_id) else {
        return Ok(());
    };
    let file = directory.join(ACTIVITY_FILENAME);
    append_line(&file, &serde_json::to_string(entry)?)?;

    let contents = std::fs::read_to_string(&file)?;
    let lines: Vec<&str> = contents.split('\n').filter(|l| !l.is_empty()).collect();
    if lines.len() > FILE_LINE_LIMIT {
        let kept = &lines[lines.len() / 2..];
        std::fs::write(&file, format!("{}\n", kept.join("\n")))?;
    }
    Ok(())
}

fn append_line(file: &Path, line: &str) -> std::io::Result<()> {
    let mut handle = OpenOptions::new().create(true).append(true).open(file)?;
    handle.write_all(format!("{line}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    //! Port of `sandboxActivity.test.ts`.
    use super::*;
    use crate::events::RecordingSink;

    struct Harness {
        root: tempfile::TempDir,
        directory: PathBuf,
        sink: Arc<RecordingSink>,
        log: ActivityLog,
    }

    fn harness() -> Harness {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("my-chat-7f3a2c");
        std::fs::create_dir(&directory).unwrap();
        let sink = Arc::new(RecordingSink::default());
        let log = ActivityLog::new(sink.clone());
        let fixed = directory.clone();
        log.set_directory_resolver(Arc::new(move |_| Some(fixed.clone())));
        Harness {
            root,
            directory,
            sink,
            log,
        }
    }

    fn read_lines(dir: &Path) -> Vec<SandboxActivityEntry> {
        let file = dir.join(ACTIVITY_FILENAME);
        let Ok(contents) = std::fs::read_to_string(file) else {
            return Vec::new();
        };
        contents
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn start(log: &ActivityLog, session: &str, command: &str) -> String {
        log.record_start(RecordStartInput {
            session_id: session.into(),
            service: "main".into(),
            command: command.into(),
            ..Default::default()
        })
    }

    fn settled(
        session: &str,
        id: &str,
        outcome: ActivityOutcome,
        exit_code: Option<i32>,
    ) -> RecordSettledInput {
        RecordSettledInput {
            session_id: session.into(),
            id: id.into(),
            outcome,
            exit_code,
            duration_ms: None,
        }
    }

    fn exit(session: &str, id: &str, exit_code: i32) -> RecordExitInput {
        RecordExitInput {
            session_id: session.into(),
            id: id.into(),
            exit_code,
            ..Default::default()
        }
    }

    // describe('the three-event model')
    #[tokio::test]
    async fn records_a_start_then_how_it_settled_then_the_real_exit() {
        let h = harness();
        let id = h.log.record_start(RecordStartInput {
            session_id: "session_1".into(),
            service: "main".into(),
            command: "npm ci".into(),
            cwd: Some("/workspace".into()),
            pid: Some(8821),
            ..Default::default()
        });
        h.log.record_settled(settled(
            "session_1",
            &id,
            ActivityOutcome::Completed,
            Some(0),
        ));
        h.log.record_exit(RecordExitInput {
            stdout_bytes: Some(6144),
            stderr_bytes: Some(0),
            ..exit("session_1", &id, 0)
        });

        let entries = h.log.get_activity("session_1").await;
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.command, "npm ci");
        assert_eq!(entry.source, ActivitySource::Agent);
        assert_eq!(entry.outcome, ActivityOutcome::Completed);
        assert_eq!(entry.exit_code, Some(0));
        assert_eq!(entry.pid, Some(8821));
        assert_eq!(entry.stdout_bytes, Some(6144));
        assert!(entry.duration_ms.unwrap() >= 0);
        h.log.flush().await;
    }

    #[tokio::test]
    async fn keeps_a_detached_process_from_reading_as_finished() {
        let h = harness();
        let id = start(&h.log, "session_1", "npm run dev");
        h.log
            .record_settled(settled("session_1", &id, ActivityOutcome::Detached, None));

        let entry = &h.log.get_activity("session_1").await[0];
        assert_eq!(entry.outcome, ActivityOutcome::Detached);
        assert_eq!(entry.exit_code, None);
        assert_eq!(entry.ended_at, None);
        h.log.flush().await;
    }

    #[tokio::test]
    async fn marks_a_non_zero_exit_as_failed() {
        let h = harness();
        let id = start(&h.log, "session_1", "npx jest");
        h.log.record_exit(exit("session_1", &id, 1));
        assert_eq!(
            h.log.get_activity("session_1").await[0].outcome,
            ActivityOutcome::Failed
        );
        h.log.flush().await;
    }

    #[tokio::test]
    async fn does_not_overwrite_a_requires_input_outcome_when_the_process_later_exits() {
        let h = harness();
        let id = start(&h.log, "session_1", "apt-get install x");
        h.log.record_settled(settled(
            "session_1",
            &id,
            ActivityOutcome::RequiresInput,
            None,
        ));
        h.log.record_exit(exit("session_1", &id, 0));

        let entry = &h.log.get_activity("session_1").await[0];
        assert_eq!(entry.outcome, ActivityOutcome::RequiresInput);
        assert_eq!(entry.exit_code, Some(0));
        h.log.flush().await;
    }

    #[tokio::test]
    async fn ignores_updates_for_an_unknown_id_rather_than_inventing_a_row() {
        let h = harness();
        h.log.record_settled(settled(
            "session_1",
            "nope",
            ActivityOutcome::Completed,
            None,
        ));
        assert!(h.log.get_activity("session_1").await.is_empty());
    }

    // describe('publishing')
    #[tokio::test]
    async fn announces_each_transition_on_the_session_channel() {
        let h = harness();
        let id = start(&h.log, "session_1", "ls");
        h.log.record_settled(settled(
            "session_1",
            &id,
            ActivityOutcome::Completed,
            Some(0),
        ));
        h.log.record_exit(exit("session_1", &id, 0));

        let types: Vec<&str> = h
            .sink
            .on_channel(&activity_channel("session_1"))
            .iter()
            .map(|event| match event {
                SandboxEvent::Activity(activity) => activity.kind(),
                _ => "other",
            })
            .collect();
        assert_eq!(types, ["start", "settled", "exit"]);
        h.log.flush().await;
    }

    // describe('stdin')
    #[tokio::test]
    async fn records_how_many_bytes_were_sent_and_never_the_text() {
        let h = harness();
        let id = start(&h.log, "session_1", "psql");
        h.log.record_stdin("session_1", &id, "hunter2\n".len());

        let entry = &h.log.get_activity("session_1").await[0];
        assert_eq!(entry.stdin_bytes, Some(8));
        assert!(!serde_json::to_string(entry).unwrap().contains("hunter2"));
        h.log.flush().await;
    }

    #[tokio::test]
    async fn accumulates_across_several_follow_ups() {
        let h = harness();
        let id = start(&h.log, "session_1", "psql");
        h.log.record_stdin("session_1", &id, 3);
        h.log.record_stdin("session_1", &id, 4);
        assert_eq!(
            h.log.get_activity("session_1").await[0].stdin_bytes,
            Some(7)
        );
        h.log.flush().await;
    }

    // describe('user events')
    #[tokio::test]
    async fn distinguishes_what_the_user_did_from_what_the_agent_did() {
        let h = harness();
        start(&h.log, "session_1", "npm ci");
        h.log.record_user_event(
            "session_1",
            "main",
            "Interactive terminal opened",
            ActivityOutcome::Running,
            None,
        );
        let sources: Vec<ActivitySource> = h
            .log
            .get_activity("session_1")
            .await
            .iter()
            .map(|e| e.source)
            .collect();
        assert_eq!(sources, [ActivitySource::Agent, ActivitySource::User]);
        h.log.flush().await;
    }

    // describe('persistence')
    #[tokio::test]
    async fn writes_entries_to_activity_jsonl_in_the_sandbox_folder() {
        let h = harness();
        let id = start(&h.log, "session_1", "npm ci");
        h.log.record_exit(exit("session_1", &id, 0));
        h.log.flush().await;

        let lines = read_lines(&h.directory);
        // Appended once per transition; the reader keeps the latest version of each id.
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].id, id);
        assert_eq!(lines[1].exit_code, Some(0));
    }

    #[tokio::test]
    async fn survives_a_restart_by_reading_the_file_back() {
        let h = harness();
        let id = start(&h.log, "session_1", "npm run build");
        h.log.record_settled(settled(
            "session_1",
            &id,
            ActivityOutcome::Completed,
            Some(0),
        ));
        h.log.flush().await;

        // A fresh process: no memory, same folder.
        let fresh = ActivityLog::new(Arc::new(RecordingSink::default()));
        let dir = h.directory.clone();
        fresh.set_directory_resolver(Arc::new(move |_| Some(dir.clone())));

        let entries = fresh.get_activity("session_1").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].command, "npm run build");
        assert_eq!(entries[0].outcome, ActivityOutcome::Completed);
    }

    #[tokio::test]
    async fn skips_a_half_written_final_line_left_by_a_crash() {
        let h = harness();
        let id = start(&h.log, "session_1", "ok");
        h.log.flush().await;
        append_line_raw(&h.directory.join(ACTIVITY_FILENAME), "{\"id\":\"trunc\"");

        let fresh = ActivityLog::new(Arc::new(RecordingSink::default()));
        let dir = h.directory.clone();
        fresh.set_directory_resolver(Arc::new(move |_| Some(dir.clone())));

        let ids: Vec<String> = fresh
            .get_activity("session_1")
            .await
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, [id]);
    }

    fn append_line_raw(file: &Path, text: &str) {
        let mut handle = OpenOptions::new().append(true).open(file).unwrap();
        handle.write_all(text.as_bytes()).unwrap();
    }

    #[tokio::test]
    async fn rewrites_the_file_without_its_oldest_half_once_it_grows_past_the_cap() {
        let h = harness();
        for i in 0..520 {
            start(&h.log, "session_1", &format!("echo {i}"));
        }
        h.log.flush().await;

        let lines = read_lines(&h.directory);
        assert!(lines.len() <= 500);
        // The newest entries are the ones kept.
        assert_eq!(lines.last().unwrap().command, "echo 519");
    }

    #[tokio::test]
    async fn re_resolves_the_folder_on_every_append() {
        let h = harness();
        start(&h.log, "session_1", "before rename");
        h.log.flush().await;

        let renamed = h.root.path().join("renamed-chat-7f3a2c");
        std::fs::create_dir(&renamed).unwrap();
        let target = renamed.clone();
        h.log
            .set_directory_resolver(Arc::new(move |_| Some(target.clone())));

        start(&h.log, "session_1", "after rename");
        h.log.flush().await;

        let commands: Vec<String> = read_lines(&renamed)
            .into_iter()
            .map(|e| e.command)
            .collect();
        assert_eq!(commands, ["after rename"]);
    }

    #[tokio::test]
    async fn does_not_turn_a_failed_write_into_an_error_for_the_caller() {
        let h = harness();
        let missing = h.root.path().join("does-not-exist");
        h.log
            .set_directory_resolver(Arc::new(move |_| Some(missing.clone())));
        start(&h.log, "session_1", "ls");
        h.log.flush().await;
    }

    #[tokio::test]
    async fn records_nothing_to_disk_when_the_sandbox_folder_cannot_be_resolved() {
        let h = harness();
        h.log.set_directory_resolver(Arc::new(|_| None));
        start(&h.log, "session_1", "ls");
        h.log.flush().await;

        assert!(read_lines(&h.directory).is_empty());
        // Still in memory for the UI, though.
        assert_eq!(h.log.get_activity("session_1").await.len(), 1);
    }

    // describe('caps')
    #[tokio::test]
    async fn keeps_only_the_most_recent_entries_per_session_in_memory() {
        let h = harness();
        for i in 0..260 {
            start(&h.log, "session_1", &format!("cmd {i}"));
        }
        let entries = h.log.get_activity("session_1").await;
        assert_eq!(entries.len(), 200);
        assert_eq!(entries[0].command, "cmd 60");
        assert_eq!(entries[199].command, "cmd 259");
        h.log.flush().await;
    }

    #[tokio::test]
    async fn forgets_the_least_recently_touched_session_past_the_session_cap() {
        let h = harness();
        h.log.set_directory_resolver(Arc::new(|_| None));
        for i in 0..21 {
            start(&h.log, &format!("session_{i}"), "ls");
        }
        assert!(h.log.get_activity("session_0").await.is_empty());
        assert_eq!(h.log.get_activity("session_20").await.len(), 1);
    }

    #[tokio::test]
    async fn truncates_an_enormous_generated_command() {
        let h = harness();
        start(&h.log, "session_1", &"x".repeat(5000));
        let entry = &h.log.get_activity("session_1").await[0];
        assert!(entry.command.chars().count() < 5000);
        assert!(entry.command.ends_with('…'));
        h.log.flush().await;
    }

    // describe('clearActivity')
    #[tokio::test]
    async fn removes_the_log_from_memory_and_from_disk() {
        let h = harness();
        start(&h.log, "session_1", "ls");
        h.log.flush().await;
        assert_eq!(read_lines(&h.directory).len(), 1);

        h.log.clear_activity("session_1").await;

        assert!(!h.directory.join(ACTIVITY_FILENAME).exists());
        assert!(h.log.get_activity("session_1").await.is_empty());
    }
}
