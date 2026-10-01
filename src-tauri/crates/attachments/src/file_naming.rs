//! Filesystem-safe attachment names (port of `fileNaming.ts`), plus the small Node `path`
//! helpers whose exact semantics the naming rules depend on.

use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// Node's `path.basename(p)` for the host platform: trailing separators are ignored and the
/// last segment is returned.
pub(crate) fn node_basename(p: &str) -> &str {
    let trimmed = p.trim_end_matches(is_separator);
    if trimmed.is_empty() {
        return "";
    }
    match trimmed.rfind(is_separator) {
        Some(index) => &trimmed[index + 1..],
        None => trimmed,
    }
}

/// Node's `path.extname(name)` on a bare file name: from the last `.` to the end, unless that
/// dot is the first character (a dotfile has no extension).
pub(crate) fn node_extname(name: &str) -> &str {
    let base = node_basename(name);
    if base == ".." {
        return "";
    }
    match base.rfind('.') {
        Some(0) | None => "",
        Some(index) => &base[index..],
    }
}

/// Node's `path.basename(name, ext)`: the file name with `ext` removed when it ends in it.
fn stem_of<'a>(name: &'a str, ext: &str) -> &'a str {
    let base = node_basename(name);
    if !ext.is_empty() && base != ext {
        if let Some(stem) = base.strip_suffix(ext) {
            return stem;
        }
    }
    base
}

/// Longest prefix of `text` that fits in `limit` UTF-16 code units (JS `slice(0, limit)`,
/// without splitting a surrogate pair). Returns the prefix and its UTF-16 length.
pub(crate) fn utf16_prefix(text: &str, limit: usize) -> (&str, usize) {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        let width = ch.len_utf16();
        if units + width > limit {
            return (&text[..index], units);
        }
        units += width;
    }
    (text, units)
}

/// Length of `text` in UTF-16 code units (JS `string.length`).
pub(crate) fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

const RESERVED_NAME_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Filesystem-safe attachment name.
///
/// Unicode letters are kept (`仕様書.pdf` survives). Only characters reserved on some platform,
/// control characters and path separators are removed; leading dots (hidden files) and trailing
/// dots (invalid on Windows) are dropped; the stem is capped at 100 characters.
pub fn sanitize_attachment_name(file_name: &str) -> String {
    let without_reserved: String = node_basename(file_name)
        .chars()
        .filter(|c| !RESERVED_NAME_CHARS.contains(c) && (*c as u32) > 0x1f)
        .collect();

    // Collapse whitespace runs to a single space, then trim.
    let mut collapsed = String::with_capacity(without_reserved.len());
    let mut in_space = false;
    for c in without_reserved.chars() {
        if c.is_whitespace() {
            if !in_space {
                collapsed.push(' ');
            }
            in_space = true;
        } else {
            collapsed.push(c);
            in_space = false;
        }
    }
    let base = collapsed
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('.');

    if base.is_empty() {
        return "attachment".to_string();
    }

    let ext = node_extname(base);
    let (stem, _) = utf16_prefix(stem_of(base, ext), 100);
    let stem = if stem.is_empty() { "attachment" } else { stem };
    format!("{stem}{ext}")
}

/// Avoid clobbering an existing file by adding a numeric suffix (`a.txt` -> `a-1.txt`).
pub fn unique_name_in(directory: &Path, file_name: &str) -> String {
    let ext = node_extname(file_name);
    let stem = stem_of(file_name, ext);

    let mut candidate = file_name.to_string();
    let mut count = 1;
    while directory.join(&candidate).exists() {
        candidate = format!("{stem}-{count}{ext}");
        count += 1;
    }
    candidate
}

