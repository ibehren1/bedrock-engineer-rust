//! Crate tests. The TS app has no tests for the background agent (COVERAGE_PARITY lists none);
//! these cover persistence against Electron-format fixtures, pub/sub, notifications, the
//! scheduler, and scripted end-to-end runs through the real `agents::AgentEngine`.

use crate::*;
use agents::{AgentEngine, ConverseBackend, SessionMeta, SessionStore, StoreReader};
use async_trait::async_trait;
use bedrock::ConverseRequest;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;
use tools::{Tool, ToolCategory, ToolContext, ToolOutput, ToolRegistry, ToolSpec};

// ---- Electron fixtures -----------------------------------------------------------------------

/// `JSON.stringify(session, null, 2)` of a manual session written by the Electron app.
const MANUAL_SESSION: &str = r#"{
  "sessionId": "manual-1",
  "agentId": "agent-a",
  "modelId": "anthropic.claude",
  "projectDirectory": "/proj",
  "createdAt": 1759200000000,
  "updatedAt": 1759200005000,
  "messages": [
    {
      "id": "m1",
      "role": "user",
      "content": [
        {
          "text": "Hello\n<context>\nDate: Tue Sep 30 2025 10:00:00 GMT+0900 (Japan Standard Time)\n<context>\n"
        }
      ],
      "timestamp": 1759200001000
    },
    {
      "id": "m2",
      "role": "assistant",
      "content": [
        {
          "toolUse": {
            "toolUseId": "tu-1",
            "name": "readFiles",
            "input": {
              "paths": [
                "/proj/a.txt"
              ]
            }
          }
        }
      ],
      "timestamp": 1759200002000
    },
    {
      "id": "m3",
      "role": "user",
      "content": [
        {
          "toolResult": {
            "toolUseId": "tu-1",
            "content": [
              {
                "text": "\"file body\""
              }
            ],
            "status": "success"
          }
        }
      ],
      "timestamp": 1759200003000
    },
    {
      "id": "m4",
      "role": "assistant",
      "content": [
        {
          "text": "Done."
        }
      ],
      "timestamp": 1759200004000,
      "metadata": {
        "converseMetadata": {
          "usage": {
            "inputTokens": 10,
            "outputTokens": 2,
            "totalTokens": 12
          },
          "stopReason": "end_turn"
        }
      }
    }
  ]
}"#;

const SCHEDULED_SESSION: &str = r#"{
  "sessionId": "scheduled-task-1-0f0e",
  "taskId": "task-1",
  "agentId": "agent-a",
  "modelId": "anthropic.claude",
  "createdAt": 1759100000000,
  "updatedAt": 1759300000000,
  "messages": [],
  "executionMetadata": {
    "executedAt": 1759300000000,
    "success": true
  }
}"#;

/// electron-store (`JSON.stringify(data, undefined, '\t')`).
const META_STORE: &str = "{\n\t\"metadata\": {\n\t\t\"manual-1\": {\n\t\t\t\"sessionId\": \"manual-1\",\n\t\t\t\"agentId\": \"agent-a\",\n\t\t\t\"modelId\": \"anthropic.claude\",\n\t\t\t\"projectDirectory\": \"/proj\",\n\t\t\t\"createdAt\": 1759200000000,\n\t\t\t\"updatedAt\": 1759200005000,\n\t\t\t\"messageCount\": 4,\n\t\t\t\"executionType\": \"manual\"\n\t\t},\n\t\t\"scheduled-task-1-0f0e\": {\n\t\t\t\"sessionId\": \"scheduled-task-1-0f0e\",\n\t\t\t\"taskId\": \"task-1\",\n\t\t\t\"agentId\": \"agent-a\",\n\t\t\t\"modelId\": \"anthropic.claude\",\n\t\t\t\"createdAt\": 1759100000000,\n\t\t\t\"updatedAt\": 1759300000000,\n\t\t\t\"messageCount\": 0,\n\t\t\t\"executionType\": \"scheduled\"\n\t\t},\n\t\t\"gone\": {\n\t\t\t\"sessionId\": \"gone\",\n\t\t\t\"agentId\": \"agent-a\",\n\t\t\t\"modelId\": \"m\",\n\t\t\t\"createdAt\": 1,\n\t\t\t\"updatedAt\": 2,\n\t\t\t\"messageCount\": 0,\n\t\t\t\"executionType\": \"manual\"\n\t\t}\n\t}\n}";

const HISTORY_STORE: &str = "{\n\t\"executionHistory\": {\n\t\t\"task-1\": [\n\t\t\t{\n\t\t\t\t\"taskId\": \"task-1\",\n\t\t\t\t\"executedAt\": 1759300000000,\n\t\t\t\t\"status\": \"success\",\n\t\t\t\t\"sessionId\": \"scheduled-task-1-0f0e\",\n\t\t\t\t\"messageCount\": 0\n\t\t\t},\n\t\t\t{\n\t\t\t\t\"taskId\": \"task-1\",\n\t\t\t\t\"executedAt\": 1759300100000,\n\t\t\t\t\"status\": \"failed\",\n\t\t\t\t\"error\": \"Agent not found: agent-a\",\n\t\t\t\t\"sessionId\": \"scheduled-task-1-aaaa\",\n\t\t\t\t\"messageCount\": 0\n\t\t\t}\n\t\t]\n\t}\n}";

/// `backgroundAgentScheduledTasks` as Electron left it after a successful scheduled run
/// (`persistTasks` ran while `isExecuting` was still true).
fn electron_tasks() -> Value {
    json!([
        {
            "id": "task-1",
            "name": "Morning report",
            "cronExpression": "0 9 * * 1-5",
            "agentId": "agent-a",
            "modelId": "anthropic.claude",
            "projectDirectory": "/proj",
            "wakeWord": "Write the report",
            "enabled": true,
            "createdAt": 1759000000000_i64,
            "nextRun": 1759300000000_i64,
            "runCount": 3,
            "inferenceConfig": { "maxTokens": 4096, "temperature": 0.5 },
            "continueSession": false,
            "lastRun": 1759300000000_i64,
            "isExecuting": true,
            "lastExecutionStarted": 1759299990000_i64,
            "futureField": { "kept": true }
        },
        {
            "id": "task-2",
            "name": "Disabled",
            "cronExpression": "*/5 * * * *",
            "agentId": "agent-a",
            "modelId": "anthropic.claude",
            "wakeWord": "ping",
            "enabled": false,
            "createdAt": 1759000000000_i64,
            "runCount": 0,
            "lastError": "boom"
        }
    ])
}

