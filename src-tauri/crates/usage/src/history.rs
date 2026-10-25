//! One-time import of the token usage chat history already holds (`metadata.converseMetadata`
//! on assistant messages in `<userData>/chat-sessions/*.json`), so months before the ledger
//! existed aren't empty. Only the chat page saved usage, so imported months cover chat only.

use crate::{insert, now_ms, set_meta, Source, UsageLedger, HISTORY_IMPORTED_AT};
use bedrock::usage::{fill_from_converse, UsageContext, UsageEvent, UsageService};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub sessions: usize,
    pub imported: usize,
    /// Files that couldn't be read or parsed.
    pub unreadable: usize,
}

/// The usage events in one session file, each with a key that identifies it across imports.
/// Messages at or after `before` are left out (the ledger recorded those live).
fn session_events(session: &Value, before: i64) -> Vec<(String, UsageEvent)> {
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let session_id = str_of(session, "id").unwrap_or_default();
    let session_model = str_of(session, "modelId").unwrap_or_default();
    let agent_id = str_of(session, "agentId");
    let Some(messages) = session.get("messages").and_then(Value::as_array) else {
        return Vec::new();
    };
    messages
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let metadata = m.get("metadata")?;
            let converse = metadata.get("converseMetadata")?;
            let ts = m.get("timestamp").and_then(Value::as_i64)?;
            if ts >= before {
                return None;
            }
            let model = str_of(metadata, "modelId").unwrap_or_else(|| session_model.clone());
            if model.is_empty() {
                return None;
            }
            let mut event = UsageEvent::new(
                UsageService::Converse,
                &model,
                "",
                UsageContext {
                    feature: "chat".into(),
                    agent_id: agent_id.clone(),
                    session_id: Some(session_id.clone()),
                },
            );
            event.ts_ms = ts;
            if !fill_from_converse(&mut event, converse) {
                return None;
            }
            let id = str_of(m, "id").unwrap_or_else(|| i.to_string());
            Some((format!("chat:{session_id}:{id}"), event))
        })
        .collect()
}

impl UsageLedger {
    /// Import the usage in `sessions_dir`'s chat sessions recorded before live recording began,
    /// then remember that the import ran. Safe to repeat: rows already imported are skipped.
    pub fn import_chat_history(&self, sessions_dir: &Path) -> Result<ImportReport, String> {
        let before = self.live_since()?;
        let mut report = ImportReport::default();
        let entries = match std::fs::read_dir(sessions_dir) {
            Ok(e) => e,
            // No chat history yet.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.with_conn(|c| set_meta(c, HISTORY_IMPORTED_AT, &now_ms().to_string()))?;
                return Ok(report);
            }
            Err(e) => return Err(e.to_string()),
        };
        let mut sessions = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            {
                Some(v) => sessions.push(v),
                None => report.unreadable += 1,
            }
        }
        report.sessions = sessions.len();
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for session in &sessions {
            for (key, event) in session_events(session, before) {
                if insert(&tx, &event, Source::Backfill, Some(&key)).map_err(|e| e.to_string())? {
                    report.imported += 1;
                }
            }
        }
        set_meta(&tx, HISTORY_IMPORTED_AT, &now_ms().to_string()).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        drop(conn);
        if report.imported > 0 {
            crate::notify(&self.on_change);
        }
        Ok(report)
    }
}
