//! A small runtime schema checker reproducing the Zod 3 semantics the TS code relies on.
//!
//! The Electron app validates agents and MCP configs with Zod (`safeParse`). Parity matters for
//! two things: *which* inputs pass, and the issue list (`path` + `message`) that gets logged or
//! shown to the user. This module mirrors Zod 3.25's behavior for the subset of combinators the
//! schemas use: `string`, `boolean`, `any`, `enum`, `string().regex()`, `array`, `record`,
//! `object` (strip mode and `.strict()`), `union`, and `.optional()`.
//!
//! Parsing returns the Zod output value: object keys come out in schema order, unknown keys are
//! stripped (non-strict objects), and absent optional keys stay absent.

use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};

/// One element of an issue path: an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

impl std::fmt::Display for PathSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathSegment::Key(k) => f.write_str(k),
            PathSegment::Index(i) => write!(f, "{i}"),
        }
    }
}

/// A validation issue, equivalent to a `ZodIssue` reduced to what the app uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub path: Vec<PathSegment>,
    pub message: String,
}

impl Issue {
    /// `issue.path.join('.')`
    pub fn path_string(&self) -> String {
        self.path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// `{ path, message }` as produced by the TS `formatZodErrors` helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormattedIssue {
    pub path: String,
    pub message: String,
}

impl From<&Issue> for FormattedIssue {
    fn from(issue: &Issue) -> Self {
        Self {
            path: issue.path_string(),
            message: issue.message.clone(),
        }
    }
}

/// Schema node.
pub enum Schema {
    String,
    Boolean,
    Any,
    Enum(Vec<&'static str>),
    /// `z.string().regex(re, message)`
    Regex(Regex, &'static str),
    Array(Box<Schema>),
    /// `z.record(z.string(), value)`
    Record(Box<Schema>),
    Object {
        fields: Vec<Field>,
        strict: bool,
    },
    Union(Vec<Schema>),
}

pub struct Field {
    pub name: &'static str,
    pub schema: Schema,
    pub optional: bool,
}

pub fn req(name: &'static str, schema: Schema) -> Field {
    Field {
        name,
        schema,
        optional: false,
    }
}

pub fn opt(name: &'static str, schema: Schema) -> Field {
    Field {
        name,
        schema,
        optional: true,
    }
}

pub fn object(fields: Vec<Field>) -> Schema {
    Schema::Object {
        fields,
        strict: false,
    }
}

pub fn strict_object(fields: Vec<Field>) -> Schema {
    Schema::Object {
        fields,
        strict: true,
    }
}

pub fn array(item: Schema) -> Schema {
    Schema::Array(Box::new(item))
}

pub fn record(value: Schema) -> Schema {
    Schema::Record(Box::new(value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Valid,
    Dirty,
    Aborted,
}

/// Zod's `getParsedType` name for a JSON value (`undefined` for a missing value).
fn type_name(value: Option<&Value>) -> &'static str {
    match value {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn invalid_type(expected: &str, received: Option<&Value>) -> String {
    match received {
        None => "Required".to_string(),
        Some(_) => format!("Expected {expected}, received {}", type_name(received)),
    }
}

fn join_values(values: &[&str], sep: &str) -> String {
    values
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(sep)
}

impl Schema {
    /// Equivalent of `schema.safeParse(value)`: the parsed output on success, or every issue.
    pub fn safe_parse(&self, value: &Value) -> Result<Value, Vec<Issue>> {
        let mut issues = Vec::new();
        let mut path = Vec::new();
        let (status, out) = self.parse_at(Some(value), &mut path, &mut issues);
        if status == Status::Valid && issues.is_empty() {
            Ok(out.unwrap_or(Value::Null))
        } else {
            Err(issues)
        }
    }

    fn parse_at(
        &self,
        value: Option<&Value>,
        path: &mut Vec<PathSegment>,
        issues: &mut Vec<Issue>,
    ) -> (Status, Option<Value>) {
        fn push(issues: &mut Vec<Issue>, path: &[PathSegment], message: String) {
            issues.push(Issue {
                path: path.to_vec(),
                message,
            })
        }
        match self {
            Schema::Any => (Status::Valid, value.cloned()),
            Schema::String => match value {
                Some(Value::String(_)) => (Status::Valid, value.cloned()),
                _ => {
                    push(issues, path, invalid_type("string", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Boolean => match value {
                Some(Value::Bool(_)) => (Status::Valid, value.cloned()),
                _ => {
                    push(issues, path, invalid_type("boolean", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Enum(options) => match value {
                Some(Value::String(s)) => {
                    if options.contains(&s.as_str()) {
                        (Status::Valid, value.cloned())
                    } else {
                        push(
                            issues,
                            path,
                            format!(
                                "Invalid enum value. Expected {}, received '{s}'",
                                join_values(options, " | ")
                            ),
                        );
                        (Status::Aborted, None)
                    }
                }
                _ => {
                    push(
                        issues,
                        path,
                        invalid_type(&join_values(options, " | "), value),
                    );
                    (Status::Aborted, None)
                }
            },
            Schema::Regex(re, message) => match value {
                Some(Value::String(s)) => {
                    if re.is_match(s) {
                        (Status::Valid, value.cloned())
                    } else {
                        push(issues, path, (*message).to_string());
                        (Status::Dirty, value.cloned())
                    }
                }
                _ => {
                    push(issues, path, invalid_type("string", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Array(item) => match value {
                Some(Value::Array(items)) => {
                    let mut status = Status::Valid;
                    let mut out = Vec::with_capacity(items.len());
                    for (i, v) in items.iter().enumerate() {
                        path.push(PathSegment::Index(i));
                        let (s, o) = item.parse_at(Some(v), path, issues);
                        path.pop();
                        status = status.max(s);
                        out.push(o.unwrap_or(Value::Null));
                    }
                    (status, Some(Value::Array(out)))
                }
                _ => {
                    push(issues, path, invalid_type("array", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Record(inner) => match value {
                Some(Value::Object(map)) => {
                    let mut status = Status::Valid;
                    let mut out = Map::new();
                    for (k, v) in map {
                        path.push(PathSegment::Key(k.clone()));
                        let (s, o) = inner.parse_at(Some(v), path, issues);
                        path.pop();
                        status = status.max(s);
                        if let Some(o) = o {
                            out.insert(k.clone(), o);
                        }
                    }
                    (status, Some(Value::Object(out)))
                }
                _ => {
                    push(issues, path, invalid_type("object", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Object { fields, strict } => match value {
                Some(Value::Object(map)) => {
                    let mut status = Status::Valid;
                    let mut out = Map::new();
                    for field in fields {
                        let v = map.get(field.name);
                        path.push(PathSegment::Key(field.name.to_string()));
                        // `.optional()` accepts `undefined` (a missing key) but not `null`.
                        let (s, o) = if v.is_none() && field.optional {
                            (Status::Valid, None)
                        } else {
                            field.schema.parse_at(v, path, issues)
                        };
                        path.pop();
                        status = status.max(s);
                        if let Some(o) = o {
                            out.insert(field.name.to_string(), o);
                        }
                    }
                    if *strict {
                        let extra: Vec<&str> = map
                            .keys()
                            .filter(|k| !fields.iter().any(|f| f.name == k.as_str()))
                            .map(String::as_str)
                            .collect();
                        if !extra.is_empty() {
                            push(
                                issues,
                                path,
                                format!(
                                    "Unrecognized key(s) in object: {}",
                                    join_values(&extra, ", ")
                                ),
                            );
                            status = status.max(Status::Dirty);
                        }
                    }
                    (status, Some(Value::Object(out)))
                }
                _ => {
                    push(issues, path, invalid_type("object", value));
                    (Status::Aborted, None)
                }
            },
            Schema::Union(options) => {
                let mut dirty: Option<(Vec<Issue>, Option<Value>)> = None;
                for option in options {
                    let mut child = Vec::new();
                    let (s, o) = option.parse_at(value, path, &mut child);
                    if s == Status::Valid && child.is_empty() {
                        return (Status::Valid, o);
                    }
                    if s == Status::Dirty && dirty.is_none() {
                        dirty = Some((child, o));
                    }
                }
                if let Some((child, o)) = dirty {
                    issues.extend(child);
                    return (Status::Dirty, o);
                }
                push(issues, path, "Invalid input".to_string());
                (Status::Aborted, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_and_type_messages_match_zod() {
        let s = object(vec![req("a", Schema::String), opt("b", Schema::Boolean)]);
        let err = s.safe_parse(&json!({ "b": 1 })).unwrap_err();
        assert_eq!(err[0].path_string(), "a");
        assert_eq!(err[0].message, "Required");
        assert_eq!(err[1].message, "Expected boolean, received number");
    }

    #[test]
    fn optional_rejects_null() {
        let s = object(vec![opt("a", Schema::String)]);
        assert!(s.safe_parse(&json!({ "a": null })).is_err());
        assert!(s.safe_parse(&json!({})).is_ok());
    }

    #[test]
    fn strips_unknown_keys_in_schema_order() {
        let s = object(vec![req("b", Schema::String), req("a", Schema::String)]);
        let out = s
            .safe_parse(&json!({ "a": "1", "x": 2, "b": "2" }))
            .unwrap();
        assert_eq!(serde_json::to_string(&out).unwrap(), r#"{"b":"2","a":"1"}"#);
    }

    #[test]
    fn union_prefers_dirty_issues() {
        let s = Schema::Union(vec![
            Schema::Enum(vec!["robot"]),
            Schema::Regex(Regex::new("^[a-z]+:[a-z]+$").unwrap(), "bad id"),
        ]);
        let err = s.safe_parse(&json!("Nope")).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].message, "bad id");
        let err = s.safe_parse(&json!(3)).unwrap_err();
        assert_eq!(err[0].message, "Invalid input");
        assert!(s.safe_parse(&json!("robot")).is_ok());
        assert!(s.safe_parse(&json!("a:b")).is_ok());
    }

    #[test]
    fn enum_messages() {
        let s = Schema::Enum(vec!["a", "b"]);
        assert_eq!(
            s.safe_parse(&json!("c")).unwrap_err()[0].message,
            "Invalid enum value. Expected 'a' | 'b', received 'c'"
        );
    }
}
