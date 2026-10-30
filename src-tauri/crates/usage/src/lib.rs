//! The usage ledger behind the Accounting page: one SQLite row per billable Bedrock call,
//! priced when it is recorded and kept forever, so the report can group any period by day,
//! month, model or feature.
//!
//! Calls reach the ledger through [`bedrock::usage`]: the app installs [`UsageLedger::sink`] as
//! the process-wide sink. The sink only queues the event; a writer thread inserts queued events
//! in one transaction per wake-up and calls the change hook at most once per
//! [`NOTIFY_INTERVAL`], so a stream of calls (an ingest embeds passage by passage, a long agent
//! run) produces about one `usage-changed` event a second rather than one per call.
//!
//! Chat history from before the ledger existed can be imported once
//! ([`UsageLedger::import_chat_history`]); those rows have `source = 'backfill'`.

mod csv;
mod history;
mod pricing;

pub use history::ImportReport;

pub use pricing::{price, Priced};

use bedrock::usage::{UnitKind, UsageEvent, UsageService, UsageSink};
use chrono::{Local, TimeZone};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Least time between two calls of the change hook; writes in between are reported together.
pub const NOTIFY_INTERVAL: Duration = Duration::from_secs(1);

/// 1: the ledger. 2: adds `usage_events.calls` (rows that stand for a batch of calls).
const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS usage_events (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL,
    local_day TEXT NOT NULL,
    model_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    raw_model_id TEXT NOT NULL,
    region TEXT NOT NULL,
    service TEXT NOT NULL,
    feature TEXT NOT NULL,
    agent_id TEXT,
    session_id TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    units REAL,
    unit_kind TEXT,
    cost_usd REAL,
    price_snapshot TEXT,
    estimated INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'live',
    dedupe_key TEXT UNIQUE,
    calls INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS usage_events_day ON usage_events (local_day);
-- Covers the month total behind the budget badge, so it never reads the table itself.
CREATE INDEX IF NOT EXISTS usage_events_day_cost ON usage_events (local_day, cost_usd);
CREATE INDEX IF NOT EXISTS usage_events_model_day ON usage_events (model_id, local_day);
CREATE INDEX IF NOT EXISTS usage_events_feature_day ON usage_events (feature, local_day);
";

/// Label for events whose caller didn't say what they were for.
pub const UNKNOWN_FEATURE: &str = "other";

/// `meta` key: Unix ms from which calls are recorded live (the ledger's first open). Imported
/// history stops here so nothing is counted twice.
const LIVE_SINCE: &str = "live_since";
/// `meta` key: Unix ms of the finished chat-history import.
const HISTORY_IMPORTED_AT: &str = "history_imported_at";

/// Where a row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Live,
    Backfill,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Live => "live",
            Source::Backfill => "backfill",
        }
    }
}

type ChangeHook = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

pub struct UsageLedger {
    /// For reads and synchronous writes; the writer thread has its own connection.
    conn: Mutex<Connection>,
    queue: mpsc::Sender<UsageEvent>,
    on_change: ChangeHook,
}

/// `YYYY-MM-DD` in the machine's time zone.
pub fn local_day(ts_ms: i64) -> String {
    Local
        .timestamp_millis_opt(ts_ms)
        .single()
        .map(|t| t.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn open_connection(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// Insert one row; a `dedupe_key` that is already present is skipped (returns `false`).
/// Bring a ledger from an older version up to [`SCHEMA_VERSION`]. Idempotent.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let has_calls = conn
        .prepare("SELECT 1 FROM pragma_table_info('usage_events') WHERE name = 'calls'")?
        .exists([])?;
    if !has_calls {
        conn.execute_batch("ALTER TABLE usage_events ADD COLUMN calls INTEGER NOT NULL DEFAULT 1")?;
    }
    conn.execute("UPDATE schema_version SET version = ?1", [SCHEMA_VERSION])?;
    Ok(())
}

fn insert(
    conn: &Connection,
    event: &UsageEvent,
    source: Source,
    dedupe_key: Option<&str>,
) -> rusqlite::Result<bool> {
    let priced = price(event);
    let feature = if event.context.feature.is_empty() {
        UNKNOWN_FEATURE
    } else {
        event.context.feature.as_str()
    };
    let n = conn.execute(
        "INSERT OR IGNORE INTO usage_events (ts, local_day, model_id, model_name, raw_model_id,
            region, service, feature, agent_id, session_id, input_tokens, output_tokens,
            cache_read_tokens, cache_write_tokens, cost_usd, price_snapshot, estimated, source,
            dedupe_key, units, unit_kind, calls)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22)",
        params![
            event.ts_ms,
            local_day(event.ts_ms),
            priced.model_id,
            priced.model_name,
            event.raw_model_id,
            event.region,
            event.service.as_str(),
            feature,
            event.context.agent_id,
            event.context.session_id,
            event.input_tokens as i64,
            event.output_tokens as i64,
            event.cache_read_tokens as i64,
            event.cache_write_tokens as i64,
            priced.cost_usd,
            priced.snapshot.map(|v| v.to_string()),
            event.estimated,
            source.as_str(),
            dedupe_key,
            event.unit_kind.map(|_| event.units),
            event.unit_kind.map(UnitKind::as_str),
            event.calls.max(1),
        ],
    )?;
    Ok(n > 0)
}

