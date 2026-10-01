//! Cron parsing / matching / heartbeat tests. The TS app had no tests for this (it relied on
//! node-cron); the expectations below follow node-cron 4.6.0's `validate` and
//! `TimeMatcher.getNextMatch`.

use super::*;
use chrono::{FixedOffset, Utc};

fn utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn next(expr: &str, base: &str) -> String {
    CronExpression::parse(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e}"))
        .next_after(&utc(base))
        .unwrap()
        .to_rfc3339()
}

#[test]
fn validate_matches_node_cron() {
    let valid = [
        "0 9 * * 1-5",
        "* * * * * *",
        "*/15 * * * *",
        "@daily",
        " @HOURLY ",
        "0  9 * * *",
        "0 0 ? * ?",
        "0 0 L * *",
        "0 0 L-3 * *",
        "0 0 15W * *",
        "0 0 1W,15 * *",
        "0 0 LW * *",
        "0 0 * * 5L",
        "0 0 * * 1#2",
        "0 0 * * 7",
        "0 0 * * MON-FRI",
        "0 0 * * sunday,Sat",
        "0 0 1 JAN,jul *",
        "0 22-2 * * *",
        "0 0-30/10 * * * *",
        "0 0 29 2 *",
        "30 */2 * * * *",
    ];
    for e in valid {
        assert!(validate(e), "expected valid: {e:?}");
    }
    let invalid = [
        "",
        "* * * *",
        "* * * * * * *",
        "60 * * * *",
        "* 24 * * *",
        "* * 0 * *",
        "* * 32 * *",
        "* * * 13 *",
        "* * * * 8",
        "0 0 30 2 *",
        "0 0 31 4,6 *",
        "0 0 L-31 * *",
        "0 0 32W * *",
        "0 0 xW * *",
        "0 0 * * 1#6",
        "0 0 * * 5L,x",
        "5/10 * * * *",
        "0 */0 * * *",
        "0\t9 * * *",
        "a b c d e",
        "0 9 * * 1-5!",
        "@every",
        "0 0 1-40 * *",
        "0 L * * *",
    ];
    for e in invalid {
        assert!(!validate(e), "expected invalid: {e:?}");
    }
}

#[test]
fn parse_reports_node_cron_messages() {
    assert_eq!(
        CronExpression::parse("61 * * * *").unwrap_err(),
        "61 is a invalid expression for minute"
    );
    assert_eq!(
        CronExpression::parse("* * *").unwrap_err(),
        "expected 5 or 6 fields but got 3"
    );
    assert_eq!(
        CronExpression::parse("0 0 30 2 *").unwrap_err(),
        "30 2 is an impossible day of month for the given month"
    );
}

#[test]
fn next_run_basic_fields() {
    let base = "2026-09-30T10:15:30Z"; // Wednesday
    assert_eq!(next("0 9 * * 1-5", base), "2026-10-01T09:00:00+00:00");
    assert_eq!(next("*/15 * * * *", base), "2026-09-30T10:30:00+00:00");
    assert_eq!(next("* * * * * *", base), "2026-09-30T10:15:31+00:00");
    assert_eq!(
        next("* * * * * *", "2026-09-30T10:15:30.900Z"),
        "2026-09-30T10:15:31+00:00"
    );
    assert_eq!(
        next("15 10 * * *", "2026-09-30T10:15:00Z"),
        "2026-10-01T10:15:00+00:00"
    );
    assert_eq!(next("@yearly", base), "2027-01-01T00:00:00+00:00");
    assert_eq!(next("0 0 29 2 *", base), "2028-02-29T00:00:00+00:00");
    assert_eq!(next("0 0 * * 7", base), "2026-10-04T00:00:00+00:00");
    assert_eq!(next("0 0 * * SAT", base), "2026-10-03T00:00:00+00:00");
    assert_eq!(next("0 0 1 JUN *", base), "2027-06-01T00:00:00+00:00");
}

#[test]
fn next_run_wrapping_range_and_steps() {
    let base = "2026-09-30T10:15:30Z";
    assert_eq!(next("0 22-2 * * *", base), "2026-09-30T22:00:00+00:00");
    assert_eq!(
        next("0 22-2 * * *", "2026-09-30T23:30:00Z"),
        "2026-10-01T00:00:00+00:00"
    );
    assert_eq!(
        next("0 22-2 * * *", "2026-10-01T02:00:00Z"),
        "2026-10-01T22:00:00+00:00"
    );
    assert_eq!(next("10-40/15 * * * *", base), "2026-09-30T10:25:00+00:00");
}

