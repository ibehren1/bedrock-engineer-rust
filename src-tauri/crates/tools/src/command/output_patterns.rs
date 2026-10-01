//! Port of `src/main/api/command/outputPatterns.ts` (and its test).
//!
//! Shared by the host shell path ([`super::CommandService`]) and, later, the Docker
//! sandbox exec path.

use regex::Regex;
use std::sync::LazyLock;

/// `{ isWaiting, prompt }` from `detectWaitingForInput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingForInput {
    pub is_waiting: bool,
    pub prompt: Option<String>,
}

struct InputDetectionPattern {
    pattern: Regex,
    prompt_extractor: fn(&str) -> String,
}

static INQUIRER_PROMPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)\? (.+\?.*$)").expect("regex"));

fn last_line(s: &str) -> String {
    s.split('\n').next_back().unwrap_or_default().to_string()
}

/// JS `trimEnd` (ECMAScript whitespace + line terminators).
fn trim_end_js(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

static INPUT_DETECTION_PATTERNS: LazyLock<Vec<InputDetectionPattern>> = LazyLock::new(|| {
    vec![
        // inquirer-style question
        InputDetectionPattern {
            pattern: Regex::new(r"(?m)\? .+\?.*$").expect("regex"),
            prompt_extractor: |output| {
                INQUIRER_PROMPT
                    .captures(output)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| output.to_string())
            },
        },
        // basic prompt, e.g. "Enter name: "
        InputDetectionPattern {
            pattern: Regex::new(r"(?m)[^:]+: $").expect("regex"),
            prompt_extractor: last_line,
        },
        // apt/dpkg style confirmations ending in "[Y/n]" / "[y/N]"
        InputDetectionPattern {
            pattern: Regex::new(r"(?m)\[[Yy]/[Nn]\]\??\s*$").expect("regex"),
            prompt_extractor: |output| last_line(trim_end_js(output)),
        },
    ]
});

/// `serverReadyPatterns`.
pub const SERVER_READY_PATTERNS: &[&str] = &[
    "listening",
    "ready",
    "started",
    "running",
    "live",
    "compiled successfully",
    "compiled",
    "waiting for file changes",
    "development server running",
];

/// `errorPatterns`.
pub const ERROR_PATTERNS: &[&str] = &[
    "EADDRINUSE",
    "Error:",
    "error:",
    "ERR!",
    "app crashed",
    "Cannot find module",
    "command not found",
    "Failed to compile",
    "Syntax error:",
    "TypeError:",
    // Windows-specific
    "The system cannot find the file specified",
    "Access is denied",
    "The filename, directory name, or volume label syntax is incorrect",
    "is not recognized as an internal or external command",
    "The process cannot access the file because it is being used by another process",
    "ENOENT",
    "EACCES",
];

/// `detectWaitingForInput`.
pub fn detect_waiting_for_input(output: &str) -> WaitingForInput {
    for p in INPUT_DETECTION_PATTERNS.iter() {
        if p.pattern.is_match(output) {
            return WaitingForInput {
                is_waiting: true,
                prompt: Some((p.prompt_extractor)(output)),
            };
        }
    }
    WaitingForInput {
        is_waiting: false,
        prompt: None,
    }
}

/// `detectServerReady`: case-insensitive substring match.
pub fn detect_server_ready(output: &str) -> bool {
    let lower = output.to_lowercase();
    SERVER_READY_PATTERNS.iter().any(|p| lower.contains(p))
}

/// `detectErrors`.
pub fn detect_errors(stdout: &str, stderr: &str) -> bool {
    if ERROR_PATTERNS
        .iter()
        .any(|p| stdout.contains(p) || stderr.contains(p))
    {
        return true;
    }
    stdout.contains("app crashed") && !stdout.contains("waiting for file changes")
}

#[cfg(test)]
mod tests {
    //! outputPatterns.test.ts
    use super::*;

    // describe('detectWaitingForInput')
    #[test]
    fn detects_an_inquirer_style_question() {
        let r = detect_waiting_for_input("? Which framework do you want?\n");
        assert!(r.is_waiting);
        assert_eq!(r.prompt.as_deref(), Some("Which framework do you want?"));
    }

    #[test]
    fn detects_a_trailing_colon_prompt() {
        let r = detect_waiting_for_input("Enter name: ");
        assert!(r.is_waiting);
        assert_eq!(r.prompt.as_deref(), Some("Enter name: "));
    }

    #[test]
    fn detects_an_apt_dpkg_confirmation() {
        for output in [
            "After this operation, 12.3 MB of additional disk space will be used.\nDo you want to continue? [Y/n] ",
            "Remove the package? [y/N]",
        ] {
            let r = detect_waiting_for_input(output);
            assert!(r.is_waiting, "{output}");
            assert!(r.prompt.unwrap().contains('['), "{output}");
        }
    }

    #[test]
    fn does_not_fire_on_ordinary_output() {
        assert!(!detect_waiting_for_input("Reading package lists... Done\n").is_waiting);
    }

    // describe('detectServerReady')
    #[test]
    fn recognizes_ready_output() {
        for output in [
            "Server listening on port 3000",
            "compiled successfully",
            "waiting for file changes",
        ] {
            assert!(detect_server_ready(output), "{output}");
        }
    }

    #[test]
    fn server_ready_is_case_insensitive() {
        assert!(detect_server_ready("LISTENING"));
    }

    #[test]
    fn server_ready_does_not_fire_on_unrelated_output() {
        assert!(!detect_server_ready("installing dependencies"));
    }

    // describe('detectErrors')
    #[test]
    fn flags_a_known_error_pattern_on_stdout() {
        assert!(detect_errors("Error: something broke", ""));
    }

    #[test]
    fn flags_a_known_error_pattern_on_stderr() {
        assert!(detect_errors("", "command not found"));
    }

    #[test]
    fn flags_app_crashed_even_when_a_watch_loop_message_follows() {
        // 'app crashed' is itself an errorPatterns entry, so the carve-out never runs.
        assert!(detect_errors("app crashed waiting for file changes", ""));
    }

    #[test]
    fn passes_clean_output_through() {
        assert!(!detect_errors("Done in 1.2s", ""));
    }
}
