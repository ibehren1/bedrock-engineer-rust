//! Naming helpers for sandboxes. Port of `src/main/api/docker/naming.ts` plus the folder
//! helpers it re-exports from `src/main/lib/chatFolderNaming.ts`.
//!
//! These names must match the Electron build byte for byte: the compose project name is how
//! compose finds containers by label, and the folder short id is how a sandbox folder is
//! recovered for a session.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use sha1::{Digest, Sha1};
use unicode_normalization::UnicodeNormalization;

/// Longest slug taken from a chat title, so folder names stay manageable.
const MAX_SLUG_LENGTH: usize = 40;

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("valid regex"))
}

/// Compose project names must be lowercase and cannot contain characters Docker rejects.
/// Chat session ids look like `session_1756900000000`, so the underscore is replaced.
///
/// Keyed on the session id alone and never changes, so compose keeps tracking a sandbox's
/// containers by label even after its folder is renamed.
pub fn to_project_name(session_id: &str) -> String {
    static INVALID: OnceLock<Regex> = OnceLock::new();
    static DASHES: OnceLock<Regex> = OnceLock::new();
    let lowered = session_id.to_lowercase();
    let replaced = regex(&INVALID, "[^a-z0-9-]+").replace_all(&lowered, "-");
    let name = format!("bedrock-sandbox-{replaced}");
    regex(&DASHES, "-+").replace_all(&name, "-").into_owned()
}

/// Container name compose gives a service, and the name the composeless path assigns.
pub fn container_name_for(project_name: &str, service: &str) -> String {
    format!("{project_name}-{service}-1")
}

