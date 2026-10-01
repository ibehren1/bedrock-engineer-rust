//! `ChatSessionManager` has no TS unit test; these cover the on-disk format against fixtures
//! written by Node's `JSON.stringify` (exactly what the Electron app writes) and the behaviour of
//! each method.

use super::*;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/electron");
const RICH: &str = "session_1790613354839";
const EMPTY: &str = "session_1788988619650";
const NO_PROMPT: &str = "session_1790000000000";
const DELETED: &str = "session_1780000000000";

/// A userData dir holding a copy of the Electron fixtures.
fn electron_user_data() -> TempDir {
    let d = TempDir::new().unwrap();
    let src = Path::new(FIXTURES);
    fs::create_dir_all(d.path().join(SESSIONS_DIR)).unwrap();
    fs::copy(src.join(META_FILE), d.path().join(META_FILE)).unwrap();
    for e in fs::read_dir(src.join(SESSIONS_DIR)).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), d.path().join(SESSIONS_DIR).join(e.file_name())).unwrap();
    }
    d
}

fn read(p: impl AsRef<Path>) -> String {
    fs::read_to_string(p).unwrap()
}

fn fixture(name: &str) -> String {
    read(
        Path::new(FIXTURES)
            .join(SESSIONS_DIR)
            .join(format!("{name}.json")),
    )
}

fn ids(list: &[Value]) -> Vec<&str> {
    list.iter().map(|m| m["id"].as_str().unwrap()).collect()
}

fn message(id: &str) -> Value {
    json!({ "id": id, "role": "user", "content": [{ "text": id }], "timestamp": 1 })
}

// ---------------------------------------------------------------------------------------------
// format
// ---------------------------------------------------------------------------------------------

#[test]
fn loads_electron_files_unchanged() {
    let d = electron_user_data();
    let before_meta = read(d.path().join(META_FILE));
    let m = ChatSessionManager::new(d.path()).unwrap();
    // Opening must not rewrite a complete metadata file.
    assert_eq!(read(d.path().join(META_FILE)), before_meta);

    let s = m.get_session(RICH).unwrap();
    assert_eq!(s["title"], "Refactor the parser — 日本語 ✓");
    assert_eq!(s["messages"].as_array().unwrap().len(), 4);
    assert_eq!(
        s["messages"][1]["metadata"]["converseMetadata"]["usage"]["outputTokens"],
        json!(35)
    );
    assert_eq!(m.get_active_session_id().as_deref(), Some(RICH));
}

#[test]
fn rewrite_is_byte_identical_to_node() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    for id in [RICH, EMPTY, NO_PROMPT] {
        let title = m.get_session(id).unwrap()["title"]
            .as_str()
            .unwrap()
            .to_owned();
        // Same title → same session and metadata content, so the files must match what
        // JSON.stringify wrote byte for byte.
        m.update_session_title(id, &title);
        assert_eq!(
            read(d.path().join(SESSIONS_DIR).join(format!("{id}.json"))),
            fixture(id),
            "{id}"
        );
    }
    assert_eq!(
        read(d.path().join(META_FILE)),
        read(Path::new(FIXTURES).join(META_FILE))
    );
}

#[test]
fn metadata_shape_matches_update_metadata() {
    let s: Value = serde_json::from_str(&fixture(RICH)).unwrap();
    let meta = session_metadata(&s);
    let keys: Vec<&String> = meta.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "id",
            "title",
            "createdAt",
            "updatedAt",
            "messageCount",
            "agentId",
            "modelId",
            "systemPrompt"
        ]
    );
    assert_eq!(meta["messageCount"], 4);
    // An absent systemPrompt stays absent (JSON.stringify drops undefined).
    let s: Value = serde_json::from_str(&fixture(NO_PROMPT)).unwrap();
    assert!(session_metadata(&s).get("systemPrompt").is_none());
}

