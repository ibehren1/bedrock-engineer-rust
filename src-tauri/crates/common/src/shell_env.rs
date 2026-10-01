//! The user's login-shell `PATH`, for an app started from Finder, the Dock or a desktop launcher.
//!
//! Port of the `fix-path` call at the top of `src/main/index.ts` (`fix-path` -> `shell-path` ->
//! `shell-env`). A GUI launch gets a minimal `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin` on macOS),
//! so tools installed through nvm, volta, asdf, Homebrew or `~/.local/bin` aren't found by child
//! processes such as MCP stdio servers (`npx` is `#!/usr/bin/env node`). Like `shell-env`, this
//! runs `$SHELL` as an interactive login shell, prints the environment between markers, strips
//! ANSI escapes and reads `PATH`. Unlike it, the shell gets a deadline, so a slow or hanging rc
//! file can't hold up startup.

use std::time::Duration;

/// How long the login shell may take before startup goes on without it.
pub const SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// Printed before and after `env`, so rc-file output around it is ignored.
const MARKER: &str = "_BEDROCK_ENGINEER_SHELL_ENV_DELIMITER_";

/// What [`fix_path`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixPathOutcome {
    /// `PATH` was set from the login shell (plus any entries only the old `PATH` had).
    Updated {
        shell: String,
        old: String,
        new: String,
    },
    /// The login shell's `PATH` added nothing to the process `PATH`.
    Unchanged { shell: String },
    /// Not applicable on this platform (Windows GUI apps get the full user `PATH`).
    Skipped,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShellEnvError {
    #[error("failed to run {shell}: {message}")]
    Spawn { shell: String, message: String },
    #[error("{shell} did not finish within {seconds}s")]
    Timeout { shell: String, seconds: u64 },
    #[error("{shell} output had no environment block")]
    MissingMarkers { shell: String },
    #[error("{shell} environment had no PATH")]
    MissingPath { shell: String },
}

/// `fixPath()`: replace the process `PATH` with the login shell's, keeping entries only the
/// current `PATH` has at the end.
///
/// Must run before any other thread starts: it calls [`std::env::set_var`]. The result is
/// returned rather than logged so it can be called before the logger exists.
pub fn fix_path() -> Result<FixPathOutcome, ShellEnvError> {
    if cfg!(windows) {
        return Ok(FixPathOutcome::Skipped);
    }
    let shell = default_shell();
    let shell_path = shell_path(&shell, SHELL_TIMEOUT)?;
    let old = std::env::var("PATH").unwrap_or_default();
    let new = merge_paths(
        [shell_path.as_str(), old.as_str()]
            .iter()
            .flat_map(|p| p.split(':')),
        ":",
        false,
    );
    if new == old {
        return Ok(FixPathOutcome::Unchanged { shell });
    }
    std::env::set_var("PATH", &new);
    Ok(FixPathOutcome::Updated { shell, old, new })
}

/// `$SHELL`, else the platform default (`shell-env`'s `defaultShell`).
pub fn default_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "/bin/zsh".to_string()
            } else {
                "/bin/sh".to_string()
            }
        })
}

/// Join path entries with `sep`, dropping empty entries and repeats (first occurrence wins).
/// `case_insensitive` compares entries the way Windows does.
pub fn merge_paths<'a>(
    entries: impl IntoIterator<Item = &'a str>,
    sep: &str,
    case_insensitive: bool,
) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for entry in entries {
        if entry.is_empty() {
            continue;
        }
        let key = if case_insensitive {
            entry.trim_end_matches(['\\', '/']).to_lowercase()
        } else {
            entry.to_string()
        };
        if seen.insert(key) {
            out.push(entry);
        }
    }
    out.join(sep)
}

/// Remove ANSI escape sequences (CSI, OSC and two-byte escapes) and carriage returns.
pub fn strip_ansi(s: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(concat!(
            r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)", // OSC ... BEL / ST (window titles)
            r"|(?:\x1b\[|\x{9b})[0-?]*[ -/]*[@-~]", // CSI (colors, cursor moves)
            r"|\x1b[=>@-Z\\-_]",                  // other two-byte escapes
            r"|\r",
        ))
        .expect("valid ANSI regex")
    });
    re.replace_all(s, "").into_owned()
}

/// Parse the `PATH` out of the shell's output: the `env` lines between the first two markers.
/// Errors when the markers or a non-empty `PATH` are missing.
pub fn parse_shell_path(output: &str) -> Result<String, ParseError> {
    let mut parts = output.split(MARKER);
    let (Some(_), Some(block), Some(_)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(ParseError::MissingMarkers);
    };
    strip_ansi(block)
        .lines()
        .find_map(|line| line.strip_prefix("PATH=").map(str::trim).map(String::from))
        .filter(|p| !p.is_empty())
        .ok_or(ParseError::MissingPath)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingMarkers,
    MissingPath,
}

/// The shell script run by [`shell_path`]. `printf` rather than `echo -n`, which `sh` may print
/// literally.
fn script() -> String {
    format!("printf '%s' '{MARKER}'; env; printf '%s' '{MARKER}'; exit")
}

