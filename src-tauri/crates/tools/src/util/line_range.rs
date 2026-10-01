//! Port of `src/preload/lib/line-range-utils.ts`.

use serde_json::Value;

/// 1-based inclusive line range, as accepted by readFiles / listFiles / fetchWebsite.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LineRange {
    pub from: Option<f64>,
    pub to: Option<f64>,
}

impl LineRange {
    /// Read `{ from?, to? }` from a JSON value. Non-numeric fields are treated as absent
    /// here; [`validate_line_range`] reports them.
    pub fn from_value(v: Option<&Value>) -> Option<LineRange> {
        let obj = v?.as_object()?;
        Some(LineRange {
            from: obj.get("from").and_then(Value::as_f64),
            to: obj.get("to").and_then(Value::as_f64),
        })
    }
}

/// `lineRange.from || 1` — zero counts as absent, like JS.
fn or_default(v: Option<f64>, default: f64) -> f64 {
    match v {
        Some(n) if n != 0.0 && !n.is_nan() => n,
        _ => default,
    }
}

/// `filterByLineRange`.
pub fn filter_by_line_range(content: &str, range: Option<&LineRange>) -> String {
    let Some(range) = range else {
        return content.to_string();
    };
    let lines: Vec<&str> = content.split('\n').collect();
    let total = lines.len() as f64;
    let from = or_default(range.from, 1.0).max(1.0);
    let to = or_default(range.to, total).min(total);
    let start = (from - 1.0).max(0.0) as usize;
    let end = if to < 0.0 { 0 } else { to as usize };
    if start >= end || start >= lines.len() {
        return String::new();
    }
    lines[start..end.min(lines.len())].join("\n")
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `getLineRangeInfo`.
pub fn get_line_range_info(total_lines: usize, range: Option<&LineRange>) -> String {
    let Some(range) = range else {
        return String::new();
    };
    let total = total_lines as f64;
    let from = or_default(range.from, 1.0);
    let to = or_default(range.to, total);
    format!(" (lines {} to {})", fmt_num(from), fmt_num(to.min(total)))
}

/// `validateLineRange` operating on the raw JSON, so type errors are reported the same way.
pub fn validate_line_range(v: Option<&Value>) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(obj) = v.and_then(Value::as_object) else {
        return errors;
    };
    let from = obj.get("from");
    let to = obj.get("to");
    let bad = |x: &Value| x.as_f64().is_none_or(|n| n < 1.0);
    if let Some(f) = from {
        if bad(f) {
            errors.push("Line range \"from\" must be a positive integer".to_string());
        }
    }
    if let Some(t) = to {
        if bad(t) {
            errors.push("Line range \"to\" must be a positive integer".to_string());
        }
    }
    if let (Some(f), Some(t)) = (from.and_then(Value::as_f64), to.and_then(Value::as_f64)) {
        if f > t {
            errors.push("Line range \"from\" must be less than or equal to \"to\"".to_string());
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn r(from: Option<f64>, to: Option<f64>) -> LineRange {
        LineRange { from, to }
    }

    #[test]
    fn filter() {
        let c = "a\nb\nc\nd";
        assert_eq!(filter_by_line_range(c, None), c);
        assert_eq!(
            filter_by_line_range(c, Some(&r(Some(2.0), Some(3.0)))),
            "b\nc"
        );
        assert_eq!(filter_by_line_range(c, Some(&r(Some(3.0), None))), "c\nd");
        assert_eq!(filter_by_line_range(c, Some(&r(None, Some(1.0)))), "a");
        assert_eq!(
            filter_by_line_range(c, Some(&r(Some(2.0), Some(99.0)))),
            "b\nc\nd"
        );
        assert_eq!(filter_by_line_range(c, Some(&r(Some(9.0), Some(10.0)))), "");
    }

    #[test]
    fn info() {
        assert_eq!(get_line_range_info(4, None), "");
        assert_eq!(
            get_line_range_info(4, Some(&r(Some(2.0), Some(9.0)))),
            " (lines 2 to 4)"
        );
        assert_eq!(
            get_line_range_info(4, Some(&r(None, None))),
            " (lines 1 to 4)"
        );
    }

    #[test]
    fn validation() {
        assert!(validate_line_range(None).is_empty());
        assert!(validate_line_range(Some(&json!({"from": 1, "to": 2}))).is_empty());
        assert_eq!(
            validate_line_range(Some(&json!({"from": 0}))),
            vec!["Line range \"from\" must be a positive integer"]
        );
        assert_eq!(
            validate_line_range(Some(&json!({"from": 5, "to": 2}))),
            vec!["Line range \"from\" must be less than or equal to \"to\""]
        );
        assert_eq!(
            validate_line_range(Some(&json!({"to": "x"}))),
            vec!["Line range \"to\" must be a positive integer"]
        );
    }
}