#[test]
fn fresh_dir_creates_layout() {
    let d = TempDir::new().unwrap();
    ChatSessionManager::new(d.path()).unwrap();
    assert!(d.path().join(SESSIONS_DIR).is_dir());
    assert_eq!(
        read(d.path().join(META_FILE)),
        "{\n\t\"recentSessions\": [],\n\t\"metadata\": {}\n}"
    );
}

#[test]
fn initializes_metadata_from_session_files() {
    let d = electron_user_data();
    fs::remove_file(d.path().join(META_FILE)).unwrap();
    // A stray non-JSON file is ignored.
    fs::write(d.path().join(SESSIONS_DIR).join("notes.txt"), "x").unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let all = m.get_all_session_metadata();
    assert_eq!(ids(&all), [RICH, NO_PROMPT]);
    assert_eq!(m.metadata().len(), 3);
    assert!(m.get_recent_sessions().is_empty());
}

#[test]
fn unreadable_session_is_not_found() {
    let d = electron_user_data();
    fs::write(d.path().join(SESSIONS_DIR).join("session_bad.json"), "{").unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    assert_eq!(m.get_session("session_bad"), None);
    assert_eq!(m.get_session("nope"), None);
    // Mutations on a missing session are silent no-ops.
    m.add_message("nope", message("x"));
    m.update_session_title("nope", "t");
    assert!(!d.path().join(SESSIONS_DIR).join("nope.json").exists());
}

// ---------------------------------------------------------------------------------------------
// listing
// ---------------------------------------------------------------------------------------------

#[test]
fn all_metadata_filters_and_sorts() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    // EMPTY has no messages, DELETED has no file; newest updatedAt first.
    assert_eq!(ids(&m.get_all_session_metadata()), [RICH, NO_PROMPT]);
}

#[test]
fn recent_keeps_recent_order_and_filters() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    assert_eq!(ids(&m.get_recent_sessions()), [RICH, NO_PROMPT]);
}

#[test]
fn snapshot_contains_lists_and_sessions() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let snap = m.snapshot();
    assert_eq!(snap.active_session_id.as_deref(), Some(RICH));
    assert_eq!(snap.metadata, m.get_all_session_metadata());
    assert_eq!(snap.recent, m.get_recent_sessions());
    let keys: Vec<&String> = snap.sessions.keys().collect();
    assert_eq!(keys, [RICH, NO_PROMPT]);
    let v = serde_json::to_value(&snap).unwrap();
    assert!(v.get("activeSessionId").is_some());
}

// ---------------------------------------------------------------------------------------------
// mutations
// ---------------------------------------------------------------------------------------------

#[test]
fn create_session_writes_file_meta_and_recent() {
    let d = TempDir::new().unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let id = m.create_session(
        "agent",
        "model",
        Some("sys"),
        Some("Chat 1/2/2026, 3:04:05 PM"),
    );
    assert!(id.starts_with("session_"));
    let s = m.get_session(&id).unwrap();
    let keys: Vec<&String> = s.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "id",
            "title",
            "createdAt",
            "updatedAt",
            "messages",
            "agentId",
            "modelId",
            "systemPrompt"
        ]
    );
    assert_eq!(s["title"], "Chat 1/2/2026, 3:04:05 PM");
    assert_eq!(s["messages"], json!([]));
    assert_eq!(m.metadata()[&id]["messageCount"], 0);
    assert_eq!(m.recent_ids(), std::slice::from_ref(&id));
    // Empty sessions are hidden from both lists.
    assert!(m.get_all_session_metadata().is_empty());
    assert!(m.get_recent_sessions().is_empty());

    let text = read(d.path().join(SESSIONS_DIR).join(format!("{id}.json")));
    assert!(text.starts_with("{\n  \"id\": "), "{text}");
    assert!(!text.ends_with('\n'));

    let none = m.create_session("a", "m", None, None);
    let s = m.get_session(&none).unwrap();
    assert!(s.get("systemPrompt").is_none());
    assert!(s["title"].as_str().unwrap().starts_with("Chat "));
}

