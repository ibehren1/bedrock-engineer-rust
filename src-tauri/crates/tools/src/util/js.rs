//! JavaScript-compatible string helpers.
//!
//! The TS tools measure and slice strings in UTF-16 code units (`String.prototype.length`,
//! `slice`). Token estimates and truncation points depend on those counts, so they are
//! reproduced here. Slices never split a surrogate pair: a cut that would land inside one
//! is moved to the preceding character boundary.

use serde_json::Value;

/// `str.length` in JavaScript: the number of UTF-16 code units.
pub fn len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Byte offset of the given UTF-16 offset, clamped to the string and rounded down to a
/// char boundary.
fn byte_offset(s: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    for (idx, ch) in s.char_indices() {
        let next = units + ch.len_utf16();
        if next > utf16_offset {
            return idx;
        }
        units = next;
    }
    s.len()
}

/// `str.slice(start, end)` with UTF-16 offsets (both clamped).
pub fn slice(s: &str, start: usize, end: usize) -> &str {
    if end <= start {
        return "";
    }
    let b_start = byte_offset(s, start);
    let b_end = byte_offset(s, end);
    &s[b_start..b_end.max(b_start)]
}

/// UTF-16 index of a byte index (for converting `find` results).
pub fn utf16_index(s: &str, byte_idx: usize) -> usize {
    len(&s[..byte_idx])
}

/// JavaScript truthiness of a JSON value (`undefined` is represented by `None`).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Number formatting as `Number.prototype.toLocaleString()` in an en-US locale for
/// integers (thousands separators).
pub fn to_locale_string(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `new Date().toISOString()`.
pub fn iso_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// `Date.now()`.
pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// `process.platform`.
pub fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn utf16_len_and_slice() {
        assert_eq!(len("abc"), 3);
        assert_eq!(len("😀"), 2);
        assert_eq!(slice("hello", 1, 3), "el");
        assert_eq!(slice("a😀b", 0, 3), "a😀");
        // A cut inside the surrogate pair rounds down.
        assert_eq!(slice("a😀b", 0, 2), "a");
        assert_eq!(slice("abc", 2, 100), "c");
        assert_eq!(slice("abc", 5, 10), "");
    }

    #[test]
    fn truthiness() {
        assert!(!truthy(None));
        assert!(!truthy(Some(&json!(null))));
        assert!(!truthy(Some(&json!(""))));
        assert!(!truthy(Some(&json!(0))));
        assert!(!truthy(Some(&json!(false))));
        assert!(truthy(Some(&json!("x"))));
        assert!(truthy(Some(&json!([]))));
        assert!(truthy(Some(&json!(1))));
    }

    #[test]
    fn locale_string() {
        assert_eq!(to_locale_string(0), "0");
        assert_eq!(to_locale_string(999), "999");
        assert_eq!(to_locale_string(1000), "1,000");
        assert_eq!(to_locale_string(1234567), "1,234,567");
    }
}
