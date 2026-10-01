//! Small helpers standing in for Node built-ins (`path.resolve`, `path.relative`,
//! `new Date().toISOString()`, streaming `Buffer#toString`).

use std::path::{Component, Path, PathBuf};

/// `new Date().toISOString()`: UTC, millisecond precision, `Z` suffix.
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// `Date.parse` for the ISO strings produced by [`now_iso`]; epoch milliseconds.
pub(crate) fn parse_iso_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.timestamp_millis())
}

/// `Date.now()`.
pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Lexically normalize a path (`.`/`..` collapsed), like Node's `path.normalize`.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Never pop past the root/prefix, matching `path.resolve('/..') === '/'`.
                let popped = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if popped {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Node's `path.resolve(base, target)`: absolute targets win, relative ones join onto
/// `base`, and a relative base is anchored at the current directory.
pub(crate) fn resolve(base: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        base.join(target)
    };
    let absolute = if joined.is_absolute() {
        joined
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&joined))
            .unwrap_or(joined)
    };
    normalize(&absolute)
}

/// `path.resolve(p)`.
pub(crate) fn resolve_one(path: &Path) -> PathBuf {
    resolve(Path::new(""), path)
}

/// Node's `path.relative(from, to)` over lexically resolved paths.
pub(crate) fn relative(from: &Path, to: &Path) -> PathBuf {
    let from = resolve_one(from);
    let to = resolve_one(to);
    let from_parts: Vec<Component> = from.components().collect();
    let to_parts: Vec<Component> = to.components().collect();

    let common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Different roots/prefixes (another drive on Windows): Node returns `to` itself.
    if common == 0 && from.has_root() {
        return to;
    }

    let mut out = PathBuf::new();
    for _ in common..from_parts.len() {
        out.push("..");
    }
    for part in &to_parts[common..] {
        out.push(part.as_os_str());
    }
    out
}

/// `isInside` from `src/main/lib/pathSafety.ts`: `target` is `parent` or beneath it.
pub(crate) fn is_inside(target: &Path, parent: &Path) -> bool {
    let rel = relative(parent, target);
    let text = rel.to_string_lossy();
    text.is_empty() || (!text.starts_with("..") && !rel.is_absolute())
}

/// Decodes a byte stream into a `String` chunk by chunk without splitting a multi-byte
/// character across chunks (what Node's `data.toString()` gets wrong at boundaries).
#[derive(Default)]
pub(crate) struct Utf8Accumulator {
    pending: Vec<u8>,
}

impl Utf8Accumulator {
    pub(crate) fn push(&mut self, out: &mut String, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    return;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                    match error.error_len() {
                        Some(bad) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..valid + bad);
                        }
                        None => {
                            // Incomplete sequence at the end: keep it for the next chunk.
                            self.pending.drain(..valid);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn finish(&mut self, out: &mut String) {
        if !self.pending.is_empty() {
            out.push_str(&String::from_utf8_lossy(&self.pending));
            self.pending.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn relative_matches_node() {
        assert_eq!(
            relative(
                Path::new("/tmp/example-project/docker-sandboxes/session_1"),
                Path::new("/tmp/example-project/assets")
            ),
            PathBuf::from("../../assets")
        );
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/a/b")),
            PathBuf::new()
        );
        assert!(is_inside(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(is_inside(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!is_inside(Path::new("/"), Path::new("/a/b")));
        assert!(!is_inside(Path::new("/a/bc"), Path::new("/a/b")));
        assert_eq!(
            resolve(Path::new("/a/b"), Path::new("../c")),
            PathBuf::from("/a/c")
        );
    }

    #[test]
    fn utf8_accumulator_keeps_split_characters() {
        let mut out = String::new();
        let mut acc = Utf8Accumulator::default();
        let bytes = "é日".as_bytes();
        acc.push(&mut out, &bytes[..1]);
        acc.push(&mut out, &bytes[1..3]);
        acc.push(&mut out, &bytes[3..]);
        acc.finish(&mut out);
        assert_eq!(out, "é日");
    }
}