/// Lexical `path.resolve`: absolute (against the cwd) with `.` and `..` folded away.
pub(crate) fn resolve_lexically(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Absolute path of a direct child of `directory`, refusing anything that would reach outside
/// it — including the folder itself, nested paths and absolute paths.
pub fn resolve_inside(directory: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name != node_basename(name) {
        return Err(Error::InvalidName(name.to_string()));
    }
    // With separators ruled out above, only `.` and `..` can resolve to something other than a
    // direct child.
    if name == "." || name == ".." {
        return Err(Error::OutsideFolder(name.to_string()));
    }
    Ok(resolve_lexically(directory).join(name))
}

#[cfg(test)]
mod tests {
    //! Port of `fileNaming.test.ts`.
    use super::*;

    mod sanitize_attachment_name {
        use super::*;

        #[test]
        fn keeps_an_ordinary_name_as_it_is() {
            assert_eq!(sanitize_attachment_name("report.pdf"), "report.pdf");
        }

        #[test]
        fn keeps_non_ascii_names_readable() {
            assert_eq!(sanitize_attachment_name("仕様書.pdf"), "仕様書.pdf");
            assert_eq!(
                sanitize_attachment_name("résumé final.docx"),
                "résumé final.docx"
            );
        }

        #[test]
        fn strips_reserved_characters_and_control_characters() {
            assert_eq!(sanitize_attachment_name("in:va*lid?.txt"), "invalid.txt");
            assert_eq!(sanitize_attachment_name("bad\u{7}name.txt"), "badname.txt");
        }

        #[test]
        fn drops_any_directory_part() {
            assert_eq!(sanitize_attachment_name("/etc/passwd"), "passwd");
            assert_eq!(sanitize_attachment_name("../../secret.txt"), "secret.txt");
        }

        #[test]
        fn refuses_to_produce_a_hidden_name_or_a_trailing_dot() {
            assert_eq!(sanitize_attachment_name(".env"), "env");
            assert_eq!(sanitize_attachment_name("notes."), "notes");
        }

        #[test]
        fn falls_back_when_nothing_usable_is_left() {
            assert_eq!(sanitize_attachment_name(""), "attachment");
            assert_eq!(sanitize_attachment_name(".."), "attachment");
            assert_eq!(sanitize_attachment_name("???"), "attachment");
        }

        #[test]
        fn caps_a_very_long_stem_but_keeps_the_extension() {
            let result = sanitize_attachment_name(&format!("{}.txt", "a".repeat(300)));
            assert_eq!(result, format!("{}.txt", "a".repeat(100)));
        }
    }

    mod unique_name_in {
        use super::*;

        #[test]
        fn returns_the_name_unchanged_when_nothing_collides() {
            let dir = tempfile::tempdir().unwrap();
            assert_eq!(unique_name_in(dir.path(), "a.txt"), "a.txt");
        }

        #[test]
        fn adds_an_increasing_suffix_rather_than_clobbering_a_file() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("a.txt"), "one").unwrap();
            assert_eq!(unique_name_in(dir.path(), "a.txt"), "a-1.txt");

            std::fs::write(dir.path().join("a-1.txt"), "two").unwrap();
            assert_eq!(unique_name_in(dir.path(), "a.txt"), "a-2.txt");
        }
    }

    mod resolve_inside {
        use super::*;

        fn directory() -> PathBuf {
            std::env::temp_dir().join("attachments-resolve")
        }

        #[test]
        fn accepts_a_plain_child_name() {
            assert_eq!(
                resolve_inside(&directory(), "file.txt").unwrap(),
                resolve_lexically(&directory()).join("file.txt")
            );
        }

        #[test]
        fn refuses_escapes() {
            for name in [
                "../escape.txt",
                "nested/file.txt",
                "/etc/passwd",
                ".",
                "",
                "..",
            ] {
                assert!(
                    resolve_inside(&directory(), name).is_err(),
                    "expected {name:?} to be refused"
                );
            }
        }
    }

    #[test]
    fn node_path_helpers_match_node() {
        assert_eq!(node_extname("a.b.c"), ".c");
        assert_eq!(node_extname(".env"), "");
        assert_eq!(node_extname("file"), "");
        assert_eq!(node_basename("foo/"), "foo");
        assert_eq!(stem_of("a.txt", ".txt"), "a");
    }
}