fn write_fixtures(dir: &Path) {
    let sessions = dir.join("background-agent-sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(sessions.join("manual-1.json"), MANUAL_SESSION).unwrap();
    fs::write(
        sessions.join("scheduled-task-1-0f0e.json"),
        SCHEDULED_SESSION,
    )
    .unwrap();
    fs::write(dir.join("background-agent-sessions-meta.json"), META_STORE).unwrap();
    fs::write(
        dir.join("background-agent-execution-history.json"),
        HISTORY_STORE,
    )
    .unwrap();
}

// ---- test doubles ----------------------------------------------------------------------------

#[derive(Default)]
struct Scripted {
    responses: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<ConverseRequest>>,
    delay: Option<Duration>,
}

impl Scripted {
    fn new(responses: Vec<Value>) -> Arc<Self> {
        Arc::new(Scripted {
            responses: Mutex::new(responses.into()),
            ..Default::default()
        })
    }
    fn with_delay(responses: Vec<Value>, delay: Duration) -> Arc<Self> {
        Arc::new(Scripted {
            responses: Mutex::new(responses.into()),
            delay: Some(delay),
            ..Default::default()
        })
    }
}

#[async_trait]
impl ConverseBackend for Scripted {
    async fn converse(&self, request: ConverseRequest) -> std::result::Result<Value, String> {
        self.requests.lock().unwrap().push(request);
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "no scripted response left".to_string())
    }
}

fn text_response(text: &str) -> Value {
    json!({
        "output": { "message": { "role": "assistant", "content": [{ "text": text }] } },
        "stopReason": "end_turn",
        "usage": { "inputTokens": 10, "outputTokens": 2, "totalTokens": 12 }
    })
}

fn tool_response(id: &str, name: &str, input: Value) -> Value {
    json!({
        "output": { "message": { "role": "assistant", "content": [
            { "text": "Reading" },
            { "toolUse": { "toolUseId": id, "name": name, "input": input } }
        ] } },
        "stopReason": "tool_use"
    })
}

struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "readFiles"
    }
    fn description(&self) -> &str {
        "reads"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Filesystem
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            "readFiles",
            "Reads files",
            json!({"type": "object"}),
        ))
    }
    fn validate_input(&self, _input: &Value) -> Vec<String> {
        vec![]
    }
    async fn execute_internal(
        &self,
        _input: Value,
        _ctx: &ToolContext,
    ) -> tools::Result<ToolOutput> {
        Ok(ToolOutput::Json(json!({
            "success": true, "name": "readFiles", "message": "ok", "result": "file body"
        })))
    }
}

#[derive(Default)]
struct Sink {
    events: Mutex<Vec<(Option<String>, String, Value)>>,
    failing_target: Option<String>,
}

impl Sink {
    fn named(&self, name: &str) -> Vec<Value> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e, _)| e == name)
            .map(|(_, _, p)| p.clone())
            .collect()
    }
}

impl EventSink for Sink {
    fn emit(&self, event: &str, payload: Value) {
        self.events
            .lock()
            .unwrap()
            .push((None, event.to_string(), payload));
    }
    fn emit_to(
        &self,
        target: &str,
        event: &str,
        payload: Value,
    ) -> std::result::Result<(), String> {
        if self.failing_target.as_deref() == Some(target) {
            return Err("window closed".into());
        }
        self.events
            .lock()
            .unwrap()
            .push((Some(target.to_string()), event.to_string(), payload));
        Ok(())
    }
}

#[derive(Default)]
struct RecordingNotifier {
    focused: bool,
    shown: Mutex<Vec<TaskOsNotification>>,
}

impl Notifier for RecordingNotifier {
    fn is_app_focused(&self) -> bool {
        self.focused
    }
    fn show(&self, n: TaskOsNotification) {
        self.shown.lock().unwrap().push(n);
    }
}

fn agent_json(id: &str) -> Value {
    json!({
        "id": id, "name": format!("Agent {id}"), "description": "does things",
        "system": "You are a reporter in {{projectPath}}.", "scenarios": [],
        "tools": ["readFiles"]
    })
}

struct Fixture {
    _dir: tempfile::TempDir,
    dir: std::path::PathBuf,
    bg: BackgroundAgents,
    config: Arc<MemoryConfigStore>,
    sink: Arc<Sink>,
    notifier: Arc<RecordingNotifier>,
    converse: Arc<Scripted>,
}

