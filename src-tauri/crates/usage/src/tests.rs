use super::*;
use bedrock::usage::{UsageContext, UsageService};
use chrono::NaiveDate;
use std::time::Duration;

mod history_tests;

/// Unix ms for noon local time on `day`, so the local day is unambiguous in any time zone.
fn noon(day: &str) -> i64 {
    let d = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
    Local
        .from_local_datetime(&d.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp_millis()
}

fn event(day: &str, model: &str, feature: &str, input: u64, output: u64) -> UsageEvent {
    let mut e = UsageEvent::new(
        UsageService::Converse,
        model,
        "us-east-1",
        UsageContext::feature(feature),
    );
    e.ts_ms = noon(day);
    e.input_tokens = input;
    e.output_tokens = output;
    e
}

fn ledger() -> (tempfile::TempDir, Arc<UsageLedger>) {
    let dir = tempfile::tempdir().unwrap();
    let ledger = UsageLedger::open(&dir.path().join("usage").join("usage.db")).unwrap();
    (dir, ledger)
}

const SONNET: &str = "us.anthropic.claude-sonnet-4-6";

fn month(m: &str) -> UsageQuery {
    UsageQuery {
        from: format!("{m}-01"),
        to: format!("{m}-31"),
        ..Default::default()
    }
}

#[test]
fn local_day_formats_in_local_time() {
    assert_eq!(local_day(noon("2026-03-09")), "2026-03-09");
}

#[test]
fn groups_by_day_month_model_and_feature() {
    let (_d, l) = ledger();
    l.record_now(&event("2026-09-30", SONNET, "chat", 1000, 0))
        .unwrap();
    l.record_now(&event("2026-10-01", SONNET, "chat", 1000, 1000))
        .unwrap();
    l.record_now(&event("2026-10-01", SONNET, "background", 2000, 0))
        .unwrap();
    l.record_now(&event("2026-10-02", "acme.mystery", "", 10, 10))
        .unwrap();

    let days = l.summary(&month("2026-10"), GroupBy::Day).unwrap();
    assert_eq!(
        days.iter()
            .map(|r| (r.key.as_str(), r.calls))
            .collect::<Vec<_>>(),
        vec![("2026-10-01", 2), ("2026-10-02", 1)]
    );
    // 3000 input at $0.003/1k + 1000 output at $0.015/1k
    assert!((days[0].cost_usd - (0.009 + 0.015)).abs() < 1e-12);
    assert_eq!(days[1].cost_usd, 0.0);
    assert_eq!(days[1].unpriced_calls, 1);

    let all = UsageQuery {
        from: "0000-00-00".into(),
        to: "9999-99-99".into(),
        ..Default::default()
    };
    let months = l.summary(&all, GroupBy::Month).unwrap();
    assert_eq!(
        months.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
        vec!["2026-09", "2026-10"]
    );
    assert_eq!(l.months().unwrap(), vec!["2026-10", "2026-09"]);

    let models = l.summary(&month("2026-10"), GroupBy::Model).unwrap();
    let sonnet = models
        .iter()
        .find(|r| r.key == "anthropic.claude-sonnet-4-6")
        .unwrap();
    assert_eq!(sonnet.calls, 2);
    assert_ne!(sonnet.label, sonnet.key, "label is the catalog name");

    let features = l.summary(&month("2026-10"), GroupBy::Feature).unwrap();
    assert_eq!(
        features.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
        vec!["background", "chat", UNKNOWN_FEATURE]
    );
}

#[test]
fn filters_and_lists_events_newest_first() {
    let (_d, l) = ledger();
    l.record_now(&event("2026-10-01", SONNET, "chat", 1, 1))
        .unwrap();
    l.record_now(&event("2026-10-03", SONNET, "title", 2, 2))
        .unwrap();
    l.record_now(&event("2026-10-02", "acme.mystery", "chat", 3, 3))
        .unwrap();

    let q = UsageQuery {
        feature: Some("chat".into()),
        ..month("2026-10")
    };
    let rows = l.events(&q, 10, 0).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.local_day.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-10-02", "2026-10-01"]
    );
    assert_eq!(rows[0].cost_usd, None);
    assert_eq!(rows[0].source, "live");
    assert_eq!(l.events(&q, 1, 1).unwrap()[0].local_day, "2026-10-01");

    let q = UsageQuery {
        model_id: Some("anthropic.claude-sonnet-4-6".into()),
        ..month("2026-10")
    };
    assert_eq!(l.summary(&q, GroupBy::Model).unwrap()[0].calls, 2);
}

#[test]
fn price_is_kept_with_the_row() {
    let (_d, l) = ledger();
    l.record_now(&event("2026-10-01", SONNET, "chat", 1000, 0))
        .unwrap();
    let snapshot: String = l
        .with_conn(|c| c.query_row("SELECT price_snapshot FROM usage_events", [], |r| r.get(0)))
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(v["input"], 0.003);
}