#[test]
fn day_of_month_and_week_are_both_required() {
    // Friday the 13th, not "the 13th or any Friday".
    assert_eq!(
        next("0 0 13 * 5", "2026-09-30T10:15:30Z"),
        "2026-11-13T00:00:00+00:00"
    );
}

#[test]
fn special_day_tokens() {
    let base = "2026-09-30T10:15:30Z";
    assert_eq!(next("0 0 L * *", base), "2026-10-31T00:00:00+00:00");
    assert_eq!(next("0 0 L-2 * *", base), "2026-10-29T00:00:00+00:00");
    // 2026-10-31 is a Saturday: the nearest weekday is Friday the 30th.
    assert_eq!(next("0 0 LW * *", base), "2026-10-30T00:00:00+00:00");
    // 2026-11-15 is a Sunday → Monday the 16th.
    assert_eq!(
        next("0 0 15W * *", "2026-10-16T00:00:00Z"),
        "2026-11-16T00:00:00+00:00"
    );
    // 2026-08-01 is a Saturday and the 1st → Monday the 3rd (never back into July).
    assert_eq!(
        next("0 0 1W * *", "2026-07-15T00:00:00Z"),
        "2026-08-03T00:00:00+00:00"
    );
    // Last Friday of October 2026.
    assert_eq!(next("0 0 * * 5L", base), "2026-10-30T00:00:00+00:00");
    // Second Monday of October 2026.
    assert_eq!(next("0 0 * * 1#2", base), "2026-10-12T00:00:00+00:00");
    // 7#1 is the first Sunday.
    assert_eq!(next("0 0 * * 7#1", base), "2026-10-04T00:00:00+00:00");
}

#[test]
fn evaluates_in_the_given_timezone() {
    let jst = FixedOffset::east_opt(9 * 3600).unwrap();
    let base = utc("2026-09-30T10:15:30Z").with_timezone(&jst); // 19:15:30 JST
    let expr = CronExpression::parse("0 9 * * *").unwrap();
    let n = expr.next_after(&base).unwrap();
    assert_eq!(n.to_rfc3339(), "2026-10-01T09:00:00+09:00");
    assert_eq!(
        expr.next_after_ms(&jst, base.timestamp_millis()),
        Some(utc("2026-10-01T00:00:00Z").timestamp_millis())
    );
    assert!(expr.matches(&n));
    assert!(!expr.matches(&n.with_timezone(&Utc)));
}

#[test]
fn plan_beat_runs_on_time_and_skips_missed_slots() {
    let minute = 60_000;
    let every_minute = |t: i64| Some((t / minute + 1) * minute);
    let t0 = 1_000 * minute;

    // Woke up early (24 h cap / timer drift): keep waiting for the same slot.
    assert_eq!(
        plan_beat(t0, t0 - 5, 1000, every_minute),
        BeatPlan {
            missed: vec![],
            run: None,
            next: Some(t0)
        }
    );
    // On time and slightly late.
    assert_eq!(
        plan_beat(t0, t0, 1000, every_minute),
        BeatPlan {
            missed: vec![],
            run: Some(t0),
            next: Some(t0 + minute)
        }
    );
    assert_eq!(plan_beat(t0, t0 + 1000, 1000, every_minute).run, Some(t0));
    // More than a second late: missed, not run.
    assert_eq!(
        plan_beat(t0, t0 + 1001, 1000, every_minute),
        BeatPlan {
            missed: vec![t0],
            run: None,
            next: Some(t0 + minute)
        }
    );
    // Slept through three slots and woke just after the fourth: only the fourth runs.
    assert_eq!(
        plan_beat(t0, t0 + 3 * minute + 500, 1000, every_minute),
        BeatPlan {
            missed: vec![t0, t0 + minute, t0 + 2 * minute],
            run: Some(t0 + 3 * minute),
            next: Some(t0 + 4 * minute)
        }
    );
    // No further match.
    assert_eq!(plan_beat(t0, t0, 1000, |_| None).next, None);
}

#[test]
fn next_after_ms_matches_node_semantics_for_scheduler() {
    let expr = CronExpression::parse("0 0 * * *").unwrap();
    let base = utc("2026-09-30T00:00:00Z").timestamp_millis();
    // Strictly after the base second.
    assert_eq!(
        expr.next_after_ms(&Utc, base),
        Some(utc("2026-10-01T00:00:00Z").timestamp_millis())
    );
}
