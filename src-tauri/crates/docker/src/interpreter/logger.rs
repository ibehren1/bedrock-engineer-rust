//! The `ToolLogger` the interpreter classes take (`src/preload/tools/base/types.ts`).

use std::sync::Mutex;

use serde_json::Value;

pub trait ToolLogger: Send + Sync {
    fn debug(&self, message: &str, meta: Value);
    fn info(&self, message: &str, meta: Value);
    fn warn(&self, message: &str, meta: Value);
    fn error(&self, message: &str, meta: Value);
}

/// Forwards to the `log` facade under the `tools:codeInterpreter` target.
#[derive(Debug, Default, Clone, Copy)]
pub struct LogFacadeLogger;

impl ToolLogger for LogFacadeLogger {
    fn debug(&self, message: &str, meta: Value) {
        log::debug!(target: "tools:codeInterpreter", "{message} {meta}");
    }
    fn info(&self, message: &str, meta: Value) {
        log::info!(target: "tools:codeInterpreter", "{message} {meta}");
    }
    fn warn(&self, message: &str, meta: Value) {
        log::warn!(target: "tools:codeInterpreter", "{message} {meta}");
    }
    fn error(&self, message: &str, meta: Value) {
        log::error!(target: "tools:codeInterpreter", "{message} {meta}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

/// Records every call, for tests (the TS tests pass `jest.fn()` loggers).
#[derive(Debug, Default)]
pub struct RecordingLogger {
    calls: Mutex<Vec<(Level, String, Value)>>,
}

impl RecordingLogger {
    pub fn calls(&self) -> Vec<(Level, String, Value)> {
        self.calls.lock().unwrap().clone()
    }

    /// True when `message` was logged at `level` with exactly `meta`.
    pub fn was_called_with(&self, level: Level, message: &str, meta: &Value) -> bool {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .any(|(l, m, v)| *l == level && m == message && v == meta)
    }

    /// True when any message at `level` contains `fragment`.
    pub fn any_containing(&self, level: Level, fragment: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .any(|(l, m, _)| *l == level && m.contains(fragment))
    }

    fn push(&self, level: Level, message: &str, meta: Value) {
        self.calls
            .lock()
            .unwrap()
            .push((level, message.to_string(), meta));
    }
}

impl ToolLogger for RecordingLogger {
    fn debug(&self, message: &str, meta: Value) {
        self.push(Level::Debug, message, meta);
    }
    fn info(&self, message: &str, meta: Value) {
        self.push(Level::Info, message, meta);
    }
    fn warn(&self, message: &str, meta: Value) {
        self.push(Level::Warn, message, meta);
    }
    fn error(&self, message: &str, meta: Value) {
        self.push(Level::Error, message, meta);
    }
}
