//! Naming helpers for per-chat folders inside the project directory.
//!
//! A chat's folder is `chats/<chat-name>/`, where the name is the chat title in lowercase words
//! joined by hyphens ([`to_chat_folder_name`]), so it reads like the title and follows it when the
//! title changes. Ownership is recorded by a marker file inside the folder, not by the name.
//!
//! The older layout (`attachments/<title-slug>-<shortId>/`, port of
//! `src/main/lib/chatFolderNaming.ts`, where `shortId` is the first 6 hex chars of
//! SHA-1(sessionId)) is still named here so existing folders can be found and moved.

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

/// Longest chat folder name taken from a title, in characters.
const MAX_CHAT_FOLDER_NAME: usize = 60;

/// Names Windows refuses for a file or folder, whatever the extension.
const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A chat title as a folder name: lowercase words (letters and digits of any script) joined by
/// single hyphens, accents dropped, at most 60 characters. Empty when the title has no letters
/// or digits.
pub fn chat_title_to_folder_slug(title: &str) -> String {
    // Decompose to drop accents, then recompose so kana voicing marks stay on their letter.
    let stripped: String = title
        .nfkd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();
    let mut slug = String::with_capacity(stripped.len());
    let mut pending_dash = false;
    let mut count = 0;
    for c in stripped.nfkc().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                if count + 1 >= MAX_CHAT_FOLDER_NAME {
                    break;
                }
                slug.push('-');
                count += 1;
            }
            pending_dash = false;
            if count >= MAX_CHAT_FOLDER_NAME {
                break;
            }
            slug.push(c);
            count += 1;
        } else {
            pending_dash = true;
        }
    }
    if WINDOWS_RESERVED.contains(&slug.as_str()) {
        slug.push_str("-chat");
    }
    slug
}

/// The folder name a chat's title gives: [`chat_title_to_folder_slug`], or `chat-<shortId>`
/// for a chat with no usable title. Another chat may already have the name; see the manager
/// for how a clash is numbered.
pub fn to_chat_folder_name(session_id: &str, title: Option<&str>) -> String {
    let slug = title.map(chat_title_to_folder_slug).unwrap_or_default();
    if slug.is_empty() {
        format!("chat-{}", to_short_id(session_id))
    } else {
        slug
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
    fn chat_folder_names_read_like_the_title() {
        assert_eq!(
            chat_title_to_folder_slug("Textract Sub-Second OCR Pipeline Support"),
            "textract-sub-second-ocr-pipeline-support"
        );
        assert_eq!(
            chat_title_to_folder_slug("  Résumé -- Final!! "),
            "resume-final"
        );
        assert_eq!(
            chat_title_to_folder_slug("Nasdaq/Verafin: Q3"),
            "nasdaq-verafin-q3"
        );
        assert_eq!(
            chat_title_to_folder_slug("仕様書のレビュー"),
            "仕様書のレビュー"
        );
        assert_eq!(
            chat_title_to_folder_slug("ｆｕｌｌ ｗｉｄｔｈ"),
            "full-width"
        );
        assert_eq!(chat_title_to_folder_slug("???"), "");
        assert_eq!(chat_title_to_folder_slug("CON"), "con-chat");
        let long = chat_title_to_folder_slug(&"word ".repeat(30));
        assert!(long.chars().count() <= 60, "{long}");
        assert!(!long.ends_with('-'));
    }

    #[test]
    fn untitled_chats_get_a_short_id_name() {
        let short = to_short_id("s");
        assert_eq!(to_chat_folder_name("s", None), format!("chat-{short}"));
        assert_eq!(
            to_chat_folder_name("s", Some("!!")),
            format!("chat-{short}")
        );
        assert_eq!(to_chat_folder_name("s", Some("My Chat")), "my-chat");
    }

    #[test]
    fn default_title_detection() {
        assert!(is_default_chat_title(None));
        assert!(is_default_chat_title(Some("Chat 9/30/2026")));
        assert!(!is_default_chat_title(Some("Fix bug")));
    }
}
