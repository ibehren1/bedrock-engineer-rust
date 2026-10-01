//! Installs the global subscriber, so it lives in its own test binary.

use common::logger::{init_logger, LogLevel, LoggerConfig};

#[test]
fn writes_formatted_lines_to_the_daily_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut config = LoggerConfig::for_user_data(dir.path(), false);
    config.console_log_enabled = false;
    let guard = init_logger(config).unwrap();

    tracing::info!(
        category = "agents:ipc",
        file = "a.yaml",
        "Imported agent file"
    );
    tracing::debug!("hidden at info level");
    guard.set_log_level(LogLevel::Debug);
    tracing::debug!("shown after set_log_level");

    let config = guard.config();
    assert_eq!(config.level, LogLevel::Debug);
    drop(guard); // flush the non-blocking writer
    let files = common::logger::get_log_files(&config);

    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    let first = text.lines().next().unwrap();
    assert!(
        first.ends_with(" [info] [main:agents:ipc] Imported agent file"),
        "{first}"
    );
    assert!(text.contains("\"file\": \"a.yaml\""));
    assert!(!text.contains("hidden at info level"));
    assert!(text.contains("[debug] [main:general] shown after set_log_level"));
}
