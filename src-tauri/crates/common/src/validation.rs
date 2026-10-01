//! Port of `src/common/validation/agent-validator.ts`.
//!
//! Warning-only validation: failures are logged (category `validation:agent`) and the original
//! data is handed back unchanged; successes return Zod's parsed output (unknown keys stripped).

use crate::agent::custom_agent_schema;
use crate::zod::{FormattedIssue, Issue};
use serde::Serialize;
use serde_json::Value;

const CATEGORY: &str = "validation:agent";

/// Context for log messages (`{ source, filePath }`).
#[derive(Debug, Clone, Default)]
pub struct ValidationContext<'a> {
    pub source: Option<&'a str>,
    pub file_path: Option<&'a str>,
}

/// `ValidationResult<CustomAgent>`. `data` is JSON so it round-trips to the renderer untouched.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub success: bool,
    pub data: Value,
    /// `None` on success, the Zod issues on failure.
    pub errors: Option<Vec<Issue>>,
}

impl ValidationResult {
    /// `formatZodErrors(result.error)`
    pub fn formatted_errors(&self) -> Vec<FormattedIssue> {
        self.errors
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(FormattedIssue::from)
            .collect()
    }
}

/// `validateCustomAgent(agent, context)`
pub fn validate_custom_agent(
    agent: &Value,
    context: Option<&ValidationContext>,
) -> ValidationResult {
    match custom_agent_schema().safe_parse(agent) {
        Ok(parsed) => {
            tracing::debug!(
                category = CATEGORY,
                agent_id = parsed
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
                agent_name = parsed
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
                source = context.and_then(|c| c.source).unwrap_or_default(),
                "CustomAgent validation succeeded"
            );
            ValidationResult {
                success: true,
                data: parsed,
                errors: None,
            }
        }
        Err(issues) => {
            let context_info = context
                .map(|c| {
                    format!(
                        "[{}{}]",
                        c.source.filter(|s| !s.is_empty()).unwrap_or("unknown"),
                        c.file_path
                            .filter(|p| !p.is_empty())
                            .map(|p| format!(": {p}"))
                            .unwrap_or_default()
                    )
                })
                .unwrap_or_default();
            let formatted: Vec<FormattedIssue> = issues.iter().map(FormattedIssue::from).collect();
            tracing::warn!(
                category = CATEGORY,
                agent_id = agent.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
                agent_name = agent.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                errors = %serde_json::to_string(&formatted).unwrap_or_default(),
                "CustomAgent validation failed {context_info}"
            );
            ValidationResult {
                success: false,
                data: agent.clone(),
                errors: Some(issues),
            }
        }
    }
}

/// `validateCustomAgents(agents, context)` — each agent's `filePath` becomes `[index: i]`.
pub fn validate_custom_agents(agents: &[Value], source: Option<&str>) -> Vec<ValidationResult> {
    agents
        .iter()
        .enumerate()
        .map(|(index, agent)| {
            let file_path = format!("[index: {index}]");
            validate_custom_agent(
                agent,
                Some(&ValidationContext {
                    source,
                    file_path: Some(&file_path),
                }),
            )
        })
        .collect()
}

/// `getValidationSummary(results)`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSummary {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub error_count: usize,
}

pub fn get_validation_summary(results: &[ValidationResult]) -> ValidationSummary {
    let valid = results.iter().filter(|r| r.success).count();
    ValidationSummary {
        total: results.len(),
        valid,
        invalid: results.len() - valid,
        error_count: results
            .iter()
            .map(|r| r.errors.as_ref().map_or(0, Vec::len))
            .sum(),
    }
}
