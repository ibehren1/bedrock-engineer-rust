//! `window.api.strandsConverter.convertAndSave` → `convert_agent_to_strands` (the
//! `convert-agent-to-strands` handler in `src/main/handlers/agent-handlers.ts`), over the
//! `strands` crate.

use crate::state::{project_path, store_all, StoreMutex};
use serde_json::{json, Value};
use std::path::Path;
use tauri::State;

const CATEGORY: &str = "agents:ipc";

/// The agent to convert: the project's shared agents on disk first (the main-process loader,
/// which keeps only `shared-…` ids), then the store's `customAgents`.
fn find_agent(project: Option<&Path>, store: &Value, agent_id: &str) -> Option<Value> {
    let has_id = |a: &Value| a.get("id").and_then(Value::as_str) == Some(agent_id);
    common::agent_files::load_shared_agents(project)
        .agents
        .into_iter()
        .find(has_id)
        .or_else(|| {
            store
                .get("customAgents")
                .and_then(Value::as_array)
                .and_then(|agents| agents.iter().find(|a| has_id(a)).cloned())
        })
}

fn convert(project: Option<&Path>, store: &Value, agent_id: &str, output_directory: &str) -> Value {
    tracing::info!(
        category = CATEGORY,
        agent_id,
        output_directory,
        "Converting agent to Strands Agents"
    );
    let Some(agent) = find_agent(project, store, agent_id) else {
        return json!({ "success": false, "error": format!("Agent with ID {agent_id} not found") });
    };
    let options = ::strands::SaveOptions {
        output_directory: output_directory.to_string(),
        include_config: false,
        overwrite: true,
        ..Default::default()
    };
    let result = ::strands::convert_and_save_agent(&agent, &options, ::strands::Utc::now());
    tracing::info!(
        category = CATEGORY,
        success = result.success,
        saved_files = result.saved_files.len(),
        "Strands Agents conversion completed"
    );
    serde_json::to_value(result).unwrap_or_else(|e| {
        let error = e.to_string();
        json!({
            "success": false,
            "error": error,
            "outputDirectory": output_directory,
            "savedFiles": [],
            "errors": [{ "file": "conversion", "error": error }]
        })
    })
}

/// `{ agentId, outputDirectory }` → `SaveResult`, or `{ success: false, error }` when the agent
/// doesn't exist. Never rejects.
#[tauri::command]
pub async fn convert_agent_to_strands(
    store: State<'_, StoreMutex>,
    agent_id: String,
    output_directory: Option<String>,
) -> Result<Value, String> {
    let project = project_path(&store);
    let snapshot = store_all(&store);
    let output_directory = output_directory.unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        convert(project.as_deref(), &snapshot, &agent_id, &output_directory)
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_custom_agent_from_the_store() {
        let out = tempfile::tempdir().unwrap();
        let store =
            json!({ "customAgents": [{ "id": "mine", "name": "Mine", "system": "Be nice" }] });
        let dir = out.path().to_string_lossy().into_owned();
        let r = convert(None, &store, "mine", &dir);
        assert_eq!(r["success"], true, "{r}");
        assert_eq!(r["outputDirectory"], dir);
        assert_eq!(r["savedFiles"].as_array().unwrap().len(), 3);
        assert!(out.path().join("agent.py").exists());
    }

    #[test]
    fn prefers_shared_agents_and_reports_missing_ones() {
        let project = tempfile::tempdir().unwrap();
        let agents_dir = project.path().join(".bedrock-engineer/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("a.yaml"),
            "id: shared-a-1\nname: Shared\ndescription: d\nsystem: From disk\nscenarios: []\n",
        )
        .unwrap();
        let store =
            json!({ "customAgents": [{ "id": "shared-a-1", "name": "Stored", "system": "x" }] });
        let out = tempfile::tempdir().unwrap();
        let r = convert(
            Some(project.path()),
            &store,
            "shared-a-1",
            &out.path().to_string_lossy(),
        );
        assert_eq!(r["success"], true, "{r}");
        let code = std::fs::read_to_string(out.path().join("agent.py")).unwrap();
        assert!(code.contains("Agent: Shared\n"));
        assert!(code.contains("SYSTEM_PROMPT = \"\"\"From disk\"\"\""));

        assert_eq!(
            convert(None, &json!({}), "nope", "/tmp"),
            json!({ "success": false, "error": "Agent with ID nope not found" })
        );
    }
}