fn meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Price rows recorded without one whose model the catalog now knows (a model added after the
/// call, or history imported before its retired price was listed). Rows with a price are never
/// touched, so past months don't move when prices change. Returns the number re-priced.
fn reprice_unpriced(conn: &Connection) -> rusqlite::Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, service, model_id, input_tokens, output_tokens, cache_read_tokens,
            cache_write_tokens, units, unit_kind FROM usage_events WHERE cost_usd IS NULL",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                [
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                ],
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut repriced = 0;
    for (id, service, model_id, [input, output, cache_read, cache_write], units, unit_kind) in rows
    {
        let Some(service) = UsageService::parse(&service) else {
            continue;
        };
        let mut event = UsageEvent::new(service, &model_id, "", Default::default());
        event.units = units.unwrap_or(0.0);
        event.unit_kind = unit_kind.as_deref().and_then(UnitKind::parse);
        event.input_tokens = input as u64;
        event.output_tokens = output as u64;
        event.cache_read_tokens = cache_read as u64;
        event.cache_write_tokens = cache_write as u64;
        let priced = price(&event);
        let Some(cost) = priced.cost_usd else {
            continue;
        };
        repriced += conn.execute(
            "UPDATE usage_events SET cost_usd = ?1, price_snapshot = ?2, model_name = ?3
             WHERE id = ?4",
            params![
                cost,
                priced.snapshot.map(|v| v.to_string()),
                priced.model_name,
                id
            ],
        )?;
    }
    Ok(repriced)
}

fn notify(hook: &ChangeHook) {
    if let Some(f) = hook.lock().ok().as_deref().and_then(Option::as_ref) {
        f();
    }
}

/// Inserts queued events until every sender is gone, a batch per wake-up. The change hook runs
/// at most once per [`NOTIFY_INTERVAL`]; a write inside the interval is reported when it ends.
fn writer(conn: Connection, queue: mpsc::Receiver<UsageEvent>, on_change: ChangeHook) {
    let mut conn = conn;
    let mut last_notify: Option<Instant> = None;
    let mut pending = false;
    loop {
        let first = if pending {
            let wait = last_notify.map_or(Duration::ZERO, |t| {
                NOTIFY_INTERVAL.saturating_sub(t.elapsed())
            });
            match queue.recv_timeout(wait) {
                Ok(e) => Some(e),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    notify(&on_change);
                    return;
                }
            }
        } else {
            match queue.recv() {
                Ok(e) => Some(e),
                Err(_) => return,
            }
        };
        if let Some(first) = first {
            let mut batch = vec![first];
            batch.extend(queue.try_iter());
            let result = conn.transaction().and_then(|tx| {
                for event in &batch {
                    insert(&tx, event, Source::Live, None)?;
                }
                tx.commit()
            });
            match result {
                Ok(()) => pending = true,
                Err(e) => {
                    tracing::error!(error = %e, dropped = batch.len(), "Failed to record usage")
                }
            }
        }
        if pending && last_notify.is_none_or(|t| t.elapsed() >= NOTIFY_INTERVAL) {
            notify(&on_change);
            last_notify = Some(Instant::now());
            pending = false;
        }
    }
}

/// What to group a summary by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    Day,
    Month,
    Model,
    Feature,
    Service,
    Agent,
}

impl GroupBy {
    fn key(self) -> &'static str {
        match self {
            GroupBy::Day => "local_day",
            GroupBy::Month => "substr(local_day, 1, 7)",
            GroupBy::Model => "model_id",
            GroupBy::Feature => "feature",
            GroupBy::Service => "service",
            GroupBy::Agent => "COALESCE(agent_id, '')",
        }
    }

    fn label(self) -> &'static str {
        match self {
            GroupBy::Model => "MAX(model_name)",
            _ => self.key(),
        }
    }
}