/// Run `shell -ilc <script>` and read its `PATH`, killing it after `timeout`.
#[cfg(unix)]
pub fn shell_path(shell: &str, timeout: Duration) -> Result<String, ShellEnvError> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(shell);
    cmd.arg("-ilc")
        .arg(script())
        // oh-my-zsh: don't stop to ask about updates.
        .env("DISABLE_AUTO_UPDATE", "true")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // A new session without a controlling terminal, so an interactive shell started from a
    // terminal (`npm run dev`) can't take it over, and the whole group can be killed on timeout.
    // SAFETY: setsid is async-signal-safe and touches no Rust state.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|e| ShellEnvError::Spawn {
        shell: shell.to_string(),
        message: e.to_string(),
    })?;

    // Read on a helper thread so a full pipe can't stall the shell; this thread only waits.
    // The reader never touches the environment, so `set_var` afterwards stays sound.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    let output = match rx.recv_timeout(timeout) {
        Ok(buf) => {
            let _ = child.wait();
            buf
        }
        Err(_) => {
            // SAFETY: plain kill(2) of the process group the child leads.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(ShellEnvError::Timeout {
                shell: shell.to_string(),
                seconds: timeout.as_secs(),
            });
        }
    };
    parse_shell_path(&String::from_utf8_lossy(&output)).map_err(|e| match e {
        ParseError::MissingMarkers => ShellEnvError::MissingMarkers {
            shell: shell.to_string(),
        },
        ParseError::MissingPath => ShellEnvError::MissingPath {
            shell: shell.to_string(),
        },
    })
}

#[cfg(not(unix))]
pub fn shell_path(shell: &str, _timeout: Duration) -> Result<String, ShellEnvError> {
    Err(ShellEnvError::Spawn {
        shell: shell.to_string(),
        message: "login shells are only read on Unix".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(env: &str) -> String {
        format!("{MARKER}{env}{MARKER}")
    }

    #[test]
    fn reads_path_between_markers() {
        let out = wrap("HOME=/Users/me\nPATH=/a/bin:/usr/bin\nSHELL=/bin/zsh\n");
        assert_eq!(parse_shell_path(&out).unwrap(), "/a/bin:/usr/bin");
    }

    #[test]
    fn ignores_rc_file_noise_around_the_block() {
        let out = format!(
            "Welcome!\nPATH=/wrong\n{}goodbye PATH=/also-wrong\n",
            wrap("\nFOO=bar\nPATH=/right:/usr/bin\n")
        );
        assert_eq!(parse_shell_path(&out).unwrap(), "/right:/usr/bin");
    }

    #[test]
    fn strips_ansi_escapes() {
        let block = "\x1b]0;user@host: ~\x07\x1b[1;32mPATH=/x/bin:/usr/bin\x1b[0m\r\nA=b\n";
        assert_eq!(parse_shell_path(&wrap(block)).unwrap(), "/x/bin:/usr/bin");
        assert_eq!(strip_ansi("\x1b[?2004hok\x1b[?2004l\x1b="), "ok");
    }

    #[test]
    fn values_containing_equals_and_similar_keys() {
        let out = wrap("MYPATH=/nope\nINFOPATH=/nope\nPATH=/a=b:/c\n");
        assert_eq!(parse_shell_path(&out).unwrap(), "/a=b:/c");
    }

    #[test]
    fn missing_markers_or_path() {
        assert_eq!(
            parse_shell_path("PATH=/usr/bin\n"),
            Err(ParseError::MissingMarkers)
        );
        assert_eq!(
            parse_shell_path(&format!("{MARKER}PATH=/usr/bin\n")),
            Err(ParseError::MissingMarkers)
        );
        assert_eq!(
            parse_shell_path(&wrap("HOME=/h\n")),
            Err(ParseError::MissingPath)
        );
        assert_eq!(
            parse_shell_path(&wrap("PATH=\n")),
            Err(ParseError::MissingPath)
        );
    }

    #[test]
    fn merge_paths_dedupes_and_drops_empties() {
        assert_eq!(
            merge_paths("/a::/b:/a:/c:/b".split(':'), ":", false),
            "/a:/b:/c"
        );
        assert_eq!(
            merge_paths(["C:\\Git\\cmd", "c:\\git\\CMD\\", "D:\\x"], ";", true),
            "C:\\Git\\cmd;D:\\x"
        );
        assert_eq!(merge_paths(["/A", "/a"], ":", false), "/A:/a");
    }

    #[cfg(unix)]
    #[test]
    fn runs_a_real_shell() {
        let path = shell_path("/bin/sh", Duration::from_secs(10)).unwrap();
        assert!(!path.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn hanging_shell_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("slow-shell");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30 &\nsleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let started = std::time::Instant::now();
        let err = shell_path(fake.to_str().unwrap(), Duration::from_millis(300)).unwrap_err();
        assert!(matches!(err, ShellEnvError::Timeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn missing_shell_is_a_spawn_error() {
        let err = shell_path("/nonexistent/shell", Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, ShellEnvError::Spawn { .. }));
    }
}