#[test]
fn sink_writes_in_background_and_notifies() {
    let (_d, l) = ledger();
    let (tx, rx) = mpsc::channel();
    l.set_on_change(move || {
        let _ = tx.send(());
    });
    let sink = l.sink();
    for _ in 0..5 {
        sink.record(event("2026-10-05", SONNET, "chat", 1, 1));
    }
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // Later batches may still be in flight; wait until all five are in.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let calls = l
            .summary(&month("2026-10"), GroupBy::Month)
            .unwrap()
            .first()
            .map_or(0, |r| r.calls);
        if calls == 5 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "only {calls} recorded"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reopening_prices_rows_that_had_no_price() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.db");
    {
        let l = UsageLedger::open(&path).unwrap();
        l.record_now(&event("2026-10-01", SONNET, "chat", 1000, 0))
            .unwrap();
        // As if recorded before the catalog knew the price.
        l.with_conn(|c| {
            c.execute(
                "UPDATE usage_events SET cost_usd = NULL, price_snapshot = NULL, model_name = 'x'",
                [],
            )
            .map(|_| ())
        })
        .unwrap();
        l.record_now(&event("2026-10-01", "acme.mystery", "chat", 1, 1))
            .unwrap();
    }
    let l = UsageLedger::open(&path).unwrap();
    let rows = l.summary(&month("2026-10"), GroupBy::Model).unwrap();
    let sonnet = rows
        .iter()
        .find(|r| r.key == "anthropic.claude-sonnet-4-6")
        .unwrap();
    assert!((sonnet.cost_usd - 0.003).abs() < 1e-12);
    assert_ne!(sonnet.label, "x");
    assert_eq!(
        rows.iter()
            .find(|r| r.key == "acme.mystery")
            .unwrap()
            .unpriced_calls,
        1
    );
}

#[test]
fn month_cost_and_budget_alerts() {
    let (_d, l) = ledger();
    l.record_now(&event("2026-10-01", SONNET, "chat", 1000, 0))
        .unwrap();
    l.record_now(&event("2026-11-01", SONNET, "chat", 2000, 0))
        .unwrap();
    assert!((l.month_cost("2026-10").unwrap() - 0.003).abs() < 1e-12);
    assert_eq!(l.month_cost("2026-12").unwrap(), 0.0);

    assert!(l.budget_alerts_sent("2026-10").unwrap().is_empty());
    l.mark_budget_alert("2026-10", 0.8).unwrap();
    l.mark_budget_alert("2026-10", 0.8).unwrap();
    l.mark_budget_alert("2026-10", 1.0).unwrap();
    assert_eq!(l.budget_alerts_sent("2026-10").unwrap(), vec![0.8, 1.0]);
    assert!(l.budget_alerts_sent("2026-11").unwrap().is_empty());
}

#[test]
fn unit_events_keep_their_units() {
    let (_d, l) = ledger();
    let mut e = UsageEvent::units(
        UsageService::Image,
        "stability.sd3-5-large-v1:0",
        "us-west-2",
        UsageContext::feature("tool:generateImage"),
        bedrock::usage::UnitKind::Images,
        3.0,
    );
    e.ts_ms = noon("2026-10-04");
    l.record_now(&e).unwrap();
    let row = &l.summary(&month("2026-10"), GroupBy::Service).unwrap()[0];
    assert_eq!(row.key, "image");
    assert_eq!(row.images, 3.0);
    assert!((row.cost_usd - 0.24).abs() < 1e-12);
    let ev = &l.events(&month("2026-10"), 10, 0).unwrap()[0];
    assert_eq!(ev.units, Some(3.0));
    assert_eq!(ev.unit_kind.as_deref(), Some("images"));
}

#[test]
fn agents_sessions_and_cache_savings() {
    let (_d, l) = ledger();
    let mut a = event("2026-10-01", SONNET, "chat", 1000, 0);
    a.context.agent_id = Some("dev".into());
    a.context.session_id = Some("s1".into());
    // 10k cache reads save (0.003 - 0.0003) * 10 = $0.027; 1k writes cost (0.00375 - 0.003) more.
    a.cache_read_tokens = 10_000;
    a.cache_write_tokens = 1_000;
    l.record_now(&a).unwrap();
    let mut b = event("2026-10-02", SONNET, "background", 2000, 0);
    b.context.session_id = Some("s1".into());
    l.record_now(&b).unwrap();
    l.record_now(&event("2026-10-03", "acme.mystery", "chat", 1, 1))
        .unwrap();

    let agents = l.summary(&month("2026-10"), GroupBy::Agent).unwrap();
    assert_eq!(
        agents
            .iter()
            .map(|r| (r.key.as_str(), r.calls))
            .collect::<Vec<_>>(),
        vec![("", 2), ("dev", 1)]
    );
    let q = UsageQuery {
        agent_id: Some("dev".into()),
        ..month("2026-10")
    };
    let dev = &l.summary(&q, GroupBy::Month).unwrap()[0];
    assert!((dev.cache_savings_usd - (0.027 - 0.00075)).abs() < 1e-12);

    let costs = l
        .session_costs(&[
            SessionWindow {
                session_id: "s1".into(),
                ..Default::default()
            },
            SessionWindow {
                session_id: "s1".into(),
                from_ts: Some(noon("2026-10-01")),
                to_ts: None,
            },
            SessionWindow {
                session_id: "nope".into(),
                ..Default::default()
            },
        ])
        .unwrap();
    assert_eq!(costs[0].calls, 2);
    assert_eq!(costs[1].calls, 1);
    assert!((costs[1].cost_usd - 0.006).abs() < 1e-12);
    assert_eq!(costs[2].calls, 0);
}

#[test]
fn reopening_keeps_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.db");
    {
        let l = UsageLedger::open(&path).unwrap();
        l.record_now(&event("2026-10-01", SONNET, "chat", 1, 1))
            .unwrap();
    }
    let l = UsageLedger::open(&path).unwrap();
    assert_eq!(l.months().unwrap(), vec!["2026-10"]);
}