fn fixture_with(converse: Arc<Scripted>, config: Value, fixtures: bool) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    if fixtures {
        write_fixtures(&dir);
    }
    let mut cfg = json!({ "projectPath": "/proj", "customAgents": [agent_json("agent-a")] });
    if let (Some(o), Some(extra)) = (cfg.as_object_mut(), config.as_object()) {
        o.extend(extra.clone());
    }
    let config = Arc::new(MemoryConfigStore::new(cfg));
    let sink = Arc::new(Sink::default());
    let notifier = Arc::new(RecordingNotifier::default());
    let storage = BackgroundStorage::open(&dir, &dir, sink.clone()).unwrap();
    let reader_cfg = config.clone();
    let reader: Arc<dyn StoreReader> = Arc::new(move || reader_cfg.all());
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadTool));
    let engine = Arc::new(
        AgentEngine::new(converse.clone(), Arc::new(registry), reader)
            .with_sessions(storage.sessions.clone())
            .with_listener(storage.listener.clone()),
    );
    let bg = BackgroundAgents::new(BackgroundDeps {
        engine,
        storage,
        config: config.clone(),
        events: sink.clone(),
        notifier: notifier.clone(),
        runtime: Handle::current(),
    });
    Fixture {
        _dir: tmp,
        dir,
        bg,
        config,
        sink,
        notifier,
        converse,
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

// ---- session persistence ---------------------------------------------------------------------

#[test]
fn loads_electron_sessions_and_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();

    assert_eq!(m.list_sessions(), vec!["manual-1", "scheduled-task-1-0f0e"]);
    let history = m.history_values("manual-1");
    assert_eq!(history.len(), 4);
    assert_eq!(history[1]["content"][0]["toolUse"]["name"], "readFiles");
    let engine_view = SessionStore::history(&m, "manual-1");
    assert_eq!(
        engine_view[3].metadata.as_ref().unwrap()["converseMetadata"]["stopReason"],
        "end_turn"
    );

    // Only sessions whose file exists, newest first.
    let all = m.get_all_sessions_metadata();
    let ids: Vec<_> = all.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(ids, vec!["scheduled-task-1-0f0e", "manual-1"]);
    assert_eq!(all[0].execution_type, "scheduled");
    assert_eq!(all[0].task_id.as_deref(), Some("task-1"));

    let stats = m.get_session_stats("manual-1");
    assert!(stats.exists);
    assert_eq!(
        (
            stats.message_count,
            stats.user_messages,
            stats.assistant_messages
        ),
        (4, 2, 2)
    );
    assert_eq!(
        stats.metadata.unwrap().project_directory.as_deref(),
        Some("/proj")
    );
    assert_eq!(
        serde_json::to_value(m.get_session_stats("nope")).unwrap(),
        json!({ "exists": false, "messageCount": 0, "userMessages": 0, "assistantMessages": 0 })
    );
    assert_eq!(m.get_sessions_by_project_directory("/proj").len(), 1);
    assert_eq!(m.get_sessions_by_agent_id("agent-a").len(), 2);
    assert_eq!(m.get_sessions_by_task_id("task-1").len(), 1);
    let totals = m.get_all_session_stats();
    assert_eq!((totals.total_sessions, totals.total_messages), (2, 4));
    assert_eq!(totals.average_messages_per_session, 2.0);
}

#[test]
fn appending_keeps_electron_file_format() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    let msg = agents::AgentMessage::new("user", vec![json!({ "text": "again" })]);
    SessionStore::add_message(&m, "manual-1", &msg).unwrap();

    let path = tmp.path().join("background-agent-sessions/manual-1.json");
    let raw = fs::read_to_string(&path).unwrap();
    assert!(
        raw.starts_with("{\n  \"sessionId\": \"manual-1\",\n  \"agentId\""),
        "{raw}"
    );
    let v: Value = serde_json::from_str(&raw).unwrap();
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "sessionId",
            "agentId",
            "modelId",
            "projectDirectory",
            "createdAt",
            "updatedAt",
            "messages"
        ]
    );
    assert_eq!(v["messages"].as_array().unwrap().len(), 5);
    let last = &v["messages"][4];
    let msg_keys: Vec<_> = last.as_object().unwrap().keys().cloned().collect();
    assert_eq!(msg_keys, ["id", "role", "content", "timestamp"]);
    assert!(v["updatedAt"].as_i64().unwrap() > 1759200005000);
    // Earlier messages are untouched.
    assert_eq!(
        v["messages"][3],
        read_json_str(MANUAL_SESSION)["messages"][3]
    );

    // Metadata store: tab-indented electron-store file, count updated.
    let meta_raw =
        fs::read_to_string(tmp.path().join("background-agent-sessions-meta.json")).unwrap();
    assert!(
        meta_raw.starts_with("{\n\t\"metadata\": {\n\t\t\"manual-1\": {"),
        "{meta_raw}"
    );
    let meta = read_json(&tmp.path().join("background-agent-sessions-meta.json"));
    assert_eq!(meta["metadata"]["manual-1"]["messageCount"], 5);
    assert_eq!(meta["metadata"]["manual-1"]["executionType"], "manual");
    // No temp files left behind.
    let leftovers: Vec<_> = fs::read_dir(tmp.path().join("background-agent-sessions"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty());
}

fn read_json_str(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn create_add_delete_and_metadata_rebuild() {
    let tmp = tempfile::tempdir().unwrap();
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    m.create_session(
        "s1",
        SessionMeta {
            task_id: Some("t".into()),
            agent_id: Some("a".into()),
            model_id: Some("m".into()),
            project_directory: None,
        },
    )
    .unwrap();
    let file = read_json(&tmp.path().join("background-agent-sessions/s1.json"));
    let keys: Vec<_> = file.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "sessionId",
            "taskId",
            "agentId",
            "modelId",
            "createdAt",
            "updatedAt",
            "messages"
        ]
    );
    // Creating again is a no-op.
    m.create_session("s1", SessionMeta::default()).unwrap();
    assert_eq!(
        read_json(&tmp.path().join("background-agent-sessions/s1.json"))["taskId"],
        "t"
    );
    // addMessage creates a missing session.
    m.add_message_value(
        "s2",
        json!({ "id": "x", "role": "user", "content": [], "timestamp": 1 }),
    )
    .unwrap();
    assert_eq!(m.get_session_metadata("s2").unwrap().agent_id, "");
    assert_eq!(m.get_session_metadata("s2").unwrap().message_count, 1);
    drop(m);

    // An empty metadata store is rebuilt from the session files.
    fs::remove_file(tmp.path().join("background-agent-sessions-meta.json")).unwrap();
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    assert_eq!(m.get_all_sessions_metadata().len(), 2);
    assert_eq!(
        m.get_session_metadata("s1").unwrap().execution_type,
        "scheduled"
    );
    assert!(m.delete_session("s1"));
    assert!(!m.has_session("s1"));
    assert!(m.get_session_metadata("s1").is_none());
    assert_eq!(m.list_sessions(), vec!["s2"]);
    // cleanupOldSessions: s2 is fresh, nothing removed; with a zero max age it goes.
    assert_eq!(m.cleanup_old_sessions(None), 0);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(m.cleanup_old_sessions(Some(Duration::from_millis(1))), 1);
}

