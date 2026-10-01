//! Logging — port of `src/common/logger/**` onto `tracing`.
//!
//! Mirrors the winston setup: files at `<userData>/logs/bedrock-engineer-YYYY-MM-DD.log` (local
//! date, rotated daily, 10 MB size cap, 5 files kept), plus console output, one line format:
//!
//! ```text
//! 2026-09-30T12:34:56.789Z [info] [main:agents:ipc] Imported agent file
//! {
//!   "file_path": "/tmp/agent.yaml"
//! }
//! ```
//!
//! Category loggers become a `category` field: `tracing::info!(category = "agents:ipc", ...)`.
//! Events without one are logged under their target's category `general`. Winston's `verbose`
//! level sits between `info` and `debug`; tracing has no such level, so `verbose` maps to `debug`.

use chrono::{Local, SecondsFormat, Utc};
use serde_json::{Map, Value};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{reload, Layer, Registry};

/// `LOG_LEVELS`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
}

impl LogLevel {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "error" => Self::Error,
            "warn" => Self::Warn,
            "info" => Self::Info,
            "debug" => Self::Debug,
            "verbose" => Self::Verbose,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Verbose => "verbose",
        }
    }

    pub fn filter(self) -> LevelFilter {
        match self {
            Self::Error => LevelFilter::ERROR,
            Self::Warn => LevelFilter::WARN,
            Self::Info => LevelFilter::INFO,
            Self::Debug | Self::Verbose => LevelFilter::DEBUG,
        }
    }
}

/// `LoggerConfig`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggerConfig {
    pub level: LogLevel,
    pub file_log_enabled: bool,
    pub console_log_enabled: bool,
    /// Size cap per file in bytes (`maxSize: '10m'`).
    pub max_size: u64,
    pub max_files: usize,
    pub log_dir: PathBuf,
    pub log_file_prefix: String,
}

impl LoggerConfig {
    /// `defaultLoggerConfig` (debug in development builds, info otherwise) with
    /// `initLoggerConfig(userDataPath)` applied: `logDir = <userData>/logs`.
    pub fn for_user_data(user_data_path: &Path, development: bool) -> Self {
        Self {
            level: if development {
                LogLevel::Debug
            } else {
                LogLevel::Info
            },
            file_log_enabled: true,
            console_log_enabled: true,
            max_size: 10 * 1024 * 1024,
            max_files: 5,
            log_dir: user_data_path.join("logs"),
            log_file_prefix: "bedrock-engineer".into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// line format
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct FieldCollector {
    message: String,
    category: Option<String>,
    process: Option<String>,
    extra: Map<String, Value>,
}

impl Visit for FieldCollector {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "category" => self.category = Some(value.to_string()),
            "process" => self.process = Some(value.to_string()),
            name => {
                self.extra.insert(name.into(), Value::String(value.into()));
            }
        }
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.extra.insert(field.name().into(), Value::Bool(value));
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.extra.insert(field.name().into(), value.into());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.extra.insert(field.name().into(), value.into());
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.extra.insert(field.name().into(), value.into());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let rendered = format!("{value:?}");
        match field.name() {
            "message" => self.message = rendered,
            "category" => self.category = Some(rendered),
            "process" => self.process = Some(rendered),
            name => {
                self.extra.insert(name.into(), Value::String(rendered));
            }
        }
    }
}

fn level_name(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "verbose",
    }
}

/// Render one log line the way winston's `customFormat` does.
pub fn format_line(
    timestamp: &str,
    level: &str,
    process: Option<&str>,
    category: Option<&str>,
    message: &str,
    extra: &Map<String, Value>,
) -> String {
    let mut line = format!(
        "{timestamp} [{level}] [{}:{}] {message}",
        process.unwrap_or("main"),
        category.unwrap_or("general")
    );
    if !extra.is_empty() {
        match serde_json::to_string_pretty(extra) {
            Ok(json) => {
                let _ = write!(line, "\n{json}");
            }
            Err(_) => line.push_str("\n[Metadata serialization error]"),
        }
    }
    line
}

