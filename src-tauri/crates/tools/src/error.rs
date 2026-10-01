//! Port of `src/preload/tools/base/errors.ts`.
//!
//! A [`ToolError`] carries the JS error class name (`name`), the message, and — for the
//! `ToolError` subclasses — the error type and metadata used by `toResponse()`. The
//! renderer shows failures as `e.toString()`, i.e. `` `${name}: ${message}` ``, which is
//! [`ToolError::to_js_string`]. At the Tauri boundary, pass `{ name, message }`
//! ([`ToolError::to_js_error`]) so the shim can rethrow an `Error` whose `toString()` is
//! identical.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// `ToolErrorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolErrorType {
    Validation,
    Execution,
    NotFound,
    PermissionDenied,
    RateLimit,
    Network,
    Unknown,
}

/// An error raised by a tool or by the registry.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{name}: {message}")]
pub struct ToolError {
    /// JS `error.name` (`ValidationError`, `ExecutionError`, `Error`, ...).
    pub name: &'static str,
    pub message: String,
    /// `None` for plain `Error`s (not a `ToolError` subclass).
    pub kind: Option<ToolErrorType>,
    /// `ToolErrorMetadata`, in insertion order (`toolName` first).
    pub metadata: Map<String, Value>,
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, ToolError>;

/// Crate error alias, per the workspace convention.
pub type Error = ToolError;

fn meta(tool_name: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("toolName".into(), json!(tool_name));
    m
}

fn insert_opt(m: &mut Map<String, Value>, key: &str, v: Option<Value>) {
    if let Some(v) = v {
        m.insert(key.into(), v);
    }
}

impl ToolError {
    /// A plain `new Error(message)`.
    pub fn plain(message: impl Into<String>) -> Self {
        ToolError {
            name: "Error",
            message: message.into(),
            kind: None,
            metadata: Map::new(),
        }
    }

    /// `new ValidationError(message, toolName, input)`.
    pub fn validation(message: impl Into<String>, tool_name: &str, input: Option<Value>) -> Self {
        let mut m = meta(tool_name);
        insert_opt(&mut m, "input", input);
        ToolError {
            name: "ValidationError",
            message: message.into(),
            kind: Some(ToolErrorType::Validation),
            metadata: m,
        }
    }

    /// `new ExecutionError(message, toolName, cause, additionalData)`.
    ///
    /// `cause` is the `JSON.stringify` form of the causing error (`{}` for a plain `Error`,
    /// `{errno, code, syscall, path}` for Node system errors), or `None` when absent.
    pub fn execution(
        message: impl Into<String>,
        tool_name: &str,
        cause: Option<Value>,
        additional: Option<Map<String, Value>>,
    ) -> Self {
        let mut m = meta(tool_name);
        insert_opt(&mut m, "cause", cause);
        if let Some(extra) = additional {
            for (k, v) in extra {
                m.insert(k, v);
            }
        }
        ToolError {
            name: "ExecutionError",
            message: message.into(),
            kind: Some(ToolErrorType::Execution),
            metadata: m,
        }
    }

    /// `new ToolNotFoundError(toolName)`.
    pub fn not_found(tool_name: &str) -> Self {
        ToolError {
            name: "ToolNotFoundError",
            message: format!("Tool not found: {tool_name}"),
            kind: Some(ToolErrorType::NotFound),
            metadata: meta(tool_name),
        }
    }

    /// `new PermissionDeniedError(message, toolName, operation)`.
    pub fn permission_denied(
        message: impl Into<String>,
        tool_name: &str,
        operation: Option<&str>,
    ) -> Self {
        let mut m = meta(tool_name);
        insert_opt(&mut m, "operation", operation.map(|o| json!(o)));
        ToolError {
            name: "PermissionDeniedError",
            message: message.into(),
            kind: Some(ToolErrorType::PermissionDenied),
            metadata: m,
        }
    }