fn corrupt_copies(dir: &Path, id: &str) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("{id}.json.corrupt-"))
        })
        .collect()
}

#[test]
fn corrupted_session_files_are_moved_aside_not_deleted() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let dir = tmp.path().join("background-agent-sessions");
    fs::write(dir.join("empty.json"), "  \n").unwrap();
    fs::write(dir.join("broken.json"), "{ not json").unwrap();
    fs::write(
        dir.join("shape.json"),
        r#"{"sessionId": "shape", "messages": {}}"#,
    )
    .unwrap();
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    for id in ["empty", "broken", "shape"] {
        assert!(!m.has_session(id));
        assert!(
            !dir.join(format!("{id}.json")).exists(),
            "{id} should be moved aside"
        );
        assert_eq!(corrupt_copies(&dir, id).len(), 1, "{id} kept as .corrupt-*");
        assert!(m.history_values(id).is_empty());
    }
    assert_eq!(
        fs::read_to_string(&corrupt_copies(&dir, "broken")[0]).unwrap(),
        "{ not json"
    );
    assert!(m.has_valid_session("manual-1"));
    assert!(!m.list_sessions().iter().any(|s| s.contains("corrupt")));
}

#[test]
fn lone_surrogate_session_files_load() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let dir = tmp.path().join("background-agent-sessions");
    fs::write(
        dir.join("sur.json"),
        r#"{"sessionId":"sur","messages":[{"id":"1","role":"user","content":[{"text":"cut \ud83d"}],"timestamp":1}]}"#,
    )
    .unwrap();
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    assert!(m.has_session("sur"));
    assert_eq!(
        m.history_values("sur")[0]["content"][0]["text"],
        json!("cut \u{fffd}")
    );
    assert!(corrupt_copies(&dir, "sur").is_empty());
}

#[test]
fn unsafe_session_ids_touch_no_files() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let outside = tmp.path().join("victim.json");
    fs::write(&outside, r#"{"sessionId":"x","messages":[]}"#).unwrap();
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    assert!(!m.delete_session("../victim"));
    assert!(!m.has_session("../victim"));
    assert!(m
        .add_message_value(
            "../victim",
            json!({"id": "1", "role": "user", "content": []})
        )
        .is_err());
    assert!(outside.exists());
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        r#"{"sessionId":"x","messages":[]}"#
    );
}

#[test]
fn session_metadata_writes_keep_external_changes() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    // Another process adds a metadata entry after we opened.
    let meta_path = tmp.path().join("background-agent-sessions-meta.json");
    let mut v = read_json(&meta_path);
    v["metadata"]["external"] = json!({ "sessionId": "external" });
    v["otherKey"] = json!(1);
    fs::write(&meta_path, serde_json::to_string(&v).unwrap()).unwrap();
    m.create_session("new-one", SessionMeta::default()).unwrap();
    let v = read_json(&meta_path);
    assert_eq!(v["metadata"]["external"]["sessionId"], "external");
    assert_eq!(v["otherKey"], 1);
    assert!(v["metadata"]["new-one"].is_object());
}

#[test]
fn execution_metadata_is_written_into_the_session() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let m = BackgroundChatSessionManager::open(tmp.path(), tmp.path()).unwrap();
    m.update_execution_metadata(
        "manual-1",
        ExecutionMetadata {
            executed_at: 5,
            success: false,
            error: Some("x".into()),
        },
    )
    .unwrap();
    let v = read_json(&tmp.path().join("background-agent-sessions/manual-1.json"));
    assert_eq!(
        v["executionMetadata"],
        json!({ "executedAt": 5, "success": false, "error": "x" })
    );
}

// ---- execution history -----------------------------------------------------------------------

fn entry(session: &str, status: ExecutionStatus) -> TaskExecutionResult {
    TaskExecutionResult {
        task_id: "t".into(),
        executed_at: 1,
        status,
        error: None,
        session_id: session.into(),
        message_count: 0,
        extra: Default::default(),
    }
}

#[test]
fn execution_history_matches_electron_store() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let h = ExecutionHistoryStore::open(tmp.path());
    let loaded = h.get("task-1");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[1].status, ExecutionStatus::Failed);
    assert_eq!(loaded[1].error.as_deref(), Some("Agent not found: agent-a"));

    // Same session id replaces, new ones append; newest 100 kept.
    h.record("t", entry("s0", ExecutionStatus::Running));
    h.record("t", entry("s0", ExecutionStatus::Success));
    assert_eq!(h.get("t").len(), 1);
    assert_eq!(h.get("t")[0].status, ExecutionStatus::Success);
    for i in 1..=105 {
        h.record("t", entry(&format!("s{i}"), ExecutionStatus::Success));
    }
    let all = h.get("t");
    assert_eq!(all.len(), 100);
    assert_eq!(all[0].session_id, "s6");

    // Live message count only touches the latest entry of the same session.
    h.update_message_count("t", "s104", 9);
    assert_eq!(h.get("t").last().unwrap().message_count, 0);
    h.update_message_count("t", "s105", 9);
    assert_eq!(h.get("t").last().unwrap().message_count, 9);

    h.remove("t");
    assert!(h.get("t").is_empty());
    let raw =
        fs::read_to_string(tmp.path().join("background-agent-execution-history.json")).unwrap();
    assert!(
        raw.starts_with("{\n\t\"executionHistory\": {\n\t\t\"task-1\": ["),
        "{raw}"
    );
    let v: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        v["executionHistory"]["task-1"][0],
        json!({ "taskId": "task-1", "executedAt": 1759300000000_i64, "status": "success",
                "sessionId": "scheduled-task-1-0f0e", "messageCount": 0 })
    );
}

