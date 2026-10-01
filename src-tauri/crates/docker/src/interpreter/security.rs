//! Security constraints for Code Interpreter containers. Port of `SecurityManager.ts`.

use std::sync::Arc;

use serde_json::json;

use super::logger::ToolLogger;
use super::types::{ExecutionConfig, PartialExecutionConfig};

/// Base constraints; memory and CPU are added from the user's configuration.
const BASE_DOCKER_SECURITY_ARGS: [&str; 4] = [
    "--rm",
    "--network=none",
    "--tmpfs=/tmp:rw,size=100m",
    "--tmpfs=/var/tmp:rw,size=50m",
];

pub const VALID_MEMORY_LIMITS: [&str; 5] = ["128m", "256m", "512m", "1g", "2g"];

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub sanitized_config: ExecutionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedCode {
    pub sanitized_code: String,
    pub warnings: Vec<String>,
}

pub struct SecurityManager {
    logger: Arc<dyn ToolLogger>,
}

impl SecurityManager {
    pub fn new(logger: Arc<dyn ToolLogger>) -> Self {
        Self { logger }
    }

    /// Validate an execution config and apply the defaults (30s, 256m, 0.5 CPU).
    pub fn validate_execution_config(
        &self,
        config: Option<&PartialExecutionConfig>,
    ) -> ConfigValidation {
        let mut errors = Vec::new();
        let mut sanitized = ExecutionConfig {
            timeout: 30.0,
            memory_limit: "256m".to_string(),
            cpu_limit: 0.5,
            environment: None,
        };

        if let Some(config) = config {
            if let Some(timeout) = config.timeout {
                if timeout > 0.0 && timeout <= 600.0 {
                    sanitized.timeout = timeout;
                } else {
                    errors.push("Timeout must be between 1 and 600 seconds".to_string());
                }
            }
            if let Some(memory) = &config.memory_limit {
                if VALID_MEMORY_LIMITS.contains(&memory.as_str()) {
                    sanitized.memory_limit = memory.clone();
                } else {
                    errors.push(format!(
                        "Memory limit must be one of: {}",
                        VALID_MEMORY_LIMITS.join(", ")
                    ));
                }
            }
            if let Some(cpu) = config.cpu_limit {
                if cpu > 0.0 && cpu <= 4.0 {
                    sanitized.cpu_limit = cpu;
                } else {
                    errors.push("CPU limit must be between 0.1 and 4.0".to_string());
                }
            }
            if config.environment.is_some() {
                sanitized.environment = config.environment;
            }
        }

        self.logger.debug(
            "Execution config validated",
            json!({
                "providedConfig": config,
                "sanitizedConfig": sanitized,
                "hasErrors": !errors.is_empty(),
            }),
        );

        ConfigValidation {
            is_valid: errors.is_empty(),
            errors,
            sanitized_config: sanitized,
        }
    }

    /// Basic checks only; Docker handles the actual isolation.
    pub fn sanitize_code(&self, code: &str, _language: &str) -> SanitizedCode {
        let mut warnings = Vec::new();
        if code.contains("__import__") {
            warnings.push("Dynamic imports detected - use with caution".to_string());
        }
        if code.contains("exec(") || code.contains("eval(") {
            warnings.push("Dynamic code execution detected - use with caution".to_string());
        }
        if code.contains("subprocess") || code.contains("os.system") {
            warnings
                .push("System command execution detected - will be blocked by Docker".to_string());
        }
        SanitizedCode {
            sanitized_code: code.to_string(),
            warnings,
        }
    }

    /// `docker run` security arguments for the given configuration.
    pub fn generate_docker_security_args(&self, config: Option<&ExecutionConfig>) -> Vec<String> {
        let mut args: Vec<String> = BASE_DOCKER_SECURITY_ARGS
            .iter()
            .map(|a| a.to_string())
            .collect();
        match config {
            Some(config) => {
                args.push(format!("--memory={}", config.memory_limit));
                // `{}` on f64 prints like JS: 0.5 -> "0.5", 2.0 -> "2".
                args.push(format!("--cpus={}", config.cpu_limit));
            }
            None => {
                args.push("--memory=256m".to_string());
                args.push("--cpus=0.5".to_string());
            }
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::logger::LogFacadeLogger;

    fn manager() -> SecurityManager {
        SecurityManager::new(Arc::new(LogFacadeLogger))
    }

    #[test]
    fn applies_defaults_and_validates_ranges() {
        let validation = manager().validate_execution_config(None);
        assert!(validation.is_valid);
        assert_eq!(validation.sanitized_config.timeout, 30.0);
        assert_eq!(validation.sanitized_config.memory_limit, "256m");
        assert_eq!(validation.sanitized_config.cpu_limit, 0.5);

        let invalid = manager().validate_execution_config(Some(&PartialExecutionConfig {
            timeout: Some(0.0),
            memory_limit: Some("64g".into()),
            cpu_limit: Some(8.0),
            environment: None,
        }));
        assert!(!invalid.is_valid);
        assert_eq!(invalid.errors.len(), 3);
        assert_eq!(
            invalid.errors[1],
            "Memory limit must be one of: 128m, 256m, 512m, 1g, 2g"
        );
    }

    #[test]
    fn builds_security_args() {
        let config = ExecutionConfig {
            timeout: 30.0,
            memory_limit: "128m".into(),
            cpu_limit: 2.0,
            environment: None,
        };
        assert_eq!(
            manager().generate_docker_security_args(Some(&config)),
            [
                "--rm",
                "--network=none",
                "--tmpfs=/tmp:rw,size=100m",
                "--tmpfs=/var/tmp:rw,size=50m",
                "--memory=128m",
                "--cpus=2"
            ]
        );
        assert_eq!(
            manager().generate_docker_security_args(None)[5],
            "--cpus=0.5"
        );
    }

    #[test]
    fn warns_about_risky_code_but_returns_it_unchanged() {
        let sanitized = manager().sanitize_code("import subprocess\neval('1')", "python");
        assert_eq!(sanitized.sanitized_code, "import subprocess\neval('1')");
        assert_eq!(sanitized.warnings.len(), 2);
    }
}
