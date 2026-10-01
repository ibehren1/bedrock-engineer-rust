//! `window.chatHistory.*` → `chat_history_*` (port of `src/preload/chat-history.ts`, backed by
//! [`history::ChatSessionManager`]).
//!
//! Commands are `async` so the file I/O runs off the main thread.
//!
//! The renderer's `getSession` is synchronous. The startup snapshot carries only the active and
//! recent sessions; for any other id the bridge does a synchronous `XMLHttpRequest` to the
//! [`SESSION_PROTOCOL`] URI scheme served by [`session_protocol`].

use crate::state::HistoryState;
use history::Snapshot;
use serde_json::Value;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, State, UriSchemeContext, Wry};

type History<'a> = State<'a, HistoryState>;

#[tauri::command]
pub async fn chat_history_snapshot(state: History<'_>) -> Result<Snapshot, String> {
    Ok(state.lock()?.snapshot())
}

/// `title` is optional: the bridge passes `Chat ${new Date().toLocaleString()}` so the title uses
/// the renderer's locale as it did under Electron.
#[tauri::command]
pub async fn chat_history_create_session(
    state: History<'_>,
    agent_id: String,
    model_id: String,
    system_prompt: Option<String>,
    title: Option<String>,
) -> Result<String, String> {
    Ok(state.lock()?.create_session(
        &agent_id,
        &model_id,
        system_prompt.as_deref(),
        title.as_deref(),
    ))
}

#[tauri::command]
pub async fn chat_history_add_message(
    state: History<'_>,
    session_id: String,
    message: Value,
) -> Result<(), String> {
    state.lock()?.add_message(&session_id, message);
    Ok(())
}

#[tauri::command]
pub async fn chat_history_get_session(
    state: History<'_>,
    session_id: String,
) -> Result<Option<Value>, String> {
    Ok(state.lock()?.get_session(&session_id))
}

#[tauri::command]
pub async fn chat_history_update_session_title(
    state: History<'_>,
    session_id: String,
    title: String,
) -> Result<(), String> {
    state.lock()?.update_session_title(&session_id, &title);
    Ok(())
}