#[test]
fn default_title_is_en_us_locale_string() {
    let t = default_session_title();
    let re_ok = t.starts_with("Chat ")
        && t.contains('/')
        && t.contains(", ")
        && (t.ends_with(" AM") || t.ends_with(" PM"));
    assert!(re_ok, "{t}");
}

#[test]
fn add_message_appends_and_moves_to_front() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let before = m.get_session(NO_PROMPT).unwrap()["updatedAt"]
        .as_i64()
        .unwrap();
    m.add_message(NO_PROMPT, message("m2"));
    let s = m.get_session(NO_PROMPT).unwrap();
    assert_eq!(s["messages"].as_array().unwrap().len(), 2);
    assert_eq!(s["messages"][1]["id"], "m2");
    assert!(s["updatedAt"].as_i64().unwrap() > before);
    assert_eq!(m.metadata()[NO_PROMPT]["messageCount"], 2);
    assert_eq!(m.metadata()[NO_PROMPT]["updatedAt"], s["updatedAt"]);
    assert_eq!(m.recent_ids()[0], NO_PROMPT);
    assert_eq!(m.recent_ids().len(), 4);
    // Metadata entry keeps its position in the object.
    let keys: Vec<String> = m.metadata().keys().cloned().collect();
    assert_eq!(keys, [EMPTY, NO_PROMPT, RICH, DELETED]);
    // Unknown message fields survive the round trip.
    assert_eq!(
        s["messages"][0],
        serde_json::from_str::<Value>(&fixture(NO_PROMPT)).unwrap()["messages"][0]
    );
}

#[test]
fn recent_list_is_capped_at_ten() {
    let d = TempDir::new().unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let mut created = Vec::new();
    for i in 0..12 {
        // Ids come from Date.now(); make them unique.
        let id = m.create_session("a", "m", None, Some(&format!("t{i}")));
        std::thread::sleep(std::time::Duration::from_millis(2));
        created.push(id);
    }
    let recent = m.recent_ids();
    assert_eq!(recent.len(), MAX_RECENT);
    assert_eq!(recent[0], created[11]);
    assert_eq!(recent[9], created[2]);
}

#[test]
fn update_title_keeps_updated_at_and_recent() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.update_session_title(NO_PROMPT, "Renamed");
    let s = m.get_session(NO_PROMPT).unwrap();
    assert_eq!(s["title"], "Renamed");
    assert_eq!(s["updatedAt"], 1790000100000_i64);
    assert_eq!(m.metadata()[NO_PROMPT]["title"], "Renamed");
    assert_eq!(m.recent_ids()[0], RICH);
}

#[test]
fn update_and_delete_message() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.update_message_content(RICH, 0, message("replaced"));
    let s = m.get_session(RICH).unwrap();
    assert_eq!(s["messages"][0]["id"], "replaced");
    assert!(s["updatedAt"].as_i64().unwrap() > 1790613999001);
    assert_eq!(m.recent_ids()[0], RICH);

    m.delete_message(RICH, 1);
    let s = m.get_session(RICH).unwrap();
    let msg_ids: Vec<&str> = s["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert_eq!(msg_ids, ["replaced", "msg_a3", "msg_a4"]);
    assert_eq!(m.metadata()[RICH]["messageCount"], 3);
}

#[test]
fn invalid_message_index_is_noop() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    for idx in [-1, 4, 100] {
        m.update_message_content(RICH, idx, message("x"));
        m.delete_message(RICH, idx);
    }
    assert_eq!(
        read(d.path().join(SESSIONS_DIR).join(format!("{RICH}.json"))),
        fixture(RICH)
    );
}

