//! Zod-style validation messages for the tools whose TS versions validate with Zod
//! (`tavilySearch`, `todoInit`, `todoUpdate`). Errors are rendered as
//! `` `${path.join('.')}: ${message}` `` exactly like the TS `validateInput`.

use serde_json::Value;

/// Zod's name for a JSON value's type (`ZodParsedType`).
pub fn zod_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Collects Zod issues.
#[derive(Default)]
pub struct Issues(pub Vec<String>);

impl Issues {
    pub fn push(&mut self, path: &[&str], message: impl AsRef<str>) {
        self.0
            .push(format!("{}: {}", path.join("."), message.as_ref()));
    }

    /// `invalid_type` issue: `Required` for undefined, else `Expected x, received y`.
    pub fn invalid_type(&mut self, path: &[&str], expected: &str, got: Option<&Value>) {
        match got {
            None => self.push(path, "Required"),
            Some(v) => self.push(
                path,
                format!("Expected {expected}, received {}", zod_type(v)),
            ),
        }
    }

    /// Checks `v` is a string (present or `optional`). Returns it when valid.
    pub fn string<'a>(
        &mut self,
        path: &[&str],
        v: Option<&'a Value>,
        optional: bool,
    ) -> Option<&'a str> {
        match v {
            None if optional => None,
            Some(Value::String(s)) => Some(s),
            other => {
                self.invalid_type(path, "string", other);
                None
            }
        }
    }

    pub fn into_vec(self) -> Vec<String> {
        self.0
    }
}