    /// `new RateLimitError(message, toolName, suggestedAlternatives)`.
    pub fn rate_limit(
        message: impl Into<String>,
        tool_name: &str,
        suggested_alternatives: Option<Vec<String>>,
    ) -> Self {
        let mut m = meta(tool_name);
        insert_opt(
            &mut m,
            "suggestedAlternatives",
            suggested_alternatives.map(|s| json!(s)),
        );
        ToolError {
            name: "RateLimitError",
            message: message.into(),
            kind: Some(ToolErrorType::RateLimit),
            metadata: m,
        }
    }

    /// `new NetworkError(message, toolName, url, statusCode)`.
    pub fn network(
        message: impl Into<String>,
        tool_name: &str,
        url: Option<&str>,
        status_code: Option<u16>,
    ) -> Self {
        let mut m = meta(tool_name);
        insert_opt(&mut m, "url", url.map(|u| json!(u)));
        insert_opt(&mut m, "statusCode", status_code.map(|s| json!(s)));
        ToolError {
            name: "NetworkError",
            message: message.into(),
            kind: Some(ToolErrorType::Network),
            metadata: m,
        }
    }

    /// `isToolError(error)`.
    pub fn is_tool_error(&self) -> bool {
        self.kind.is_some()
    }

    /// `wrapError(error, toolName)`: tool errors pass through; plain errors become an
    /// `ExecutionError` whose cause is the (non-enumerable, hence `{}`) original error.
    pub fn wrap(self, tool_name: &str) -> Self {
        if self.is_tool_error() {
            return self;
        }
        if self.name == "ThrottlingException" {
            return ToolError::rate_limit(self.message, tool_name, None);
        }
        ToolError::execution(self.message, tool_name, Some(json!({})), None)
    }

    /// `toolError.toResponse()`:
    /// `JSON.stringify({ success: false, error, type, ...metadata })`.
    pub fn to_response(&self) -> String {
        let mut m = Map::new();
        m.insert("success".into(), json!(false));
        m.insert("error".into(), json!(self.message));
        m.insert(
            "type".into(),
            serde_json::to_value(self.kind.unwrap_or(ToolErrorType::Unknown))
                .unwrap_or(Value::Null),
        );
        for (k, v) in &self.metadata {
            m.insert(k.clone(), v.clone());
        }
        Value::Object(m).to_string()
    }

    /// `String(error)` / `error.toString()` in JS.
    pub fn to_js_string(&self) -> String {
        format!("{}: {}", self.name, self.message)
    }

    /// `{ name, message }` for the Tauri command boundary.
    pub fn to_js_error(&self) -> Value {
        json!({ "name": self.name, "message": self.message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_shape_matches_ts() {
        let e = ToolError::validation(
            "Invalid input: Path is required",
            "createFolder",
            Some(json!({"type": "createFolder"})),
        );
        assert_eq!(
            e.to_response(),
            r#"{"success":false,"error":"Invalid input: Path is required","type":"VALIDATION","toolName":"createFolder","input":{"type":"createFolder"}}"#
        );
        assert_eq!(
            e.to_js_string(),
            "ValidationError: Invalid input: Path is required"
        );
    }

    #[test]
    fn wrap_plain_error() {
        let e = ToolError::plain("boom").wrap("think");
        assert_eq!(e.name, "ExecutionError");
        assert_eq!(
            e.to_response(),
            r#"{"success":false,"error":"boom","type":"EXECUTION","toolName":"think","cause":{}}"#
        );
        let t = ToolError::not_found("x");
        assert_eq!(t.clone().wrap("y"), t);
    }

    #[test]
    fn network_metadata() {
        let e = ToolError::network(
            "Tavily API error: 201",
            "tavilySearch",
            Some("https://api.tavily.com/search"),
            Some(201),
        );
        assert_eq!(
            e.to_response(),
            r#"{"success":false,"error":"Tavily API error: 201","type":"NETWORK","toolName":"tavilySearch","url":"https://api.tavily.com/search","statusCode":201}"#
        );
    }
}