#[test]
fn execution_history_keeps_entries_it_cannot_read() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("background-agent-execution-history.json");
    let unknown = json!({ "taskId": "t", "status": "cancelled", "sessionId": "old" });
    fs::write(
        &path,
        serde_json::to_string(&json!({ "executionHistory": { "t": [
            unknown.clone(),
            { "taskId": null, "executedAt": 2, "status": "success", "sessionId": "s1", "messageCount": null }
        ] } }))
        .unwrap(),
    )
    .unwrap();
    let h = ExecutionHistoryStore::open(tmp.path());
    // null fields load as defaults; the unknown status is not served but kept.
    let got = h.get("t");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].session_id, "s1");
    assert_eq!(got[0].message_count, 0);
    h.record("t", entry("s2", ExecutionStatus::Success));
    let v = read_json(&path);
    let list = v["executionHistory"]["t"].as_array().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], unknown);

    // Another process adds a task's history meanwhile: kept.
    let mut v = read_json(&path);
    v["executionHistory"]["other"] = json!([]);
    fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
    h.update_message_count("t", "s2", 4);
    let v = read_json(&path);
    assert!(v["executionHistory"]["other"].is_array());
    assert_eq!(v["executionHistory"]["t"][2]["messageCount"], 4);
}

#[test]
fn corrupt_history_file_is_moved_aside() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("background-agent-execution-history.json");
    fs::write(&path, "{ nope").unwrap();
    let h = ExecutionHistoryStore::open(tmp.path());
    assert!(h.get("t").is_empty());
    let kept = corrupt_copies(tmp.path(), "background-agent-execution-history");
    assert_eq!(kept.len(), 1);
    assert_eq!(fs::read_to_string(&kept[0]).unwrap(), "{ nope");
}

// ---- pub/sub & notifications -----------------------------------------------------------------

#[test]
fn pubsub_event_names_match_the_bridge() {
    assert_eq!(
        pubsub_event_name("session-update:abc-123"),
        "pubsub:session-update:abc-123"
    );
    assert_eq!(pubsub_event_name("a.b c/d_e"), "pubsub:a_b_c/d_e");
    assert_eq!(pubsub_event_name("x😀"), "pubsub:x__");
}

#[test]
fn pubsub_routes_to_subscribers_and_tracks_stats() {
    let sink = Arc::new(Sink {
        failing_target: Some("dead".into()),
        ..Default::default()
    });
    let ps = PubSubManager::new(sink.clone());
    ps.publish("c1", json!(0)); // no subscribers: nothing sent
    ps.subscribe("c1", "main");
    ps.subscribe("c1", "main");
    ps.subscribe("c1", "dead");
    ps.subscribe("c2", "main");
    assert_eq!(
        serde_json::to_value(ps.stats()).unwrap(),
        json!({ "totalChannels": 2, "totalSubscribers": 3, "channels": [
            { "channel": "c1", "subscriberCount": 2 }, { "channel": "c2", "subscriberCount": 1 }
        ] })
    );
    ps.publish("c1", json!({ "n": 1 }));
    let sent = sink.events.lock().unwrap().clone();
    assert_eq!(
        sent,
        vec![(Some("main".into()), "pubsub:c1".into(), json!({ "n": 1 }))]
    );
    // The failing subscriber was dropped.
    assert_eq!(ps.stats().total_subscribers, 2);
    ps.unsubscribe("c2", "main");
    assert_eq!(ps.stats().total_channels, 1);
    ps.unsubscribe_all("main");
    assert_eq!(ps.stats().total_channels, 0);
}

#[test]
fn os_notifications_follow_settings_and_focus() {
    let n = RecordingNotifier::default();
    let on = MemoryConfigStore::new(json!({}));
    assert!(show_background_agent_notification(
        &on,
        &n,
        "t",
        "Report",
        true,
        Some("hi"),
        None
    ));
    assert!(show_background_agent_notification(
        &on,
        &n,
        "t",
        "Report",
        true,
        Some(""),
        None
    ));
    assert!(show_background_agent_notification(
        &on,
        &n,
        "t",
        "Report",
        false,
        None,
        Some("bad")
    ));
    let shown = n.shown.lock().unwrap().clone();
    assert_eq!(shown[0].title, "Background Agent Task Completed");
    assert_eq!(shown[0].body, "[Report] hi");
    assert_eq!(shown[1].body, "[Report] Task completed successfully");
    assert_eq!(shown[2].title, "Background Agent Task Failed");
    assert_eq!(shown[2].body, "[Report] bad");
    assert_eq!(shown[2].task_id, "t");

    let off = MemoryConfigStore::new(json!({ "notification": false }));
    assert!(!show_background_agent_notification(
        &off, &n, "t", "R", true, None, None
    ));
    let focused = RecordingNotifier {
        focused: true,
        ..Default::default()
    };
    assert!(!show_background_agent_notification(
        &on, &focused, "t", "R", true, None, None
    ));
}

// ---- scheduler -------------------------------------------------------------------------------

