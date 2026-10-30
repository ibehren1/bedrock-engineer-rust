//! `window.api.usage.*` → `usage_*`: the Accounting page's reads of the usage ledger
//! (`<userData>/usage/usage.db`, see the `usage` crate). Main window only. After each batch of
//! recorded calls the `usage-changed` event (no payload) tells an open report to refresh.
//!
//! Every command is async and runs its query on a blocking thread: a synchronous command runs on
//! the main thread, and a query over a large month (or one waiting for the ledger's lock while
//! the writer inserts) would freeze the window.

use crate::backend::Backend;
use crate::commands::file::file_path_string;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use usage::{
    BreakdownRow, EventRow, GroupBy, SessionCost, SessionWindow, SummaryRow, UsageLedger,
    UsageQuery,
};

/// Tauri event emitted after recorded usage is written.
pub const USAGE_CHANGED_EVENT: &str = "usage-changed";

fn ledger(backend: &State<'_, Backend>) -> Result<Arc<UsageLedger>, String> {
    backend
        .usage
        .clone()
        .ok_or_else(|| "The usage ledger could not be opened; see the log for details.".into())
}

/// Run `f` against the ledger on a blocking thread.
async fn with_ledger<T: Send + 'static>(
    backend: &State<'_, Backend>,
    f: impl FnOnce(&UsageLedger) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let ledger = ledger(backend)?;
    tauri::async_runtime::spawn_blocking(move || f(&ledger))
        .await
        .map_err(|e| e.to_string())?
}

/// Totals for `query`, one row per `groupBy` group.
#[tauri::command]
pub async fn usage_summary(
    backend: State<'_, Backend>,
    query: UsageQuery,
    group_by: GroupBy,
) -> Result<Vec<SummaryRow>, String> {
    with_ledger(&backend, move |l| l.summary(&query, group_by)).await
}

/// Cost per (`by`, `then`) pair, e.g. per day and model for the stacked chart.
#[tauri::command]
pub async fn usage_breakdown(
    backend: State<'_, Backend>,
    query: UsageQuery,
    by: GroupBy,
    then: GroupBy,
) -> Result<Vec<BreakdownRow>, String> {
    with_ledger(&backend, move |l| l.breakdown(&query, by, then)).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageInfo {
    /// Unix ms from which calls are recorded live; earlier rows came from chat history.
    live_since: i64,
    history_imported: bool,
}

#[tauri::command]
pub async fn usage_info(backend: State<'_, Backend>) -> Result<UsageInfo, String> {
    with_ledger(&backend, |l| {
        Ok(UsageInfo {
            live_since: l.live_since()?,
            history_imported: l.history_imported()?,
        })
    })
    .await
}

/// Save every call matching `query` as CSV through a save dialog (Downloads). Resolves to the
/// written path, or `null` when the dialog is canceled.
#[tauri::command]
pub async fn usage_export_csv(
    window: WebviewWindow,
    backend: State<'_, Backend>,
    query: UsageQuery,
    file_name: Option<String>,
) -> Result<Option<String>, String> {
    let csv = with_ledger(&backend, move |l| l.export_csv(&query)).await?;
    let mut dialog = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Export usage")
        .set_file_name(file_name.unwrap_or_else(|| "bedrock-usage.csv".into()))
        .add_filter("CSV", &["csv"]);
    if let Some(downloads) = dirs::download_dir() {
        dialog = dialog.set_directory(downloads);
    }
    let Some(target) = dialog.blocking_save_file().and_then(file_path_string) else {
        return Ok(None);
    };
    std::fs::write(Path::new(&target), csv).map_err(|e| e.to_string())?;
    Ok(Some(target))
}

/// This month's spend against the `usageBudget` setting. Also sends any budget alert that is
/// now due (e.g. right after the budget was lowered).
#[tauri::command]
pub async fn usage_budget_status(
    app: tauri::AppHandle,
    backend: State<'_, Backend>,
) -> Result<crate::budget::BudgetStatus, String> {
    with_ledger(&backend, move |l| crate::budget::check(&app, l)).await
}

/// Cost of each chat or background-agent session (optionally a time window of one), in order.
/// Unlike the other `usage_*` commands, every full-UI window may call this (chat history lists
/// and the task history window show costs).
#[tauri::command]
pub async fn session_usage_costs(
    backend: State<'_, Backend>,
    sessions: Vec<SessionWindow>,
) -> Result<Vec<SessionCost>, String> {
    with_ledger(&backend, move |l| l.session_costs(&sessions)).await
}

/// Months (`YYYY-MM`) with recorded usage, newest first.
#[tauri::command]
pub async fn usage_months(backend: State<'_, Backend>) -> Result<Vec<String>, String> {
    with_ledger(&backend, |l| l.months()).await
}

/// Individual calls matching `query`, newest first (`limit` defaults to 100).
#[tauri::command]
pub async fn usage_events(
    backend: State<'_, Backend>,
    query: UsageQuery,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<EventRow>, String> {
    with_ledger(&backend, move |l| {
        l.events(&query, limit.unwrap_or(100).min(1000), offset.unwrap_or(0))
    })
    .await
}