/// An inclusive range of local days (`YYYY-MM-DD`) plus optional filters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// `""` matches calls with no agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl UsageQuery {
    fn where_clause(&self) -> (String, Vec<String>) {
        let mut sql = "local_day >= ?1 AND local_day <= ?2".to_string();
        let mut args = vec![self.from.clone(), self.to.clone()];
        if let Some(m) = &self.model_id {
            args.push(m.clone());
            sql.push_str(&format!(" AND model_id = ?{}", args.len()));
        }
        if let Some(f) = &self.feature {
            args.push(f.clone());
            sql.push_str(&format!(" AND feature = ?{}", args.len()));
        }
        if let Some(a) = &self.agent_id {
            args.push(a.clone());
            sql.push_str(&format!(" AND COALESCE(agent_id, '') = ?{}", args.len()));
        }
        (sql, args)
    }
}

/// Totals for one group.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRow {
    pub key: String,
    pub label: String,
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Sum over priced calls only.
    pub cost_usd: f64,
    /// Calls with no known price (not in `cost_usd`).
    pub unpriced_calls: u64,
    /// Calls whose token counts were estimated.
    pub estimated_calls: u64,
    /// Calls imported from chat history (recorded before the ledger existed; chat only).
    pub imported_calls: u64,
    /// Units of the non-token services.
    pub images: f64,
    pub video_seconds: f64,
    pub characters: f64,
    pub guardrail_text_units: f64,
    /// What prompt caching saved: cache reads at the input rate minus what they cost, less the
    /// cache-write premium over the input rate. Negative when writes outweighed reads.
    pub cache_savings_usd: f64,
}

/// A session (chat or background-agent run), optionally limited to a time window (Unix ms,
/// `from` exclusive, `to` inclusive).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWindow {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_ts: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_ts: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCost {
    pub session_id: String,
    pub calls: u64,
    pub cost_usd: f64,
}

/// Cost of one (group, sub-group) pair, e.g. one model on one day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownRow {
    pub key: String,
    pub sub_key: String,
    pub sub_label: String,
    pub calls: u64,
    pub cost_usd: f64,
}

/// One recorded call, or batch of calls (see `calls`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub id: i64,
    pub ts: i64,
    pub local_day: String,
    pub model_id: String,
    pub model_name: String,
    pub raw_model_id: String,
    pub region: String,
    pub service: String,
    pub feature: String,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: Option<f64>,
    pub estimated: bool,
    pub source: String,
    pub units: Option<f64>,
    pub unit_kind: Option<String>,
    /// Bedrock calls the row stands for (more than 1 for a batch of Titan embeddings).
    pub calls: u64,
}

struct QueueSink(mpsc::Sender<UsageEvent>);

impl UsageSink for QueueSink {
    fn record(&self, event: UsageEvent) {
        let _ = self.0.send(event);
    }
}