/// Short, stable discriminator derived from the session id: first six hex chars of SHA-1.
pub fn to_short_id(session_id: &str) -> String {
    let digest = Sha1::digest(session_id.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex[..6].to_string()
}

/// Turn a chat title into a filesystem-safe slug. Empty when nothing usable remains.
pub fn slugify_title(title: &str) -> String {
    static NON_ALNUM: OnceLock<Regex> = OnceLock::new();
    let decomposed: String = title
        .to_lowercase()
        .nfkd()
        // Drop combining marks so accented characters reduce to their base letter.
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();
    let dashed = regex(&NON_ALNUM, "[^a-z0-9]+").replace_all(&decomposed, "-");
    let trimmed = dashed.trim_matches('-');
    // Everything left is ASCII, so byte slicing is character slicing.
    let capped = &trimmed[..trimmed.len().min(MAX_SLUG_LENGTH)];
    capped.trim_end_matches('-').to_string()
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

/// True when a chat still carries the title it was created with (`Chat <locale date>`).
pub fn is_default_chat_title(title: Option<&str>) -> bool {
    match title {
        None => true,
        Some(title) => title.is_empty() || title.starts_with("Chat "),
    }
}

/// Find the folder a chat owns inside `root` by its `-<shortId>` suffix.
pub fn find_chat_folder_by_short_id(root: &Path, session_id: &str) -> Option<PathBuf> {
    let suffix = format!("-{}", to_short_id(session_id));
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        if entry.file_name().to_string_lossy().ends_with(&suffix) {
            return Some(root.join(entry.file_name()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! Port of `naming.test.ts`.
    use super::*;

    // describe('toProjectName')
    #[test]
    fn converts_a_chat_session_id_into_a_valid_compose_project_name() {
        assert_eq!(
            to_project_name("session_1756900000000"),
            "bedrock-sandbox-session-1756900000000"
        );
    }

    #[test]
    fn lowercases_and_collapses_characters_compose_would_reject() {
        assert_eq!(
            to_project_name("subagent-Chat-Agent_99"),
            "bedrock-sandbox-subagent-chat-agent-99"
        );
    }

    #[test]
    fn never_emits_consecutive_dashes() {
        assert_eq!(to_project_name("a__b--c"), "bedrock-sandbox-a-b-c");
    }

    #[test]
    fn stays_keyed_on_the_session_id() {
        assert_eq!(to_project_name("session_1"), to_project_name("session_1"));
    }

    // describe('containerNameFor')
    #[test]
    fn matches_the_name_compose_assigns_to_the_first_replica() {
        assert_eq!(
            container_name_for("bedrock-sandbox-session-1", "main"),
            "bedrock-sandbox-session-1-main-1"
        );
    }

    // describe('toShortId')
    #[test]
    fn short_id_is_stable() {
        assert_eq!(
            to_short_id("session_1756900000000"),
            to_short_id("session_1756900000000")
        );
    }

    #[test]
    fn short_id_differs_between_session_ids() {
        assert_ne!(to_short_id("session_1"), to_short_id("session_2"));
    }

    #[test]
    fn short_id_is_six_hex_characters() {
        let id = to_short_id("session_1");
        assert_eq!(id.len(), 6);
        assert!(id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn short_id_matches_node_sha1() {
        // crypto.createHash('sha1').update('session_1').digest('hex').slice(0, 6)
        let full = Sha1::digest(b"session_1");
        assert_eq!(
            to_short_id("session_1"),
            format!("{:02x}{:02x}{:02x}", full[0], full[1], full[2])
        );
    }

    // describe('slugifyTitle')
    #[test]
    fn slugifies_titles() {
        for (title, expected) in [
            ("Fix the auth bug", "fix-the-auth-bug"),
            ("  Padded  Title  ", "padded-title"),
            ("Refactor: parse/format!", "refactor-parse-format"),
            ("Multiple   spaces", "multiple-spaces"),
        ] {
            assert_eq!(slugify_title(title), expected, "slugifies {title}");
        }
    }

    #[test]
    fn reduces_accented_characters_to_their_base_letters() {
        assert_eq!(slugify_title("Café déjà vu"), "cafe-deja-vu");
    }

    #[test]
    fn returns_an_empty_string_when_nothing_usable_remains() {
        assert_eq!(slugify_title("日本語"), "");
        assert_eq!(slugify_title("!!!"), "");
        assert_eq!(slugify_title(""), "");
    }

    #[test]
    fn caps_the_length_and_never_leaves_a_trailing_dash() {
        let slug = slugify_title(&"a".repeat(80));
        assert_eq!(slug.len(), 40);

        let truncated = slugify_title(&format!("{} tail", "b".repeat(39)));
        assert!(!truncated.ends_with('-'));
    }

    // describe('toFolderName')
    #[test]
    fn combines_the_title_slug_with_a_short_id() {
        let short_id = to_short_id("session_1756900000000");
        assert_eq!(
            to_folder_name("session_1756900000000", Some("Fix the auth bug")),
            format!("fix-the-auth-bug-{short_id}")
        );
    }

    #[test]
    fn falls_back_to_a_session_prefixed_name_when_there_is_no_title() {
        let short_id = to_short_id("session_1");
        assert_eq!(
            to_folder_name("session_1", None),
            format!("session-{short_id}")
        );
    }

    #[test]
    fn falls_back_when_the_title_produces_no_usable_slug() {
        let short_id = to_short_id("session_1");
        assert_eq!(
            to_folder_name("session_1", Some("日本語")),
            format!("session-{short_id}")
        );
    }

    #[test]
    fn keeps_two_chats_with_the_same_title_in_separate_folders() {
        assert_ne!(
            to_folder_name("session_1", Some("Same title")),
            to_folder_name("session_2", Some("Same title"))
        );
    }

    // describe('isDefaultChatTitle')
    #[test]
    fn treats_the_generated_chat_date_title_as_not_yet_named() {
        assert!(is_default_chat_title(Some("Chat 9/3/2026, 2:22:15 PM")));
    }

    #[test]
    fn treats_a_missing_title_as_not_yet_named() {
        assert!(is_default_chat_title(None));
        assert!(is_default_chat_title(Some("")));
    }

    #[test]
    fn treats_a_real_title_as_named() {
        assert!(!is_default_chat_title(Some("Fix the auth bug")));
    }

    #[test]
    fn does_not_mistake_a_real_title_that_merely_contains_chat() {
        assert!(!is_default_chat_title(Some("Rework the Chat page layout")));
    }

    #[test]
    fn finds_a_chat_folder_by_short_id() {
        let root = tempfile::tempdir().unwrap();
        let name = to_folder_name("session_1", Some("Hello"));
        std::fs::create_dir(root.path().join(&name)).unwrap();
        assert_eq!(
            find_chat_folder_by_short_id(root.path(), "session_1"),
            Some(root.path().join(name))
        );
        assert_eq!(find_chat_folder_by_short_id(root.path(), "session_2"), None);
    }
}
