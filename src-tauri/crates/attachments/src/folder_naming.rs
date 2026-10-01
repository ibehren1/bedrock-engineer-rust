//! Naming helpers for per-chat folders inside the project directory (port of
//! `src/main/lib/chatFolderNaming.ts`). Shared by attachments and Docker sandboxes: a folder is
//! `<title-slug>-<shortId>` or `session-<shortId>`, where `shortId` is the first 6 hex chars of
//! SHA-1(sessionId).

use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use unicode_normalization::UnicodeNormalization;

/// Longest slug taken from a chat title.
const MAX_SLUG_LENGTH: usize = 40;

/// Short, stable discriminator derived from the session id.
pub fn to_short_id(session_id: &str) -> String {
    let digest = Sha1::digest(session_id.as_bytes());
    digest[..3].iter().map(|b| format!("{b:02x}")).collect()
}

/// Turn a chat title into a filesystem-safe slug; empty when nothing usable is left.
pub fn slugify_title(title: &str) -> String {
    let decomposed: String = title
        .to_lowercase()
        .nfkd()
        // Drop combining marks so accented characters reduce to their base letter.
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();

    let mut slug = String::with_capacity(decomposed.len());
    let mut pending_dash = false;
    for c in decomposed.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(c);
        } else {
            pending_dash = true;
        }
    }
    let truncated: String = slug.chars().take(MAX_SLUG_LENGTH).collect();
    truncated.trim_end_matches('-').to_string()
}

/// Human-readable folder name for a chat: the chat's title plus a short id.
pub fn to_folder_name(session_id: &str, title: Option<&str>) -> String {
    let slug = title.map(slugify_title).unwrap_or_default();
    let short_id = to_short_id(session_id);
    if slug.is_empty() {
        format!("session-{short_id}")
    } else {
        format!("{slug}-{short_id}")
    }
}

/// True when a chat still carries the title it was created with (`Chat <date>`).
pub fn is_default_chat_title(title: Option<&str>) -> bool {
    match title {
        None => true,
        Some(t) => t.is_empty() || t.starts_with("Chat "),
    }
}

/// Find the first folder inside `root` with the chat's `-<shortId>` suffix.
///
/// The suffix alone does not prove ownership (6 hex chars collide); the attachments manager
/// checks the folder's marker file instead. Kept for callers that only need a best guess.
///
/// Entries are scanned in name order (as Node's `readdirSync` returns them).
pub fn find_chat_folder_by_short_id(root: &Path, session_id: &str) -> Option<PathBuf> {
    let suffix = format!("-{}", to_short_id(session_id));
    let entries = std::fs::read_dir(root).ok()?;
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
        .into_iter()
        .find(|name| name.ends_with(&suffix))
        .map(|name| root.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_is_first_six_hex_of_sha1() {
        // sha1("abc") = a9993e36...
        assert_eq!(to_short_id("abc"), "a9993e");
    }

    #[test]
    fn slugify_matches_the_ts_rules() {
        assert_eq!(slugify_title("Fix the auth bug"), "fix-the-auth-bug");
        assert_eq!(slugify_title("  Résumé -- Final!! "), "resume-final");
        assert_eq!(slugify_title("仕様書"), "");
        let long = slugify_title(&format!("{} tail", "a".repeat(39)));
        assert_eq!(long, "a".repeat(39));
    }

    #[test]
    fn folder_name_falls_back_to_session_prefix() {
        let short = to_short_id("s");
        assert_eq!(to_folder_name("s", None), format!("session-{short}"));
        assert_eq!(to_folder_name("s", Some("???")), format!("session-{short}"));
        assert_eq!(to_folder_name("s", Some("Hello")), format!("hello-{short}"));
    }

    #[test]
    fn default_title_detection() {
        assert!(is_default_chat_title(None));
        assert!(is_default_chat_title(Some("Chat 9/30/2026")));
        assert!(!is_default_chat_title(Some("Fix bug")));
    }
}
