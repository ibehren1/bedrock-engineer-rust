//! Port of `src/common/mcp/schemas.ts` — the `{ mcpServers: { … } }` config schema.

use common::zod::{array, object, opt, record, req, Issue, Schema};
use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;

/// `mcpServerConfigSchema`
pub fn mcp_server_config_schema() -> &'static Schema {
    static CELL: OnceLock<Schema> = OnceLock::new();
    CELL.get_or_init(|| {
        object(vec![req(
            "mcpServers",
            record(Schema::Union(vec![
                // command server
                object(vec![
                    req("command", Schema::String),
                    req("args", array(Schema::String)),
                    opt("env", record(Schema::String)),
                ]),
                // URL server
                object(vec![
                    req("url", Schema::String),
                    opt("enabled", Schema::Boolean),
                    opt("headers", record(Schema::String)),
                ]),
            ])),
        )])
    })
}

/// `formatZodError`
pub fn format_zod_error(issues: &[Issue]) -> String {
    let lines: Vec<String> = issues
        .iter()
        .map(|i| format!("{}: {}", i.path_string(), i.message))
        .collect();
    if lines.len() == 1 {
        return format!("Validation error: {}", lines[0]);
    }
    format!(
        "Validation errors:\n{}",
        lines
            .iter()
            .map(|l| format!("- {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Result of [`validate_mcp_server_config`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpConfigValidation {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `validateMcpServerConfig`
pub fn validate_mcp_server_config(data: &Value) -> McpConfigValidation {
    match mcp_server_config_schema().safe_parse(data) {
        Ok(parsed) => McpConfigValidation {
            success: true,
            data: Some(parsed),
            error: None,
        },
        Err(issues) => McpConfigValidation {
            success: false,
            data: None,
            error: Some(format_zod_error(&issues)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_command_and_url_servers() {
        let v = validate_mcp_server_config(&json!({
            "mcpServers": {
                "a": { "command": "npx", "args": ["-y", "x"], "extra": 1 },
                "b": { "url": "https://x", "enabled": true }
            }
        }));
        assert!(v.success);
        // unknown keys are stripped like Zod does
        assert_eq!(
            v.data.unwrap()["mcpServers"]["a"],
            json!({ "command": "npx", "args": ["-y", "x"] })
        );
    }

    #[test]
    fn formats_errors_like_zod() {
        let v = validate_mcp_server_config(&json!({ "mcpServers": { "a": { "command": 1 } } }));
        assert_eq!(
            v.error.unwrap(),
            "Validation error: mcpServers.a: Invalid input"
        );
        let v = validate_mcp_server_config(&json!({}));
        assert_eq!(v.error.unwrap(), "Validation error: mcpServers: Required");
    }
}
