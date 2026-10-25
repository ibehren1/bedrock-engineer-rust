use super::*;
use serde_json::json;

fn write_session(dir: &Path, id: &str, messages: serde_json::Value) {
    let session = json!({
        "id": id,
        "title": "t",
        "createdAt": 0,
        "updatedAt": 0,
        "agentId": "softwareAgent",
        "modelId": SONNET,
        "messages": messages,
    });
    std::fs::write(dir.join(format!("{id}.json")), session.to_string()).unwrap();
}

fn assistant(id: &str, day: &str, input: u64, output: u64) -> serde_json::Value {
    json!({
        "id": id,
        "role": "assistant",
        "content": [{ "text": "hi" }],
        "timestamp": noon(day),
        "metadata": {
            "converseMetadata": {
                "metrics": { "latencyMs": 1 },
                "usage": { "inputTokens": input, "outputTokens": output, "totalTokens": input + output }
            }
        }
    })
}

#[test]
fn imports_chat_usage_once_and_stops_at_live_recording() {
    let (dir, l) = ledger();
    let sessions = dir.path().join("chat-sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    // Pretend live recording began on 2026-10-10.
    l.with_conn(|c| set_meta(c, LIVE_SINCE, &noon("2026-10-10").to_string()))
        .unwrap();
    write_session(
        &sessions,
        "s1",
        json!([
            { "id": "u1", "role": "user", "content": [], "timestamp": noon("2026-09-01") },
            assistant("a1", "2026-09-01", 1000, 0),
            // Recorded live already: must not be imported.
            assistant("a2", "2026-10-11", 5000, 0),
        ]),
    );
    write_session(
        &sessions,
        "s2",
        json!([{
            "id": "a3",
            "role": "assistant",
            "timestamp": noon("2026-10-02"),
            "metadata": { "modelId": "acme.mystery", "converseMetadata": { "usage": { "inputTokens": 1, "outputTokens": 1 } } }
        }]),
    );
    std::fs::write(sessions.join("broken.json"), "").unwrap();

    assert!(!l.history_imported().unwrap());
    let report = l.import_chat_history(&sessions).unwrap();
    assert_eq!(
        report,
        ImportReport {
            sessions: 2,
            imported: 2,
            unreadable: 1
        }
    );
    assert!(l.history_imported().unwrap());

    let rows = l
        .events(
            &UsageQuery {
                from: "2026-01-01".into(),
                to: "2026-12-31".into(),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|r| r.source == "backfill" && r.feature == "chat"));
    let sep = rows.iter().find(|r| r.local_day == "2026-09-01").unwrap();
    assert_eq!(sep.agent_id.as_deref(), Some("softwareAgent"));
    assert_eq!(sep.session_id.as_deref(), Some("s1"));
    assert!((sep.cost_usd.unwrap() - 0.003).abs() < 1e-12);
    assert_eq!(
        rows.iter()
            .find(|r| r.local_day == "2026-10-02")
            .unwrap()
            .model_id,
        "acme.mystery",
        "the message's own model wins over the session's"
    );

    let sept = l.summary(&month("2026-09"), GroupBy::Month).unwrap();
    assert_eq!(sept[0].imported_calls, 1);

    // Running it again adds nothing.
    assert_eq!(l.import_chat_history(&sessions).unwrap().imported, 0);
}

#[test]
fn missing_history_dir_counts_as_imported() {
    let (dir, l) = ledger();
    let report = l.import_chat_history(&dir.path().join("nope")).unwrap();
    assert_eq!(report, ImportReport::default());
    assert!(l.history_imported().unwrap());
}

#[test]
fn live_since_starts_at_the_first_live_row_for_old_ledgers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.db");
    {
        let l = UsageLedger::open(&path).unwrap();
        l.record_now(&event("2026-10-01", SONNET, "chat", 1, 1))
            .unwrap();
        // Simulate a ledger from before `meta` existed.
        l.with_conn(|c| c.execute("DELETE FROM meta", []).map(|_| ()))
            .unwrap();
    }
    let l = UsageLedger::open(&path).unwrap();
    assert_eq!(l.live_since().unwrap(), noon("2026-10-01"));
}

#[test]
fn breakdown_and_csv() {
    let (_d, l) = ledger();
    l.record_now(&event("2026-10-01", SONNET, "chat", 1000, 0))
        .unwrap();
    l.record_now(&event("2026-10-01", "acme.mystery", "chat", 1, 1))
        .unwrap();
    l.record_now(&event("2026-10-02", SONNET, "chat,quoted", 2000, 0))
        .unwrap();
    let rows = l
        .breakdown(&month("2026-10"), GroupBy::Day, GroupBy::Model)
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.key.as_str(), r.sub_key.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("2026-10-01", "anthropic.claude-sonnet-4-6"),
            ("2026-10-01", "acme.mystery"),
            ("2026-10-02", "anthropic.claude-sonnet-4-6"),
        ]
    );

    let csv = l.export_csv(&month("2026-10")).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("time,day,model,"));
    assert!(
        lines[1].contains(",2026-10-01,"),
        "oldest first: {}",
        lines[1]
    );
    assert!(lines[3].contains("\"chat,quoted\""));
    assert!(lines[3].contains(",0.006000,"));
}