/// `tracing_subscriber` event formatter producing [`format_line`] output.
#[derive(Debug, Clone, Copy, Default)]
pub struct BedrockEngineerFormat {
    pub ansi: bool,
}

impl<S, N> FormatEvent<S, N> for BedrockEngineerFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut fields = FieldCollector::default();
        event.record(&mut fields);
        let level = level_name(event.metadata().level());
        let level = if self.ansi {
            let color = match *event.metadata().level() {
                Level::ERROR => "31",
                Level::WARN => "33",
                Level::INFO => "32",
                Level::DEBUG => "34",
                Level::TRACE => "36",
            };
            format!("\x1b[{color}m{level}\x1b[39m")
        } else {
            level.to_string()
        };
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        writeln!(
            writer,
            "{}",
            format_line(
                &timestamp,
                &level,
                fields.process.as_deref(),
                fields.category.as_deref(),
                &fields.message,
                &fields.extra,
            )
        )
    }
}

// ---------------------------------------------------------------------------------------------
// daily rotating file
// ---------------------------------------------------------------------------------------------

/// Appends to `<dir>/<prefix>-YYYY-MM-DD.log` (local date), moving on to
/// `<prefix>-YYYY-MM-DD.N.log` past `max_size`, and keeping at most `max_files` files.
pub struct DailyRotatingFile {
    dir: PathBuf,
    prefix: String,
    max_size: u64,
    max_files: usize,
    current: Option<(String, usize, File, u64)>,
}

impl DailyRotatingFile {
    pub fn new(config: &LoggerConfig) -> io::Result<Self> {
        fs::create_dir_all(&config.log_dir)?;
        Ok(Self {
            dir: config.log_dir.clone(),
            prefix: config.log_file_prefix.clone(),
            max_size: config.max_size,
            max_files: config.max_files,
            current: None,
        })
    }

    fn file_name(&self, date: &str, index: usize) -> String {
        if index == 0 {
            format!("{}-{date}.log", self.prefix)
        } else {
            format!("{}-{date}.{index}.log", self.prefix)
        }
    }

    fn open(&mut self, date: String, mut index: usize) -> io::Result<()> {
        loop {
            let path = self.dir.join(self.file_name(&date, index));
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if self.max_size > 0 && size >= self.max_size {
                index += 1;
                continue;
            }
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            self.current = Some((date, index, file, size));
            self.prune();
            return Ok(());
        }
    }

    fn prune(&self) {
        if self.max_files == 0 {
            return;
        }
        let mut files = log_files_in(&self.dir, &self.prefix);
        files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
        while files.len() > self.max_files {
            let _ = fs::remove_file(files.remove(0));
        }
    }
}

impl Write for DailyRotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let reopen = match &self.current {
            None => Some((today.clone(), 0)),
            Some((date, _, _, _)) if *date != today => Some((today.clone(), 0)),
            Some((date, index, _, size)) if self.max_size > 0 && *size >= self.max_size => {
                Some((date.clone(), index + 1))
            }
            _ => None,
        };
        if let Some((date, index)) = reopen {
            self.open(date, index)?;
        }
        let (_, _, file, size) = self.current.as_mut().expect("log file open");
        let n = file.write(buf)?;
        *size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.current {
            Some((_, _, file, _)) => file.flush(),
            None => Ok(()),
        }
    }
}

fn log_files_in(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.starts_with(prefix) && name.ends_with(".log")
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

/// `getLogFiles()` / `getLogFilePaths(config)`: matching files, most recent (by name) first.
pub fn get_log_files(config: &LoggerConfig) -> Vec<PathBuf> {
    let mut files = log_files_in(&config.log_dir, &config.log_file_prefix);
    files.sort();
    files.reverse();
    files
}

// ---------------------------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------------------------

/// Keeps the non-blocking file writer flushing; hold it for the life of the app. Also allows
/// changing the level at runtime (`setLogLevel`).
pub struct LoggerGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    level: reload::Handle<LevelFilter, Registry>,
    config: Mutex<LoggerConfig>,
}