fn schedule_config(name: &str, cron: &str, enabled: bool) -> ScheduleConfig {
    serde_json::from_value(json!({
        "name": name,
        "cronExpression": cron,
        "agentConfig": { "modelId": "m1", "agentId": "agent-a", "projectDirectory": "/work",
                         "inferenceConfig": { "maxTokens": 100 } },
        "wakeWord": "Go",
        "enabled": enabled
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn restores_electron_tasks_and_resets_execution_state() {
    let f = fixture_with(
        Scripted::new(vec![]),
        json!({ "backgroundAgentScheduledTasks": electron_tasks() }),
        true,
    );
    assert!(!f.bg.scheduler_started());
    f.bg.initialize_scheduler();
    assert!(f.bg.scheduler_started());

    let stored = f.config.get(SCHEDULED_TASKS_KEY).unwrap();
    assert_eq!(stored[0]["isExecuting"], false);
    assert!(stored[0].get("lastExecutionStarted").is_none());
    assert_eq!(stored[0]["futureField"], json!({ "kept": true }));
    assert_eq!(stored[1]["lastError"].as_str().is_some(), !cfg!(windows));

    let s = f.bg.scheduler();
    let tasks = s.list_tasks();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].run_count, 3);
    // nextRun recomputed from the cron expression.
    assert!(tasks[0].next_run.unwrap() > chrono::Utc::now().timestamp_millis());
    assert!(s.has_cron_job("task-1"));
    assert!(!s.has_cron_job("task-2"));
    assert_eq!(
        serde_json::to_value(s.get_stats()).unwrap(),
        json!({ "totalTasks": 2, "enabledTasks": 1, "disabledTasks": 1, "totalExecutions": 3,
                "tasksWithErrors": if cfg!(windows) { 0 } else { 1 }, "activeCronJobs": 1 })
    );
    // Existing execution history is served as-is.
    assert_eq!(
        f.bg.get_task_execution_history("task-1")["history"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    f.bg.shutdown_scheduler();
    assert!(!f.bg.scheduler_started());
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_tasks_are_kept_and_null_fields_tolerated() {
    let mut tasks = electron_tasks();
    let arr = tasks.as_array_mut().unwrap();
    // A task with `null` where TS allows unset values still loads.
    arr[1]["name"] = Value::Null;
    arr[1]["runCount"] = Value::Null;
    arr[1]["wakeWord"] = Value::Null;
    // Entries this version can't type: kept byte-for-byte.
    arr.push(json!({ "id": "weird", "enabled": "yes-please", "cronExpression": 5 }));
    arr.push(json!("not an object"));
    let f = fixture_with(
        Scripted::new(vec![]),
        json!({ "backgroundAgentScheduledTasks": tasks }),
        true,
    );
    f.bg.initialize_scheduler();
    let s = f.bg.scheduler();
    assert_eq!(s.list_tasks().len(), 2);
    assert_eq!(s.get_task("task-2").unwrap().name, "");

    // A save (toggle) writes the unreadable entries back unchanged.
    assert_eq!(
        f.bg.toggle_task("task-1", false),
        json!({ "success": true })
    );
    let stored = f.config.get(SCHEDULED_TASKS_KEY).unwrap();
    let stored = stored.as_array().unwrap();
    assert_eq!(stored.len(), 4);
    // (The startup reset adds `isExecuting: false` to every object entry, as in TS.)
    assert!(stored.contains(
        &json!({ "id": "weird", "enabled": "yes-please", "cronExpression": 5, "isExecuting": false })
    ));
    assert!(stored.contains(&json!("not an object")));
    f.bg.shutdown_scheduler();
}

#[tokio::test(flavor = "multi_thread")]
async fn initialize_skips_without_saved_tasks() {
    let f = fixture_with(Scripted::new(vec![]), json!({}), false);
    f.bg.initialize_scheduler();
    assert!(!f.bg.scheduler_started());
    // First use creates it.
    assert_eq!(f.bg.list_tasks(), json!({ "tasks": [] }));
    assert!(f.bg.scheduler_started());
}

#[tokio::test(flavor = "multi_thread")]
async fn task_crud_persists_to_the_config_store() {
    let f = fixture_with(Scripted::new(vec![]), json!({}), false);
    let err =
        f.bg.schedule_task(schedule_config("bad", "61 * * * *", true))
            .unwrap_err();
    assert_eq!(err.to_string(), "Invalid cron expression: 61 * * * *");

    let out =
        f.bg.schedule_task(schedule_config("Daily", "0 9 * * *", true))
            .unwrap();
    assert_eq!(out["success"], true);
    let id = out["taskId"].as_str().unwrap().to_string();
    let stored = f.config.get(SCHEDULED_TASKS_KEY).unwrap();
    let t = &stored[0];
    assert_eq!(t["id"], id.as_str());
    assert_eq!(t["agentId"], "agent-a");
    assert_eq!(t["modelId"], "m1");
    assert_eq!(t["projectDirectory"], "/work");
    assert_eq!(t["runCount"], 0);
    assert_eq!(t["inferenceConfig"], json!({ "maxTokens": 100 }));
    assert!(t["nextRun"].as_i64().is_some());
    assert!(t.get("continueSession").is_none());
    let s = f.bg.scheduler();
    assert!(s.has_cron_job(&id));

    assert_eq!(f.bg.toggle_task(&id, false), json!({ "success": true }));
    assert!(!s.has_cron_job(&id));
    assert_eq!(
        f.config.get(SCHEDULED_TASKS_KEY).unwrap()[0]["enabled"],
        false
    );
    assert_eq!(
        f.bg.toggle_task("missing", true),
        json!({ "success": false })
    );

    let mut cfg = schedule_config("Renamed", "30 8 * * 1", true);
    cfg.continue_session = Some(true);
    assert_eq!(
        f.bg.update_task(&id, cfg).unwrap(),
        json!({ "success": true, "taskId": id })
    );
    let t = s.get_task(&id).unwrap();
    assert_eq!(
        (t.name.as_str(), t.cron_expression.as_str(), t.enabled),
        ("Renamed", "30 8 * * 1", true)
    );
    assert_eq!(t.continue_session, Some(true));
    assert!(s.has_cron_job(&id));
    assert_eq!(
        f.bg.update_task(&id, schedule_config("x", "nope", true))
            .unwrap_err()
            .to_string(),
        format!("Failed to update task: {id}")
    );
    assert_eq!(f.bg.get_task(&id)["task"]["name"], "Renamed");
    assert_eq!(f.bg.get_task("missing"), json!({}));

    assert_eq!(f.bg.cancel_task(&id), json!({ "success": true }));
    assert_eq!(f.bg.cancel_task(&id), json!({ "success": false }));
    assert_eq!(f.config.get(SCHEDULED_TASKS_KEY).unwrap(), json!([]));
    assert!(!s.has_cron_job(&id));
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_run_end_to_end_with_tools() {
    let converse = Scripted::new(vec![
        tool_response("tu-1", "readFiles", json!({ "paths": ["/work/a.txt"] })),
        text_response("Report is ready."),
    ]);
    let f = fixture_with(converse, json!({}), false);
    let id =
        f.bg.schedule_task(schedule_config("Report", "0 9 * * *", false))
            .unwrap()["taskId"]
            .as_str()
            .unwrap()
            .to_string();

    // Disabled tasks still run manually.
    let out = f.bg.execute_task_manually(&id).await.unwrap();
    let result = &out["result"];
    assert_eq!(result["status"], "success");
    assert_eq!(result["taskId"], id.as_str());
    assert_eq!(result["messageCount"], 4);
    let session_id = result["sessionId"].as_str().unwrap().to_string();
    assert!(session_id.starts_with(&format!("scheduled-{id}-")));

    // The session file carries the task id and the whole exchange.
    let file = read_json(
        &f.dir
            .join(format!("background-agent-sessions/{session_id}.json")),
    );
    assert_eq!(file["taskId"], id.as_str());
    assert_eq!(file["projectDirectory"], "/work");
    let roles: Vec<_> = file["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].clone())
        .collect();
    assert_eq!(roles, vec!["user", "assistant", "user", "assistant"]);
    assert!(file["messages"][0]["content"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Go\n<context>\nDate: "));
    assert_eq!(
        file["messages"][2]["content"][0]["toolResult"]["status"],
        "success"
    );
    let meta = f.bg.get_all_sessions_metadata();
    assert_eq!(meta["metadata"][0]["executionType"], "scheduled");

    // Model call used the task's model, agent prompt and inference config.
    let reqs = f.converse.requests.lock().unwrap().clone();
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].model_id, "m1");
    assert_eq!(
        reqs[0].inference_config.as_ref().unwrap().max_tokens,
        Some(100)
    );
    assert!(reqs[0].system.as_ref().unwrap()[0]["text"]
        .as_str()
        .unwrap()
        .starts_with("You are a reporter in /work."));

    // Task stats and history.
    let task = f.bg.scheduler().get_task(&id).unwrap();
    assert_eq!(task.run_count, 1);
    assert!(task.last_run.is_some());
    assert!(task.last_session_id.is_none());
    let history = f.bg.get_task_execution_history(&id)["history"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(history.len(), 1, "running entry replaced by the final one");

    // Events and OS notification.
    let starts = f.sink.named(TASK_EXECUTION_START_EVENT);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["taskName"], "Report");
    let notes = f.sink.named(TASK_NOTIFICATION_EVENT);
    assert_eq!(notes.len(), 1);
    let n = &notes[0];
    assert_eq!(n["success"], true);
    assert_eq!(n["aiMessage"], "Report is ready.");
    assert_eq!(n["sessionId"], session_id.as_str());
    assert_eq!(n["messageCount"], 4);
    assert_eq!(n["toolExecutions"], 1);
    assert_eq!(n["runCount"], 1);
    assert!(n["nextRun"].as_i64().is_some());
    let shown = f.notifier.shown.lock().unwrap().clone();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].body, "[Report] Report is ready.");
    assert_eq!(shown[0].task_id, id);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_run_records_error() {
    let f = fixture_with(Scripted::new(vec![]), json!({}), false);
    let mut cfg = schedule_config("Broken", "0 9 * * *", false);
    cfg.agent_config.agent_id = "ghost".into();
    let id = f.bg.schedule_task(cfg).unwrap()["taskId"]
        .as_str()
        .unwrap()
        .to_string();
    let out = f.bg.execute_task_manually(&id).await.unwrap();
    assert_eq!(out["result"]["status"], "failed");
    assert_eq!(out["result"]["error"], "Agent not found: ghost");
    assert_eq!(out["result"]["messageCount"], 0);
    let task = f.bg.scheduler().get_task(&id).unwrap();
    assert_eq!(task.last_error.as_deref(), Some("Agent not found: ghost"));
    assert_eq!(task.run_count, 0);
    let n = &f.sink.named(TASK_NOTIFICATION_EVENT)[0];
    assert_eq!(n["success"], false);
    assert_eq!(n["error"], "Agent not found: ghost");
    assert_eq!(
        f.notifier.shown.lock().unwrap()[0].title,
        "Background Agent Task Failed"
    );
    assert_eq!(
        f.bg.execute_task_manually("missing")
            .await
            .unwrap_err()
            .to_string(),
        "Task not found: missing"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn continued_session_uses_continuation_prompt_and_publishes_updates() {
    let f = fixture_with(
        Scripted::new(vec![
            text_response("first"),
            text_response("second"),
            text_response("third"),
        ]),
        json!({}),
        false,
    );
    let mut cfg = schedule_config("Loop", "0 9 * * *", false);
    cfg.continue_session = Some(true);
    cfg.continue_session_prompt = Some("Continue please".into());
    let id = f.bg.schedule_task(cfg).unwrap()["taskId"]
        .as_str()
        .unwrap()
        .to_string();

    let first = f.bg.execute_task_manually(&id).await.unwrap()["result"].clone();
    let session = first["sessionId"].as_str().unwrap().to_string();
    assert_eq!(
        f.bg.scheduler()
            .get_task(&id)
            .unwrap()
            .last_session_id
            .as_deref(),
        Some(session.as_str())
    );

    f.bg.pubsub_subscribe(&format!("session-update:{session}"), "main");
    let second = f.bg.execute_task_manually(&id).await.unwrap()["result"].clone();
    assert_eq!(second["sessionId"], session.as_str());
    assert_eq!(second["messageCount"], 4);
    let history = f.bg.get_session_history(&session)["history"]
        .as_array()
        .unwrap()
        .clone();
    assert!(history[2]["content"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Continue please\n"));

    let event = format!("pubsub:session-update:{session}");
    let updates = f.sink.named(&event);
    assert_eq!(updates.len(), 2, "user message + final assistant message");
    assert_eq!(updates[0]["type"], "message-added");
    assert_eq!(updates[0]["sessionId"], session.as_str());
    assert_eq!(updates[1]["message"]["role"], "assistant");
    // Both runs share the session, and history entries are keyed by session: one entry.
    assert_eq!(
        f.bg.get_task_execution_history(&id)["history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // A deleted session falls back to a new one and clears lastSessionId first.
    assert_eq!(
        f.bg.delete_session(&session),
        json!({ "success": true, "sessionId": session })
    );
    let third = f.bg.execute_task_manually(&id).await.unwrap()["result"].clone();
    assert_ne!(third["sessionId"], session.as_str());
    let reqs = f.converse.requests.lock().unwrap().clone();
    let last_user = reqs[2].messages.last().unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(last_user.starts_with("Go\n"), "{last_user}");
}

#[tokio::test(flavor = "multi_thread")]
async fn overlapping_scheduled_run_is_skipped() {
    let f = fixture_with(
        Scripted::with_delay(vec![text_response("slow")], Duration::from_millis(400)),
        json!({}),
        false,
    );
    let id =
        f.bg.schedule_task(schedule_config("Slow", "0 9 * * *", true))
            .unwrap()["taskId"]
            .as_str()
            .unwrap()
            .to_string();
    let s = f.bg.scheduler();
    let s2 = s.clone();
    let id2 = id.clone();
    let first = tokio::spawn(async move { s2.execute_task(&id2).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(s.get_task(&id).unwrap().is_executing, Some(true));
    s.execute_task(&id).await;
    let skipped = f.sink.named(TASK_SKIPPED_EVENT);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["reason"], "duplicate_execution");
    assert_eq!(skipped[0]["taskName"], "Slow");
    first.await.unwrap();
    let t = s.get_task(&id).unwrap();
    assert_eq!(t.is_executing, Some(false));
    assert!(t.last_execution_started.is_none());
    assert_eq!(t.run_count, 1);
    // Disabled tasks are not run by the schedule.
    s.toggle_task(&id, false);
    s.execute_task(&id).await;
    assert_eq!(s.get_task(&id).unwrap().run_count, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn cron_job_fires_on_schedule() {
    let f = fixture_with(
        Scripted::new(vec![text_response("tick"), text_response("tick")]),
        json!({}),
        false,
    );
    let id =
        f.bg.schedule_task(schedule_config("Every second", "* * * * * *", true))
            .unwrap()["taskId"]
            .as_str()
            .unwrap()
            .to_string();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if f.bg.scheduler().get_task(&id).unwrap().run_count >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "cron job did not fire"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    f.bg.toggle_task(&id, false);
    assert_eq!(f.sink.named(TASK_NOTIFICATION_EVENT)[0]["success"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn session_handlers_return_electron_shapes() {
    let f = fixture_with(
        Scripted::new(vec![text_response("hi there"), text_response("again")]),
        json!({}),
        true,
    );
    assert_eq!(
        f.bg.create_session(
            "new-1",
            Some(SessionMeta {
                project_directory: Some("/p".into()),
                ..Default::default()
            })
        )
        .unwrap(),
        json!({ "success": true, "sessionId": "new-1", "projectDirectory": "/p" })
    );
    assert_eq!(
        f.bg.create_session("new-2", None).unwrap(),
        json!({ "success": true, "sessionId": "new-2" })
    );
    assert_eq!(
        f.bg.list_sessions(),
        json!({ "sessions": ["manual-1", "new-1", "new-2", "scheduled-task-1-0f0e"] })
    );
    assert_eq!(
        f.bg.get_sessions_by_project("/p")["sessions"][0]["sessionId"],
        "new-1"
    );
    assert_eq!(
        f.bg.get_sessions_by_agent("agent-a")["sessions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let params: ChatParams = serde_json::from_value(json!({
        "sessionId": "chat-1",
        "config": { "modelId": "m9", "agentId": "agent-a", "projectDirectory": "/x", "systemPrompt": "ignored" },
        "userMessage": "Hello"
    }))
    .unwrap();
    let result = f.bg.chat(params).await.unwrap();
    let v = serde_json::to_value(&result).unwrap();
    assert_eq!(v["response"]["role"], "assistant");
    assert_eq!(v["response"]["content"], json!([{ "text": "hi there" }]));
    assert!(v.get("toolExecutions").is_none());
    let stats = serde_json::to_value(f.bg.get_session_stats("chat-1")).unwrap();
    assert_eq!(stats["messageCount"], 2);
    assert_eq!(stats["metadata"]["executionType"], "manual");
    assert_eq!(stats["metadata"]["modelId"], "m9");
    assert_eq!(stats["metadata"]["projectDirectory"], "/x");

    // continue-session uses the task's agent/model.
    let id =
        f.bg.schedule_task(schedule_config("T", "0 9 * * *", false))
            .unwrap()["taskId"]
            .as_str()
            .unwrap()
            .to_string();
    let params: ContinueSessionParams = serde_json::from_value(json!({
        "sessionId": "chat-1", "taskId": id, "userMessage": "More"
    }))
    .unwrap();
    let r = f.bg.continue_session(params).await.unwrap();
    assert_eq!(r.response.content, vec![json!({ "text": "again" })]);
    assert_eq!(f.converse.requests.lock().unwrap()[1].model_id, "m1");
    let missing: ContinueSessionParams = serde_json::from_value(
        json!({ "sessionId": "chat-1", "taskId": "nope", "userMessage": "x" }),
    )
    .unwrap();
    assert_eq!(
        f.bg.continue_session(missing)
            .await
            .unwrap_err()
            .to_string(),
        "Task not found: nope"
    );

    let prompt = f.bg.get_task_system_prompt(&id).unwrap();
    assert!(prompt["systemPrompt"]
        .as_str()
        .unwrap()
        .starts_with("You are a reporter in /work."));
    assert_eq!(
        f.bg.get_task_system_prompt("nope").unwrap_err().to_string(),
        "Task not found: nope"
    );

    assert_eq!(
        f.bg.delete_session("chat-1"),
        json!({ "success": true, "sessionId": "chat-1" })
    );
    assert_eq!(f.bg.get_session_history("chat-1"), json!({ "history": [] }));

    f.bg.task_notification(json!({ "taskId": "x" }));
    assert_eq!(
        f.sink.named(TASK_NOTIFICATION_EVENT),
        vec![json!({ "taskId": "x" })]
    );
    assert_eq!(
        serde_json::to_value(f.bg.get_scheduler_stats()).unwrap()["stats"]["totalTasks"],
        1
    );
}
