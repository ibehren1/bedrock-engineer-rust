//! Port of `src/main/services/strandsAgentsConverter/` (`StrandsAgentsConverter`): turn a
//! Bedrock Engineer `CustomAgent` into a runnable Strands Agents project (`agent.py`,
//! `requirements.txt`, `README.md`, optionally `config.yaml`).
//!
//! Not ported (nothing called them): `convertMultipleAgents`, `getConversionStats`,
//! `getSupportedTools`, `getUnsupportedTools`, and the legacy `generatePythonCode` template.

pub mod generator;
pub mod template;
pub mod tool_mapper;

pub use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub use generator::{generate_strands_agent, StrandsAgentOutput};

/// `SaveOptions`
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub output_directory: String,
    /// `agent.py` when `None`.
    pub agent_file_name: Option<String>,
    pub include_config: bool,
    pub overwrite: bool,
}

/// One entry of `SaveResult.errors`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileError {
    pub file: String,
    pub error: String,
}

/// `SaveResult`, serialized as the renderer receives it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub success: bool,
    pub output_directory: String,
    pub saved_files: Vec<String>,
    pub errors: Vec<FileError>,
}

/// `validateAgent`
fn validate_agent(agent: &Value) -> Result<(), String> {
    let name = agent.get("name").and_then(Value::as_str).unwrap_or("");
    if template::js_trim(name).is_empty() {
        return Err("Agent name is required".into());
    }
    let system = agent.get("system").and_then(Value::as_str).unwrap_or("");
    if template::js_trim(system).is_empty() {
        return Err("Agent system prompt is required".into());
    }
    if !agent
        .get("description")
        .and_then(Value::as_str)
        .is_some_and(|d| !d.is_empty())
    {
        tracing::warn!("Agent {name} has no description");
    }
    if let Some(tools) = agent.get("tools").and_then(Value::as_array) {
        let mut seen: Vec<&Value> = Vec::new();
        for t in tools {
            if seen.contains(&t) {
                tracing::warn!("Agent {name} has duplicate tools");
                break;
            }
            seen.push(t);
        }
    }
    Ok(())
}

/// `convertAgent`: validate and convert.
pub fn convert_agent(agent: &Value, now: DateTime<Utc>) -> Result<StrandsAgentOutput, String> {
    let name = generator::js_string(agent.get("name"));
    tracing::info!("Converting agent: {name}");
    let result = validate_agent(agent).and_then(|()| generate_strands_agent(agent, now));
    match &result {
        Ok(output) => {
            tracing::info!("Successfully converted agent: {name}");
            tracing::info!(
                "Supported tools: {}",
                output.tool_mapping.supported_tools.len()
            );
            tracing::info!(
                "Unsupported tools: {}",
                output.tool_mapping.unsupported_tools.len()
            );
            tracing::info!("MCP servers: {}", output.mcp_servers.len());
            for s in &output.mcp_servers {
                tracing::info!(
                    "  - {}: {}",
                    generator::js_string(s.original.get("name")),
                    generator::js_string(s.original.get("command"))
                );
            }
        }
        Err(e) => tracing::error!("Failed to convert agent: {name} {e}"),
    }
    result
}

/// `convertAndSaveAgent`: a conversion failure becomes a `conversion` error entry.
pub fn convert_and_save_agent(
    agent: &Value,
    options: &SaveOptions,
    now: DateTime<Utc>,
) -> SaveResult {
    tracing::info!(
        "Converting and saving agent: {}",
        generator::js_string(agent.get("name"))
    );
    match convert_agent(agent, now) {
        Ok(output) => save_agent_to_directory(&output, options),
        Err(error) => SaveResult {
            success: false,
            output_directory: options.output_directory.clone(),
            saved_files: Vec::new(),
            errors: vec![FileError {
                file: "conversion".into(),
                error,
            }],
        },
    }
}

/// `validateSaveOptions`
fn validate_save_options(options: &SaveOptions) -> Result<(), String> {
    if template::js_trim(&options.output_directory).is_empty() {
        return Err("Output directory is required".into());
    }
    if options
        .agent_file_name
        .as_deref()
        .is_some_and(|f| !f.is_empty() && !f.ends_with(".py"))
    {
        return Err("Agent file name must end with .py".into());
    }
    Ok(())
}

/// `saveAgentToDirectory`: write the files, collecting per-file errors. Succeeds only when at
/// least one file was written and nothing failed.
pub fn save_agent_to_directory(output: &StrandsAgentOutput, options: &SaveOptions) -> SaveResult {
    let mut result = SaveResult {
        success: false,
        output_directory: options.output_directory.clone(),
        saved_files: Vec::new(),
        errors: Vec::new(),
    };

    let dir = PathBuf::from(&options.output_directory);
    if let Err(error) = validate_save_options(options)
        .and_then(|()| std::fs::create_dir_all(&dir).map_err(|e| e.to_string()))
    {
        tracing::error!("Failed to save agent files: {error}");
        result.errors.push(FileError {
            file: "directory".into(),
            error,
        });
        return result;
    }
    tracing::info!("Saving agent files to: {}", options.output_directory);

    let agent_file = options
        .agent_file_name
        .clone()
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "agent.py".into());
    let mut files: Vec<(String, &str, &str)> = vec![
        (agent_file, &output.python_code, "Python agent code"),
        (
            "requirements.txt".into(),
            &output.requirements_text,
            "Python dependencies",
        ),
        (
            "README.md".into(),
            &output.readme_text,
            "Usage documentation",
        ),
    ];
    if options.include_config {
        files.push((
            "config.yaml".into(),
            &output.config_yaml_text,
            "Agent configuration",
        ));
    }

    for (name, content, description) in files {
        let path = dir.join(&name);
        if !options.overwrite && Path::new(&path).exists() {
            let error = format!("File already exists: {name} (use overwrite: true to replace)");
            tracing::warn!("{error}");
            result.errors.push(FileError { file: name, error });
            continue;
        }
        match std::fs::write(&path, content) {
            Ok(()) => {
                result.saved_files.push(path.to_string_lossy().into_owned());
                tracing::info!("Saved {description}: {name}");
            }
            Err(e) => {
                let error = e.to_string();
                tracing::error!("Failed to save {name}: {error}");
                result.errors.push(FileError { file: name, error });
            }
        }
    }

    result.success = !result.saved_files.is_empty() && result.errors.is_empty();
    if result.success {
        tracing::info!(
            "Successfully saved {} files to {}",
            result.saved_files.len(),
            options.output_directory
        );
    } else {
        tracing::warn!(
            "Partial success: saved {} files with {} errors",
            result.saved_files.len(),
            result.errors.len()
        );
    }
    result
}

#[cfg(test)]
mod tests;