impl LoggerGuard {
    /// `setLogLevel(level)`
    pub fn set_log_level(&self, level: LogLevel) {
        let _ = self.level.modify(|f| *f = level.filter());
        if let Ok(mut c) = self.config.lock() {
            c.level = level;
        }
    }

    /// `getLoggerConfig()`
    pub fn config(&self) -> LoggerConfig {
        self.config
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    /// `getLogFiles()`
    pub fn log_files(&self) -> Vec<PathBuf> {
        get_log_files(&self.config())
    }
}

/// `initLogger()`: install the global subscriber (file + console per `config`).
///
/// Fails if a global subscriber is already set.
pub fn init_logger(config: LoggerConfig) -> Result<LoggerGuard, crate::Error> {
    let (level_layer, level_handle) = reload::Layer::new(config.level.filter());

    let (file_layer, file_guard) = if config.file_log_enabled {
        let file = DailyRotatingFile::new(&config)?;
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .event_format(BedrockEngineerFormat { ansi: false });
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    let console_layer = config.console_log_enabled.then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(io::stdout)
            .event_format(BedrockEngineerFormat { ansi: true })
            .boxed()
    });

    let subscriber = Registry::default()
        .with(level_layer)
        .with(file_layer)
        .with(console_layer);
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| crate::Error::Logger(e.to_string()))?;

    Ok(LoggerGuard {
        _file_guard: file_guard,
        level: level_handle,
        config: Mutex::new(config),
    })
}

/// The previous panic hook, as returned by [`std::panic::take_hook`].
pub type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static>;

/// At most this many lines of a panic's backtrace go to the log (two lines per frame: the
/// symbol and its `at file:line`), so a deep stack can't flood the log file.
pub const PANIC_BACKTRACE_MAX_LINES: usize = 80;

/// How long a panic hook waits after logging so the non-blocking file writer can write the
/// entry before a panic that aborts the process (one unwinding out of the event loop) kills it.
const PANIC_FLUSH_GRACE: std::time::Duration = std::time::Duration::from_millis(100);

/// `registerGlobalErrorHandlers()` (Electron's `uncaughtException` / `unhandledRejection`
/// logging): a panic hook that logs the panic — message, location, thread name and a backtrace
/// (truncated to [`PANIC_BACKTRACE_MAX_LINES`]) — at error level through `tracing`, then calls
/// `previous` so the usual stderr report is still printed.
///
/// The backtrace is always captured (not only with `RUST_BACKTRACE` set), since released apps
/// are never started with it and a panic without a stack is rarely actionable.
pub fn panic_hook(previous: PanicHook) -> PanicHook {
    Box::new(move |info| {
        let message = panic_message(info.payload());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let current = std::thread::current();
        let thread = current.name().unwrap_or("<unnamed>");
        let backtrace = truncate_lines(
            &std::backtrace::Backtrace::force_capture().to_string(),
            PANIC_BACKTRACE_MAX_LINES,
        );
        tracing::error!(
            error = %message,
            location = %location,
            thread = %thread,
            backtrace = %backtrace,
            "Panic"
        );
        std::thread::sleep(PANIC_FLUSH_GRACE);
        previous(info);
    })
}

/// Install [`panic_hook`] around the current hook. Call right after [`init_logger`].
pub fn install_panic_hook() {
    std::panic::set_hook(panic_hook(std::panic::take_hook()));
}

/// The `panic!` message: a `&str` or `String` payload, else a placeholder.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".into()
    }
}