#[test]
fn delete_session_removes_file_meta_and_recent_not_active() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.delete_session(RICH);
    assert!(!d
        .path()
        .join(SESSIONS_DIR)
        .join(format!("{RICH}.json"))
        .exists());
    assert!(!m.metadata().contains_key(RICH));
    assert!(!m.recent_ids().contains(&RICH.to_string()));
    // deleteSession leaves activeSessionId alone.
    assert_eq!(m.get_active_session_id().as_deref(), Some(RICH));
}

#[test]
fn delete_session_without_file_keeps_metadata() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.delete_session(DELETED);
    // unlink failed, so the metadata entry stays; the recent list is still cleaned.
    assert!(m.metadata().contains_key(DELETED));
    assert!(!m.recent_ids().contains(&DELETED.to_string()));
}

#[test]
fn delete_sessions_bulk() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.delete_sessions(&[]);
    assert_eq!(m.metadata().len(), 4);

    m.delete_sessions(&[RICH.into(), DELETED.into(), RICH.into()]);
    assert!(!d
        .path()
        .join(SESSIONS_DIR)
        .join(format!("{RICH}.json"))
        .exists());
    // Metadata goes even when the file was already missing.
    let keys: Vec<String> = m.metadata().keys().cloned().collect();
    assert_eq!(keys, [EMPTY, NO_PROMPT]);
    assert_eq!(m.recent_ids(), [NO_PROMPT, EMPTY]);
    assert_eq!(m.get_active_session_id(), None);
}

#[test]
fn delete_sessions_keeps_other_active() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.delete_sessions(&[NO_PROMPT.into()]);
    assert_eq!(m.get_active_session_id().as_deref(), Some(RICH));
}

#[test]
fn delete_all_sessions() {
    let d = electron_user_data();
    fs::write(d.path().join(SESSIONS_DIR).join("keep.txt"), "x").unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.delete_all_sessions();
    let left: Vec<String> = fs::read_dir(d.path().join(SESSIONS_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["keep.txt"]);
    assert_eq!(
        read(d.path().join(META_FILE)),
        "{\n\t\"recentSessions\": [],\n\t\"metadata\": {}\n}"
    );
    assert!(m.get_all_session_metadata().is_empty());
}

#[test]
fn active_session_set_get_clear() {
    let d = TempDir::new().unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    assert_eq!(m.get_active_session_id(), None);
    m.set_active_session(Some("session_1"));
    assert_eq!(m.get_active_session_id().as_deref(), Some("session_1"));
    m.set_active_session(None);
    assert_eq!(m.get_active_session_id(), None);
}

#[test]
fn read_chat_title_reads_session_file() {
    let d = electron_user_data();
    assert_eq!(
        read_chat_title(d.path(), RICH).as_deref(),
        Some("Refactor the parser — 日本語 ✓")
    );
    assert_eq!(read_chat_title(d.path(), "missing"), None);
}

/// Round-trips every session file in a real Electron `chat-sessions` directory through
/// parse → serialize and checks the bytes are unchanged. Read-only.
///
/// `HISTORY_ROUNDTRIP_DIR="$HOME/Library/Application Support/Bedrock Engineer/chat-sessions" \
///  cargo test -p history -- --ignored real_electron`
#[test]
#[ignore]
fn real_electron_files_round_trip() {
    let dir = std::env::var("HISTORY_ROUNDTRIP_DIR").expect("set HISTORY_ROUNDTRIP_DIR");
    let mut checked = 0;
    for e in fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let text = read(&p);
        let v: Value = serde_json::from_str(&text).unwrap();
        let out = String::from_utf8(meta::to_json(&v, b"  ").unwrap()).unwrap();
        assert!(out == text, "round trip differs: {}", p.display());
        checked += 1;
    }
    let meta_path = Path::new(&dir).parent().unwrap().join(META_FILE);
    if meta_path.exists() {
        let text = read(&meta_path);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert!(String::from_utf8(meta::to_json(&v, b"\t").unwrap()).unwrap() == text);
    }
    eprintln!("round-tripped {checked} session files");
}