#[tauri::command]
pub async fn chat_history_delete_session(
    state: History<'_>,
    session_id: String,
) -> Result<(), String> {
    state.lock()?.delete_session(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn chat_history_delete_sessions(
    state: History<'_>,
    session_ids: Vec<String>,
) -> Result<(), String> {
    state.lock()?.delete_sessions(&session_ids);
    Ok(())
}

#[tauri::command]
pub async fn chat_history_delete_all_sessions(state: History<'_>) -> Result<(), String> {
    state.lock()?.delete_all_sessions();
    Ok(())
}

#[tauri::command]
pub async fn chat_history_get_recent_sessions(state: History<'_>) -> Result<Vec<Value>, String> {
    Ok(state.lock()?.get_recent_sessions())
}

#[tauri::command]
pub async fn chat_history_get_all_session_metadata(
    state: History<'_>,
) -> Result<Vec<Value>, String> {
    Ok(state.lock()?.get_all_session_metadata())
}

#[tauri::command]
pub async fn chat_history_set_active_session(
    state: History<'_>,
    session_id: Option<String>,
) -> Result<(), String> {
    state.lock()?.set_active_session(session_id.as_deref());
    Ok(())
}

#[tauri::command]
pub async fn chat_history_get_active_session_id(
    state: History<'_>,
) -> Result<Option<String>, String> {
    Ok(state.lock()?.get_active_session_id())
}

#[tauri::command]
pub async fn chat_history_update_message_content(
    state: History<'_>,
    session_id: String,
    message_index: i64,
    updated_message: Value,
) -> Result<(), String> {
    state
        .lock()?
        .update_message_content(&session_id, message_index, updated_message);
    Ok(())
}

#[tauri::command]
pub async fn chat_history_delete_message(
    state: History<'_>,
    session_id: String,
    message_index: i64,
) -> Result<(), String> {
    state.lock()?.delete_message(&session_id, message_index);
    Ok(())
}

/// URI scheme for synchronous session reads: `chathistory://localhost/<sessionId>` (macOS/Linux)
/// or `http://chathistory.localhost/<sessionId>` (Windows) — what `convertFileSrc(id,
/// 'chathistory')` builds. Responds with the session JSON, or `404` with body `null`.
///
/// Only the app's own pages may read it: the request's `Origin` must be an app origin
/// ([`crate::security::is_app_origin`]), which is echoed back in `Access-Control-Allow-Origin`.
/// Any other (or no) origin gets `403` without CORS headers.
pub const SESSION_PROTOCOL: &str = "chathistory";

pub fn session_protocol(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let dev_url = crate::security::dev_url(ctx.app_handle());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let Some(origin) = allowed_origin(origin, dev_url.as_ref()) else {
        tracing::warn!(
            category = "chat:history",
            origin = origin.unwrap_or("<none>"),
            "Rejected chat history request from a non-app origin"
        );
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(b"null".to_vec())
            .expect("valid response");
    };
    let respond = |status: StatusCode, body: Vec<u8>| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(header::VARY, "Origin")
            .body(body)
            .expect("valid response")
    };
    let Some(session_id) = session_id_from_path(request.uri().path()) else {
        return respond(StatusCode::BAD_REQUEST, b"null".to_vec());
    };
    let state = ctx.app_handle().state::<HistoryState>();
    let session = match state.lock() {
        Ok(manager) => manager.get_session(&session_id),
        Err(e) => {
            tracing::error!(category = "chat:history", error = %e, "session protocol");
            return respond(StatusCode::INTERNAL_SERVER_ERROR, b"null".to_vec());
        }
    };
    match session.and_then(|s| serde_json::to_vec(&s).ok()) {
        Some(body) => respond(StatusCode::OK, body),
        None => respond(StatusCode::NOT_FOUND, b"null".to_vec()),
    }
}

/// The request's `Origin` if it is one of the app's, else `None` (also for a missing header).
fn allowed_origin<'a>(origin: Option<&'a str>, dev_url: Option<&tauri::Url>) -> Option<&'a str> {
    origin.filter(|o| crate::security::is_app_origin(o, dev_url))
}

/// The session id from a request path (`/<percent-encoded id>`). Only plain file-name ids are
/// accepted, so the scheme can't be used to read other files.
fn session_id_from_path(path: &str) -> Option<String> {
    let raw = path.strip_prefix('/').unwrap_or(path);
    let id = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?
        .into_owned();
    let valid = !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    valid.then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_app_origins_may_read_sessions() {
        let dev: tauri::Url = "http://localhost:5173".parse().unwrap();
        for ok in [
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
        ] {
            assert_eq!(allowed_origin(Some(ok), None), Some(ok));
        }
        assert_eq!(
            allowed_origin(Some("http://localhost:5173"), Some(&dev)),
            Some("http://localhost:5173")
        );
        assert_eq!(allowed_origin(Some("http://localhost:5173"), None), None);
        for bad in [
            "null",
            "https://evil.example",
            "htmlpreview://localhost",
            "",
        ] {
            assert_eq!(allowed_origin(Some(bad), Some(&dev)), None, "{bad}");
        }
        assert_eq!(allowed_origin(None, Some(&dev)), None);
    }

    #[test]
    fn session_id_from_protocol_path() {
        assert_eq!(
            session_id_from_path("/session_1790613354839").as_deref(),
            Some("session_1790613354839")
        );
        assert_eq!(
            session_id_from_path("/session%5F1").as_deref(),
            Some("session_1")
        );
        for bad in ["/", "/..%2Fconfig", "/../x", "/a%2Fb", "/.hidden", "/a b"] {
            assert_eq!(session_id_from_path(bad), None, "{bad}");
        }
    }
}