/// The first `max` lines of `s`, with a note of how many were dropped.
fn truncate_lines(s: &str, max: usize) -> String {
    let total = s.lines().count();
    if total <= max {
        return s.trim_end().to_string();
    }
    let mut out = s.lines().take(max).collect::<Vec<_>>().join("\n");
    let _ = write!(out, "\n... ({} more lines)", total - max);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// A `MakeWriter` collecting everything into a shared buffer.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
        type Writer = Captured;
        fn make_writer(&'a self) -> Captured {
            self.clone()
        }
    }

    #[test]
    fn panic_hook_logs_then_calls_previous_hook() {
        let captured = Captured::default();
        let subscriber = Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(captured.clone())
                .event_format(BedrockEngineerFormat { ansi: false }),
        );
        let previous_called = Arc::new(AtomicBool::new(false));
        let flag = previous_called.clone();
        let hook = panic_hook(Box::new(move |_| flag.store(true, Ordering::SeqCst)));

        // The hook is process-wide; install it only for this panic, on a named thread whose
        // default subscriber is the capturing one.
        let original = std::panic::take_hook();
        std::panic::set_hook(hook);
        let result = std::thread::Builder::new()
            .name("panic-test".into())
            .spawn(move || {
                tracing::subscriber::with_default(subscriber, || {
                    std::panic::catch_unwind(|| panic!("boom {}", 42)).is_err()
                })
            })
            .unwrap()
            .join();
        std::panic::set_hook(original);

        assert!(result.unwrap(), "the closure panicked");
        assert!(previous_called.load(Ordering::SeqCst));
        let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(log.contains("[error] [main:general] Panic"), "{log}");
        assert!(log.contains("\"error\": \"boom 42\""), "{log}");
        assert!(log.contains("\"thread\": \"panic-test\""), "{log}");
        assert!(log.contains("logger.rs:"), "{log}");
        assert!(log.contains("\"backtrace\""), "{log}");
    }

    #[test]
    fn panic_message_handles_str_string_and_other_payloads() {
        assert_eq!(panic_message(&"static"), "static");
        assert_eq!(panic_message(&String::from("owned")), "owned");
        assert_eq!(panic_message(&7_u8), "<non-string panic payload>");
    }

    #[test]
    fn truncate_lines_caps_long_text() {
        assert_eq!(truncate_lines("a\nb\n", 5), "a\nb");
        assert_eq!(truncate_lines("a\nb\nc\nd", 2), "a\nb\n... (2 more lines)");
    }

    #[test]
    fn line_format_matches_winston_custom_format() {
        let mut extra = Map::new();
        assert_eq!(
            format_line("T", "info", None, None, "hello", &extra),
            "T [info] [main:general] hello"
        );
        extra.insert("file".into(), json!("a.yaml"));
        assert_eq!(
            format_line("T", "warn", Some("main"), Some("agents:ipc"), "m", &extra),
            "T [warn] [main:agents:ipc] m\n{\n  \"file\": \"a.yaml\"\n}"
        );
    }

    #[test]
    fn writes_daily_files_under_logs_and_lists_them() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = LoggerConfig::for_user_data(dir.path(), false);
        assert_eq!(config.log_dir, dir.path().join("logs"));
        let mut file = DailyRotatingFile::new(&config).unwrap();
        file.write_all(b"line\n").unwrap();
        file.flush().unwrap();
        let files = get_log_files(&config);
        assert_eq!(files.len(), 1);
        let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("bedrock-engineer-{today}.log"));
    }

    #[test]
    fn rolls_over_past_max_size_and_prunes() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut config = LoggerConfig::for_user_data(dir.path(), false);
        config.max_size = 4;
        config.max_files = 2;
        let mut file = DailyRotatingFile::new(&config).unwrap();
        for _ in 0..4 {
            file.write_all(b"12345").unwrap();
        }
        file.flush().unwrap();
        assert_eq!(get_log_files(&config).len(), 2);
    }

    #[test]
    fn verbose_maps_to_debug() {
        assert_eq!(
            LogLevel::parse("verbose").unwrap().filter(),
            LevelFilter::DEBUG
        );
        assert_eq!(LogLevel::parse("nope"), None);
    }
}
