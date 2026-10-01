//! `window.logger.*` (Electron: `ipcRenderer.send('logger:log', entry)`), written to the same
//! winston-format log files as the backend via `common::logger`.

use serde_json::{Map, Value};

macro_rules! emit {
    ($level:ident, $category:expr, $message:expr, $meta:expr) => {
        match $meta {
            Some(meta) => tracing::$level!(process = "renderer", category = $category, meta = %meta, "{}", $message),
            None => tracing::$level!(process = "renderer", category = $category, "{}", $message),
        }
    };
}

#[tauri::command]
pub fn logger_log(entry: Value) {
    let mut fields: Map<String, Value> = entry.as_object().cloned().unwrap_or_default();
    let mut take = |k: &str| match fields.remove(k) {
        Some(Value::String(s)) => s,
        _ => String::new(),
    };
    let level = take("level");
    let message = take("message");
    let category = take("category");
    let category = if category.is_empty() {
        "general".to_string()
    } else {
        category
    };
    fields.remove("timestamp");
    fields.remove("process");
    let meta = (!fields.is_empty()).then(|| Value::Object(fields).to_string());
    let category = category.as_str();
    match level.as_str() {
        "error" => emit!(error, category, message, meta),
        "warn" => emit!(warn, category, message, meta),
        "debug" => emit!(debug, category, message, meta),
        "verbose" => emit!(trace, category, message, meta),
        _ => emit!(info, category, message, meta),
    }
}
