//! Output pattern matching for sandbox execs. Port of
//! `src/main/api/command/outputPatterns.ts`, which the host shell path shares; the copy lives
//! here so this crate does not depend on the tools crate (which depends on this one).

use std::sync::OnceLock;

use regex::Regex;

struct InputPattern {
    pattern: Regex,
    extract: fn(&str) -> String,
}

fn input_patterns() -> &'static [InputPattern] {
    static PATTERNS: OnceLock<Vec<InputPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // inquirer-style question
            InputPattern {
                pattern: Regex::new(r"(?m)\? .+\?.*$").unwrap(),
                extract: |output| {
                    static CAPTURE: OnceLock<Regex> = OnceLock::new();
                    let capture = CAPTURE.get_or_init(|| Regex::new(r"(?m)\? (.+\?.*$)").unwrap());
                    capture
                        .captures(output)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| output.to_string())
                },
            },
            // basic prompt, e.g. "Enter name: "
            InputPattern {
                pattern: Regex::new(r"(?m)[^:]+: $").unwrap(),
                extract: |output| output.split('\n').next_back().unwrap_or("").to_string(),
            },
            // apt/dpkg style confirmations ending in "[Y/n]" or "[y/N]".
            InputPattern {
                pattern: Regex::new(r"(?m)\[[Yy]/[Nn]\]\??\s*$").unwrap(),
                extract: |output| {
                    output
                        .trim_end()
                        .split('\n')
                        .next_back()
                        .unwrap_or("")
                        .to_string()
                },
            },
        ]
    })
}

pub const SERVER_READY_PATTERNS: [&str; 9] = [
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

pub const ERROR_PATTERNS: [&str; 17] = [
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
    "The system cannot find the file specified",
    "Access is denied",
    "The filename, directory name, or volume label syntax is incorrect",
    "is not recognized as an internal or external command",
    "The process cannot access the file because it is being used by another process",
    "ENOENT",
    "EACCES",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingForInput {
    pub is_waiting: bool,
    pub prompt: Option<String>,
}

/// Whether the output looks like it is waiting for input, and the prompt if so.
pub fn detect_waiting_for_input(output: &str) -> WaitingForInput {
    for pattern in input_patterns() {
        if pattern.pattern.is_match(output) {
            return WaitingForInput {
                is_waiting: true,
                prompt: Some((pattern.extract)(output)),
            };
        }
    }
    WaitingForInput {
        is_waiting: false,
        prompt: None,
    }
}

/// Whether the output looks like a server that has come up.
pub fn detect_server_ready(output: &str) -> bool {
    let lowered = output.to_lowercase();
    SERVER_READY_PATTERNS
        .iter()
        .any(|pattern| lowered.contains(pattern))
}

/// Whether the output contains a known error marker.
pub fn detect_errors(stdout: &str, stderr: &str) -> bool {
    if ERROR_PATTERNS
        .iter()
        .any(|pattern| stdout.contains(pattern) || stderr.contains(pattern))
    {
        return true;
    }
    stdout.contains("app crashed") && !stdout.contains("waiting for file changes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_prompts() {
        let basic = detect_waiting_for_input("Enter name: ");
        assert!(basic.is_waiting);
        assert_eq!(basic.prompt.as_deref(), Some("Enter name: "));

        let apt = detect_waiting_for_input("Do you want to continue? [Y/n] ");
        assert!(apt.is_waiting);

        let inquirer = detect_waiting_for_input("? Pick one? (Use arrow keys)");
        assert_eq!(
            inquirer.prompt.as_deref(),
            Some("Pick one? (Use arrow keys)")
        );

        assert!(!detect_waiting_for_input("done\n").is_waiting);
    }

    #[test]
    fn detects_servers_and_errors() {
        assert!(detect_server_ready("Server LISTENING on 3000"));
        assert!(!detect_server_ready("hello"));
        assert!(detect_errors("Error: boom", ""));
        assert!(detect_errors("", "EADDRINUSE"));
        assert!(!detect_errors("ok", "fine"));
    }
}
