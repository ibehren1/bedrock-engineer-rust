//! The Accounting page's monthly budget (store key `usageBudget`): status for the page and the
//! sidebar badge, and an OS notification the first time each alert threshold is crossed in a
//! month (checked after every batch of recorded usage and whenever the status is read).

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use usage::UsageLedger;

/// `usageBudget`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BudgetSettings {
    /// Dollars per month; `None` (or 0) means no budget.
    pub monthly_usd: Option<f64>,
    /// Alert thresholds as fractions of the budget.
    pub alert_at: Vec<f64>,
}

impl Default for BudgetSettings {
    fn default() -> Self {
        Self {
            monthly_usd: None,
            alert_at: vec![0.8, 1.0],
        }
    }
}

impl BudgetSettings {
    pub fn from_store(snapshot: &Value) -> Self {
        snapshot
            .get("usageBudget")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    fn budget(&self) -> Option<f64> {
        self.monthly_usd.filter(|b| *b > 0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    /// `YYYY-MM`, local time.
    pub month: String,
    pub spent_usd: f64,
    pub budget_usd: Option<f64>,
    /// `spent / budget` (`None` without a budget).
    pub fraction: Option<f64>,
    /// Spend so far extended linearly to the end of the month.
    pub projected_usd: f64,
    pub over_budget: bool,
}

/// The local month of `now` and how much of it has passed (0..=1, at least one day's worth so
/// an early-month projection isn't wild).
fn month_progress(now: chrono::DateTime<Local>) -> (String, f64) {
    let month = format!("{:04}-{:02}", now.year(), now.month());
    let first = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).expect("valid date");
    let next = if now.month() == 12 {
        NaiveDate::from_ymd_opt(now.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
    }
    .expect("valid date");
    let start = Local
        .from_local_datetime(&first.and_hms_opt(0, 0, 0).expect("midnight"))
        .earliest()
        .unwrap_or(now);
    let end = Local
        .from_local_datetime(&next.and_hms_opt(0, 0, 0).expect("midnight"))
        .earliest()
        .unwrap_or(now);
    let total = (end - start).num_seconds().max(1) as f64;
    let days = (next - first).num_days().max(1) as f64;
    let elapsed = (now - start).num_seconds() as f64 / total;
    (month, elapsed.clamp(1.0 / days, 1.0))
}

pub fn status_at(
    spent: f64,
    settings: &BudgetSettings,
    now: chrono::DateTime<Local>,
) -> BudgetStatus {
    let (month, progress) = month_progress(now);
    let budget = settings.budget();
    let fraction = budget.map(|b| spent / b);
    BudgetStatus {
        month,
        spent_usd: spent,
        budget_usd: budget,
        fraction,
        projected_usd: spent / progress,
        over_budget: fraction.is_some_and(|f| f >= 1.0),
    }
}

/// Thresholds reached by `fraction` that haven't been alerted yet, lowest first.
pub fn new_alerts(fraction: f64, alert_at: &[f64], sent: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = alert_at
        .iter()
        .copied()
        .filter(|t| *t > 0.0 && fraction >= *t && !sent.contains(t))
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup();
    out
}

/// This month's status, alerting for any newly crossed threshold.
pub fn check(app: &AppHandle, ledger: &UsageLedger) -> Result<BudgetStatus, String> {
    let settings = BudgetSettings::from_store(&crate::backend::store_snapshot(app));
    let now = Local::now();
    let (month, _) = month_progress(now);
    let status = status_at(ledger.month_cost(&month)?, &settings, now);
    if let (Some(fraction), Some(budget)) = (status.fraction, status.budget_usd) {
        let sent = ledger.budget_alerts_sent(&month)?;
        // Only the highest new threshold is shown; the lower ones are marked sent with it.
        let fresh = new_alerts(fraction, &settings.alert_at, &sent);
        if let Some(top) = fresh.last() {
            notify(app, status.spent_usd, budget, *top);
            for t in &fresh {
                ledger.mark_budget_alert(&month, *t)?;
            }
        }
    }
    Ok(status)
}

fn notify(app: &AppHandle, spent: f64, budget: f64, threshold: f64) {
    let title = if threshold >= 1.0 {
        "Bedrock budget reached"
    } else {
        "Bedrock budget alert"
    };
    let body = format!(
        "Estimated Bedrock cost this month is ${spent:.2}, {:.0}% of your ${budget:.2} budget.",
        spent / budget * 100.0
    );
    let handle = app.clone();
    let on_click = move || crate::store_sync::focus_main_window(&handle);
    if let Err(error) = crate::notify::show_with_click(app, title, &body, on_click) {
        tracing::warn!(%error, "Failed to show the budget notification");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(y: i32, m: u32, d: u32, h: u32) -> chrono::DateTime<Local> {
        Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(y, m, d)
                    .unwrap()
                    .and_hms_opt(h, 0, 0)
                    .unwrap(),
            )
            .earliest()
            .unwrap()
    }

    #[test]
    fn settings_default_and_parse() {
        assert_eq!(
            BudgetSettings::from_store(&json!({})),
            BudgetSettings::default()
        );
        let s = BudgetSettings::from_store(
            &json!({ "usageBudget": { "monthlyUsd": 50, "alertAt": [0.5] } }),
        );
        assert_eq!(s.monthly_usd, Some(50.0));
        assert_eq!(s.alert_at, vec![0.5]);
        // No budget: 0 counts as none.
        let s = BudgetSettings::from_store(&json!({ "usageBudget": { "monthlyUsd": 0 } }));
        assert_eq!(s.budget(), None);
    }

    #[test]
    fn status_projects_and_flags_over_budget() {
        let settings = BudgetSettings {
            monthly_usd: Some(100.0),
            ..Default::default()
        };
        // Halfway through a 30-day month.
        let s = status_at(40.0, &settings, at(2026, 11, 16, 0));
        assert_eq!(s.month, "2026-11");
        assert!((s.projected_usd - 80.0).abs() < 1.0, "{}", s.projected_usd);
        assert_eq!(s.fraction, Some(0.4));
        assert!(!s.over_budget);
        assert!(status_at(100.0, &settings, at(2026, 11, 16, 0)).over_budget);
        // The first hours project from at least one day.
        let early = status_at(1.0, &settings, at(2026, 11, 1, 1));
        assert!(early.projected_usd <= 30.0 + 1e-9);
        let none = status_at(5.0, &BudgetSettings::default(), at(2026, 11, 16, 0));
        assert_eq!(none.fraction, None);
        assert!(!none.over_budget);
    }

    #[test]
    fn alerts_fire_once_per_threshold() {
        assert_eq!(new_alerts(0.5, &[0.8, 1.0], &[]), Vec::<f64>::new());
        assert_eq!(new_alerts(0.85, &[0.8, 1.0], &[]), vec![0.8]);
        assert_eq!(new_alerts(1.2, &[1.0, 0.8], &[]), vec![0.8, 1.0]);
        assert_eq!(new_alerts(1.2, &[0.8, 1.0], &[0.8]), vec![1.0]);
        assert_eq!(new_alerts(1.2, &[0.8, 1.0], &[0.8, 1.0]), Vec::<f64>::new());
    }
}