impl UsageLedger {
    /// Open (creating if needed) the ledger at `path` and start its writer thread.
    pub fn open(path: &Path) -> Result<Arc<Self>, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = open_connection(path).map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        let version: Option<i64> = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        if version.is_none() {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )
            .map_err(|e| e.to_string())?;
        }
        migrate(&conn).map_err(|e| e.to_string())?;
        // Ledgers created before `meta` existed already hold live rows; start from the first.
        if meta(&conn, LIVE_SINCE)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            let first: Option<i64> = conn
                .query_row(
                    "SELECT MIN(ts) FROM usage_events WHERE source = 'live'",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            set_meta(&conn, LIVE_SINCE, &first.unwrap_or_else(now_ms).to_string())
                .map_err(|e| e.to_string())?;
        }
        match reprice_unpriced(&conn) {
            Ok(0) => {}
            Ok(n) => tracing::info!(rows = n, "Priced usage recorded without a price"),
            Err(error) => tracing::warn!(%error, "Failed to re-price usage"),
        }
        let writer_conn = open_connection(path).map_err(|e| e.to_string())?;
        let (queue, rx) = mpsc::channel();
        let on_change: ChangeHook = Arc::default();
        let hook = on_change.clone();
        std::thread::Builder::new()
            .name("usage-ledger".into())
            .spawn(move || writer(writer_conn, rx, hook))
            .map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
            queue,
            on_change,
        }))
    }

    /// The sink to install with [`bedrock::usage::set_sink`]; events are written in the
    /// background.
    pub fn sink(&self) -> Arc<dyn UsageSink> {
        Arc::new(QueueSink(self.queue.clone()))
    }

    /// Called after each batch of queued events is written.
    pub fn set_on_change(&self, f: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut hook) = self.on_change.lock() {
            *hook = Some(Box::new(f));
        }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&conn).map_err(|e| e.to_string())
    }

    /// Write an event now (tests).
    pub fn record_now(&self, event: &UsageEvent) -> Result<(), String> {
        self.with_conn(|c| insert(c, event, Source::Live, None).map(|_| ()))
    }

    /// When live recording began (Unix ms).
    pub fn live_since(&self) -> Result<i64, String> {
        self.with_conn(|c| meta(c, LIVE_SINCE))?
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| "The usage ledger has no start time".to_string())
    }

    /// Estimated cost of `month` (`YYYY-MM`), priced calls only.
    pub fn month_cost(&self, month: &str) -> Result<f64, String> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(SUM(cost_usd), 0) FROM usage_events
                 WHERE local_day >= ?1 AND local_day <= ?2",
                [format!("{month}-01"), format!("{month}-31")],
                |r| r.get(0),
            )
        })
    }

    /// Budget alert thresholds (fractions of the budget) already sent for `month`.
    pub fn budget_alerts_sent(&self, month: &str) -> Result<Vec<f64>, String> {
        let key = format!("budget_alerts:{month}");
        Ok(self
            .with_conn(|c| meta(c, &key))?
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default())
    }

    /// Remember that the `threshold` alert went out for `month`.
    pub fn mark_budget_alert(&self, month: &str, threshold: f64) -> Result<(), String> {
        let mut sent = self.budget_alerts_sent(month)?;
        if !sent.contains(&threshold) {
            sent.push(threshold);
        }
        let value = serde_json::to_string(&sent).map_err(|e| e.to_string())?;
        self.with_conn(|c| set_meta(c, &format!("budget_alerts:{month}"), &value))
    }

    /// Whether the chat-history import has run.
    pub fn history_imported(&self) -> Result<bool, String> {
        Ok(self.with_conn(|c| meta(c, HISTORY_IMPORTED_AT))?.is_some())
    }

    /// Totals for `query`, one row per group, ordered by key (days and months chronologically).
    pub fn summary(
        &self,
        query: &UsageQuery,
        group_by: GroupBy,
    ) -> Result<Vec<SummaryRow>, String> {
        let (filter, args) = query.where_clause();
        let sql = format!(
            "SELECT {key}, {label}, SUM(calls), SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read_tokens), SUM(cache_write_tokens), COALESCE(SUM(cost_usd), 0),
                SUM(CASE WHEN cost_usd IS NULL THEN calls ELSE 0 END),
                SUM(CASE WHEN estimated THEN calls ELSE 0 END),
                SUM(CASE WHEN source = 'backfill' THEN calls ELSE 0 END),
                COALESCE(SUM(CASE unit_kind WHEN 'images' THEN units END), 0),
                COALESCE(SUM(CASE unit_kind WHEN 'video_seconds' THEN units END), 0),
                COALESCE(SUM(CASE unit_kind WHEN 'characters' THEN units END), 0),
                COALESCE(SUM(CASE unit_kind WHEN 'guardrail_text_units' THEN units END), 0),
                COALESCE(SUM(CASE WHEN json_extract(price_snapshot, '$.cacheRead') IS NOT NULL THEN
                    (cache_read_tokens * (json_extract(price_snapshot, '$.input')
                        - json_extract(price_snapshot, '$.cacheRead'))
                     - cache_write_tokens * (json_extract(price_snapshot, '$.cacheWrite')
                        - json_extract(price_snapshot, '$.input'))) / 1000.0
                END), 0)
             FROM usage_events WHERE {filter} GROUP BY {key} ORDER BY {key}",
            key = group_by.key(),
            label = group_by.label(),
        );
        self.with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
                Ok(SummaryRow {
                    key: r.get(0)?,
                    label: r.get(1)?,
                    calls: r.get::<_, i64>(2)? as u64,
                    input_tokens: r.get::<_, i64>(3)? as u64,
                    output_tokens: r.get::<_, i64>(4)? as u64,
                    cache_read_tokens: r.get::<_, i64>(5)? as u64,
                    cache_write_tokens: r.get::<_, i64>(6)? as u64,
                    cost_usd: r.get(7)?,
                    unpriced_calls: r.get::<_, i64>(8)? as u64,
                    estimated_calls: r.get::<_, i64>(9)? as u64,
                    imported_calls: r.get::<_, i64>(10)? as u64,
                    images: r.get(11)?,
                    video_seconds: r.get(12)?,
                    characters: r.get(13)?,
                    guardrail_text_units: r.get(14)?,
                    cache_savings_usd: r.get(15)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Cost per (`by`, `then`) pair, e.g. per day and model for a stacked chart. Ordered by
    /// `by`, then by cost (highest first).
    pub fn breakdown(
        &self,
        query: &UsageQuery,
        by: GroupBy,
        then: GroupBy,
    ) -> Result<Vec<BreakdownRow>, String> {
        let (filter, args) = query.where_clause();
        let sql = format!(
            "SELECT {key}, {sub}, {sub_label}, SUM(calls), COALESCE(SUM(cost_usd), 0) AS cost
             FROM usage_events WHERE {filter} GROUP BY {key}, {sub} ORDER BY {key}, cost DESC",
            key = by.key(),
            sub = then.key(),
            sub_label = then.label(),
        );
        self.with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
                Ok(BreakdownRow {
                    key: r.get(0)?,
                    sub_key: r.get(1)?,
                    sub_label: r.get(2)?,
                    calls: r.get::<_, i64>(3)? as u64,
                    cost_usd: r.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Every call matching `query` as CSV (header row first, oldest first).
    pub fn export_csv(&self, query: &UsageQuery) -> Result<String, String> {
        let rows = self.events(query, u32::MAX, 0)?;
        Ok(csv::events_csv(rows.iter().rev()))
    }

    /// Cost of each session (or session window), in the order given.
    pub fn session_costs(&self, sessions: &[SessionWindow]) -> Result<Vec<SessionCost>, String> {
        self.with_conn(|c| {
            let mut stmt = c.prepare_cached(
                "SELECT COALESCE(SUM(calls), 0), COALESCE(SUM(cost_usd), 0) FROM usage_events
                 WHERE session_id = ?1 AND ts > COALESCE(?2, -1) AND ts <= COALESCE(?3, 9223372036854775807)",
            )?;
            sessions
                .iter()
                .map(|s| {
                    stmt.query_row(params![s.session_id, s.from_ts, s.to_ts], |r| {
                        Ok(SessionCost {
                            session_id: s.session_id.clone(),
                            calls: r.get::<_, i64>(0)? as u64,
                            cost_usd: r.get(1)?,
                        })
                    })
                })
                .collect()
        })
    }

    /// Every month (`YYYY-MM`) with recorded usage, newest first.
    pub fn months(&self) -> Result<Vec<String>, String> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT DISTINCT substr(local_day, 1, 7) AS m FROM usage_events ORDER BY m DESC",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }

    /// Individual calls matching `query`, newest first.
    pub fn events(
        &self,
        query: &UsageQuery,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<EventRow>, String> {
        let (filter, mut args) = query.where_clause();
        args.push(limit.to_string());
        args.push(offset.to_string());
        let sql = format!(
            "SELECT id, ts, local_day, model_id, model_name, raw_model_id, region, service, feature,
                agent_id, session_id, input_tokens, output_tokens, cache_read_tokens,
                cache_write_tokens, cost_usd, estimated, source, units, unit_kind, calls
             FROM usage_events WHERE {filter} ORDER BY ts DESC, id DESC
             LIMIT CAST(?{l} AS INTEGER) OFFSET CAST(?{o} AS INTEGER)",
            l = args.len() - 1,
            o = args.len(),
        );
        self.with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
                Ok(EventRow {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    local_day: r.get(2)?,
                    model_id: r.get(3)?,
                    model_name: r.get(4)?,
                    raw_model_id: r.get(5)?,
                    region: r.get(6)?,
                    service: r.get(7)?,
                    feature: r.get(8)?,
                    agent_id: r.get(9)?,
                    session_id: r.get(10)?,
                    input_tokens: r.get::<_, i64>(11)? as u64,
                    output_tokens: r.get::<_, i64>(12)? as u64,
                    cache_read_tokens: r.get::<_, i64>(13)? as u64,
                    cache_write_tokens: r.get::<_, i64>(14)? as u64,
                    cost_usd: r.get(15)?,
                    estimated: r.get(16)?,
                    source: r.get(17)?,
                    units: r.get(18)?,
                    unit_kind: r.get(19)?,
                    calls: r.get::<_, i64>(20)? as u64,
                })
            })?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests;
