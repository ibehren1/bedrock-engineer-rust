//! Task execution history: the electron-store file `background-agent-execution-history.json`
//! (`{ "executionHistory": { [taskId]: TaskExecutionResult[] } }`), newest last, at most 100
//! entries per task.

use crate::config::JsonFile;
use crate::types::TaskExecutionResult;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Mutex;

/// electron-store name of the history file.
pub const EXECUTION_HISTORY_STORE: &str = "background-agent-execution-history";
/// Entries kept per task.
pub const MAX_HISTORY_PER_TASK: usize = 100;

pub struct ExecutionHistoryStore {
    file: Mutex<JsonFile>,
}

/// One stored entry: typed when it deserializes, otherwise kept as the raw JSON it was so a
/// rewrite of the list writes it back unchanged (an entry from a newer version, or with a field
/// this version types more strictly, is not dropped).
#[derive(Debug, Clone)]
enum Entry {
    Typed(Box<TaskExecutionResult>),
    Raw(Value),
}

impl Entry {
    fn parse(v: Value) -> Entry {
        match serde_json::from_value::<TaskExecutionResult>(v.clone()) {
            Ok(t) => Entry::Typed(Box::new(t)),
            Err(e) => {
                tracing::warn!(error = %e, "Keeping an execution history entry this version can't read");
                Entry::Raw(v)
            }
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Entry::Typed(t) => serde_json::to_value(t).unwrap_or(Value::Null),
            Entry::Raw(v) => v.clone(),
        }
    }

    fn typed(&self) -> Option<&TaskExecutionResult> {
        match self {
            Entry::Typed(t) => Some(t),
            Entry::Raw(_) => None,
        }
    }
}

fn all_history(v: Option<Value>) -> Map<String, Value> {
    match v {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn entries(all: &Map<String, Value>, task_id: &str) -> Vec<Entry> {
    all.get(task_id)
        .and_then(Value::as_array)
        .map(|a| a.iter().cloned().map(Entry::parse).collect())
        .unwrap_or_default()
}

fn to_array(list: &[Entry]) -> Value {
    Value::Array(list.iter().map(Entry::to_value).collect())
}

impl ExecutionHistoryStore {
    /// Open `<store_dir>/background-agent-execution-history.json`.
    pub fn open(store_dir: impl AsRef<Path>) -> Self {
        ExecutionHistoryStore {
            file: Mutex::new(JsonFile::open(
                store_dir.as_ref(),
                EXECUTION_HISTORY_STORE,
                "executionHistory",
                json!({}),
            )),
        }
    }

    /// Read-modify-write of one task's list in a single pass over the file; nothing is written
    /// when the file can't be read or `f` returns `false`.
    fn edit(&self, task_id: &str, f: impl FnOnce(&mut Map<String, Value>) -> bool) {
        let mut file = self.file.lock().unwrap();
        let result = file.update_key("executionHistory", |current| {
            let mut all = all_history(current.clone());
            if f(&mut all) {
                (Value::Object(all), ())
            } else {
                (current.unwrap_or_else(|| json!({})), ())
            }
        });
        if let Err(e) = result {
            tracing::error!(task_id, error = %e, "Failed to record execution history");
        }
    }

    /// `recordExecution(taskId, result)`: replace the entry with the same `sessionId`, or append;
    /// then keep the newest 100.
    pub fn record(&self, task_id: &str, result: TaskExecutionResult) {
        self.edit(task_id, |all| {
            let mut list = entries(all, task_id);
            let pos = list
                .iter()
                .position(|e| e.typed().is_some_and(|t| t.session_id == result.session_id));
            let entry = Entry::Typed(Box::new(result));
            match pos {
                Some(i) => list[i] = entry,
                None => list.push(entry),
            }
            if list.len() > MAX_HISTORY_PER_TASK {
                list.drain(..list.len() - MAX_HISTORY_PER_TASK);
            }
            all.insert(task_id.to_string(), to_array(&list));
            true
        });
    }

    /// `updateExecutionHistoryMessageCount`: update the latest entry's `messageCount` when it
    /// belongs to `session_id`.
    pub fn update_message_count(&self, task_id: &str, session_id: &str, message_count: usize) {
        self.edit(task_id, |all| {
            let mut list = entries(all, task_id);
            let Some(Entry::Typed(latest)) = list.last_mut() else {
                return false;
            };
            if latest.session_id != session_id {
                tracing::debug!(
                    task_id,
                    session_id,
                    "Session ID mismatch for real-time update"
                );
                return false;
            }
            latest.message_count = message_count;
            all.insert(task_id.to_string(), to_array(&list));
            true
        });
    }

    /// `getTaskExecutionHistory(taskId)` (entries this version can read).
    pub fn get(&self, task_id: &str) -> Vec<TaskExecutionResult> {
        let file = self.file.lock().unwrap();
        entries(&all_history(file.get("executionHistory")), task_id)
            .into_iter()
            .filter_map(|e| match e {
                Entry::Typed(t) => Some(*t),
                Entry::Raw(_) => None,
            })
            .collect()
    }

    /// Drop a task's history (`cancelTask`).
    pub fn remove(&self, task_id: &str) {
        self.edit(task_id, |all| all.remove(task_id).is_some());
    }
}