#[test]
fn session_with_lone_surrogate_loads_and_updates() {
    let d = electron_user_data();
    let path = d.path().join(SESSIONS_DIR).join("session_42.json");
    fs::write(
        &path,
        r#"{"id":"session_42","title":"half \ud83d","createdAt":1,"updatedAt":1,"messages":[],"agentId":"a","modelId":"m"}"#,
    )
    .unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let s = m.get_session("session_42").unwrap();
    assert_eq!(s["title"], json!("half \u{fffd}"));
    m.add_message("session_42", message("x"));
    assert_eq!(
        m.get_session("session_42").unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        read_chat_title(d.path(), "session_42").as_deref(),
        Some("half \u{fffd}")
    );
}

#[test]
fn unparseable_session_is_left_untouched() {
    let d = electron_user_data();
    let path = d.path().join(SESSIONS_DIR).join("session_43.json");
    fs::write(&path, "{broken").unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    m.add_message("session_43", message("x"));
    m.update_session_title("session_43", "t");
    assert_eq!(read(&path), "{broken");
}

#[test]
fn unsafe_session_ids_address_no_files() {
    let d = electron_user_data();
    let outside = d.path().join("victim.json");
    fs::write(&outside, r#"{"id":"v","messages":[]}"#).unwrap();
    let m = ChatSessionManager::new(d.path()).unwrap();
    for id in ["../victim", "..", ".hidden", "a/b", ""] {
        assert!(m.get_session(id).is_none(), "{id}");
        m.add_message(id, message("x"));
        m.update_session_title(id, "t");
        m.delete_session(id);
        assert!(read_chat_title(d.path(), id).is_none());
    }
    m.delete_sessions(&["../victim".to_string()]);
    assert_eq!(read(&outside), r#"{"id":"v","messages":[]}"#);
}

#[test]
fn metadata_updates_keep_entries_written_by_another_process() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let meta_path = d.path().join(META_FILE);
    let mut v: Value = serde_json::from_str(&read(&meta_path)).unwrap();
    v["metadata"]["session_ext"] = json!({"id": "session_ext", "messageCount": 1});
    fs::write(&meta_path, serde_json::to_string(&v).unwrap()).unwrap();
    let id = m.create_session("a", "m", None, Some("t"));
    let v: Value = serde_json::from_str(&read(&meta_path)).unwrap();
    assert!(v["metadata"]["session_ext"].is_object());
    assert!(v["metadata"][&id].is_object());
}

#[test]
fn metadata_read_error_skips_the_write() {
    let d = electron_user_data();
    let m = ChatSessionManager::new(d.path()).unwrap();
    let meta_path = d.path().join(META_FILE);
    fs::remove_file(&meta_path).unwrap();
    fs::create_dir(&meta_path).unwrap();
    m.create_session("a", "m", None, Some("t"));
    m.delete_sessions(&[RICH.to_string()]);
    assert!(
        meta_path.is_dir(),
        "an unreadable metadata file is never replaced"
    );
}

#[test]
fn new_returns_an_error_on_a_metadata_read_error_and_keeps_the_data() {
    let d = TempDir::new().unwrap();
    let meta = d.path().join(META_FILE);
    // A directory where the metadata file should be: a real I/O error, not "missing".
    std::fs::create_dir(&meta).unwrap();
    std::fs::write(meta.join("keep"), "data").unwrap();
    let sessions = d.path().join(SESSIONS_DIR);
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("session_1.json"), "{}").unwrap();

    assert!(ChatSessionManager::new(d.path()).is_err());
    assert!(meta.is_dir());
    assert_eq!(std::fs::read_to_string(meta.join("keep")).unwrap(), "data");
    assert_eq!(
        std::fs::read_to_string(sessions.join("session_1.json")).unwrap(),
        "{}"
    );
}
