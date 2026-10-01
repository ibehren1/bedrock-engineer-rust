//! Ports of `src/test/agent-schema-validation.test.ts` and
//! `src/test/directory-agents-validation.test.ts`.
//!
//! The directory agents live in the renderer (`src/renderer/src/assets/directory-agents`), which
//! stays in the repo after the port.

use crate::agent::*;
use crate::agent_files::parse_yaml;
use crate::validation::{validate_custom_agent, validate_custom_agents, ValidationContext};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn directory_agents_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../src/renderer/src/assets/directory-agents")
}

fn yaml_files() -> Vec<String> {
    let mut files: Vec<String> = fs::read_dir(directory_agents_path())
        .expect("directory agents folder")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".yaml") || f.ends_with(".yml"))
        .collect();
    files.sort();
    files
}

fn load(file: &str) -> Value {
    let content = fs::read_to_string(directory_agents_path().join(file)).unwrap();
    parse_yaml(&content).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn ok(schema: &crate::zod::Schema, v: Value) -> bool {
    schema.safe_parse(&v).is_ok()
}

// ---- agent-schema-validation.test.ts ----------------------------------------------------------

#[test]
fn command_config_valid() {
    assert!(ok(
        command_config_schema(),
        json!({ "pattern": "^npm (install|i).*", "description": "npm install command" })
    ));
}

#[test]
fn command_config_rejects_missing_fields() {
    assert!(!ok(command_config_schema(), json!({ "pattern": "^npm.*" })));
}

#[test]
fn window_config_valid() {
    assert!(ok(
        window_config_schema(),
        json!({ "id": "chrome-window", "name": "Google Chrome", "enabled": true })
    ));
}

#[test]
fn window_config_rejects_missing_fields() {
    assert!(!ok(
        window_config_schema(),
        json!({ "id": "test", "enabled": true })
    ));
}

#[test]
fn camera_config_valid() {
    assert!(ok(
        camera_config_schema(),
        json!({ "id": "camera-1", "name": "FaceTime HD Camera", "enabled": false })
    ));
}

#[test]
fn scenario_valid() {
    assert!(ok(
        scenario_schema(),
        json!({ "title": "Test Scenario", "content": "Scenario content here" })
    ));
}

#[test]
fn knowledge_base_valid() {
    assert!(ok(
        knowledge_base_schema(),
        json!({ "knowledgeBaseId": "KB123456", "description": "Product documentation" })
    ));
}

#[test]
fn flow_config_valid() {
    assert!(ok(
        flow_config_schema(),
        json!({
            "flowIdentifier": "FLOW123",
            "flowAliasIdentifier": "ALIAS123",
            "description": "Test flow",
            "inputType": "object",
            "schema": { "type": "object", "properties": {} }
        })
    ));
}

#[test]
fn flow_config_without_optional_fields() {
    assert!(ok(
        flow_config_schema(),
        json!({ "flowIdentifier": "FLOW123", "flowAliasIdentifier": "ALIAS123", "description": "Test flow" })
    ));
}

#[test]
fn mcp_server_config_command() {
    assert!(ok(
        mcp_server_config_schema(),
        json!({
            "name": "test-server",
            "description": "Test MCP server",
            "connectionType": "command",
            "command": "node",
            "args": ["server.js"],
            "env": { "NODE_ENV": "production" }
        })
    ));
}

#[test]
fn mcp_server_config_url() {
    assert!(ok(
        mcp_server_config_schema(),
        json!({
            "name": "api-server",
            "description": "API MCP server",
            "connectionType": "url",
            "url": "https://api.example.com",
            "headers": { "Authorization": "Bearer token" }
        })
    ));
}

#[test]
fn tavily_search_config_valid() {
    assert!(ok(
        tavily_search_config_schema(),
        json!({ "includeDomains": ["example.com", "test.com"], "excludeDomains": ["spam.com"] })
    ));
}

#[test]
fn tavily_search_config_empty_arrays() {
    assert!(ok(
        tavily_search_config_schema(),
        json!({ "includeDomains": [], "excludeDomains": [] })
    ));
}

#[test]
fn environment_context_settings_valid() {
    assert!(ok(
        environment_context_settings_schema(),
        json!({ "projectRule": true, "visualExpressionRules": false })
    ));
}

#[test]
fn environment_context_settings_rejects_extra_properties() {
    let err = environment_context_settings_schema()
        .safe_parse(&json!({ "projectRule": true, "visualExpressionRules": false, "todoListInstruction": true }))
        .unwrap_err();
    assert_eq!(
        err[0].message,
        "Unrecognized key(s) in object: 'todoListInstruction'"
    );
}

#[test]
fn environment_context_settings_rejects_missing_properties() {
    assert!(!ok(
        environment_context_settings_schema(),
        json!({ "projectRule": true })
    ));
}

#[test]
fn custom_agent_minimal_valid() {
    assert!(ok(
        custom_agent_schema(),
        json!({
            "id": "test-agent",
            "name": "Test Agent",
            "description": "A test agent",
            "system": "System prompt",
            "scenarios": [{ "title": "Test", "content": "Test scenario" }]
        })
    ));
}

fn full_agent() -> Value {
    json!({
        "id": "test-agent",
        "name": "Test Agent",
        "description": "A test agent",
        "system": "System prompt",
        "scenarios": [],
        "icon": "robot",
        "iconColor": "#FF0000",
        "tags": ["test", "example"],
        "author": "Test Author",
        "isCustom": true,
        "isShared": false,
        "directoryOnly": false,
        "organizationId": "org-123",
        "tools": ["read_file", "write_file"],
        "category": "coding",
        "allowedCommands": [{ "pattern": "^npm.*", "description": "npm commands" }],
        "allowedWindows": [{ "id": "chrome", "name": "Chrome", "enabled": true }],
        "allowedCameras": [{ "id": "cam1", "name": "Camera 1", "enabled": false }],
        "knowledgeBases": [{ "knowledgeBaseId": "KB123", "description": "Docs" }],
        "flows": [{ "flowIdentifier": "FLOW1", "flowAliasIdentifier": "ALIAS1", "description": "Flow 1" }],
        "mcpServers": [{ "name": "server1", "description": "Server 1", "command": "node", "args": ["server.js"] }],
        "tavilySearchConfig": { "includeDomains": ["example.com"], "excludeDomains": [] },
        "additionalInstruction": "Extra instructions",
        "environmentContextSettings": { "projectRule": true, "visualExpressionRules": true }
    })
}

#[test]
fn custom_agent_with_all_optional_fields() {
    let agent = full_agent();
    let parsed = custom_agent_schema().safe_parse(&agent).unwrap();
    // Zod output == input here (schema order == input order, no unknown keys)
    assert_eq!(
        serde_json::to_string(&parsed).unwrap(),
        serde_json::to_string(&agent).unwrap()
    );
    // The serde type round-trips to the same camelCase JSON
    let typed: CustomAgent = serde_json::from_value(agent.clone()).unwrap();
    assert_eq!(
        serde_json::to_string(&typed).unwrap(),
        serde_json::to_string(&agent).unwrap()
    );
}

#[test]
fn icon_accepts_iconify_ids_and_rejects_others() {
    let mut agent = full_agent();
    agent["icon"] = json!("tabler:rocket");
    assert!(ok(custom_agent_schema(), agent.clone()));
    agent["icon"] = json!("Not An Icon");
    let err = custom_agent_schema().safe_parse(&agent).unwrap_err();
    assert_eq!(err[0].path_string(), "icon");
    assert_eq!(
        err[0].message,
        "Expected an Iconify icon id like \"tabler:rocket\""
    );
}

// describe('Directory Agents Validation')
#[test]
fn every_directory_agent_passes_custom_agent_schema() {
    for file in yaml_files() {
        let agent = load(&file);
        if let Err(errors) = custom_agent_schema().safe_parse(&agent) {
            panic!("Validation failed for {file}: {errors:#?}");
        }
    }
}

#[test]
fn every_directory_agent_has_valid_structure_with_validate_custom_agent() {
    for file in yaml_files() {
        let agent = load(&file);
        let result = validate_custom_agent(
            &agent,
            Some(&ValidationContext {
                source: Some("test"),
                file_path: Some(&file),
            }),
        );
        assert!(result.data.get("id").is_some(), "{file}");
        assert!(result.data.get("name").is_some(), "{file}");
    }
}

// describe('Validation Utilities')
#[test]
fn validates_multiple_agents() {
    let agents = vec![
        json!({ "id": "agent1", "name": "Agent 1", "description": "First agent", "system": "System 1", "scenarios": [] }),
        json!({ "id": "agent2", "name": "Agent 2", "description": "Second agent", "system": "System 2", "scenarios": [] }),
    ];
    let results = validate_custom_agents(&agents, Some("test"));
    assert_eq!(results.len(), 2);
    assert!(results[0].success);
    assert!(results[1].success);
}

#[test]
fn handles_validation_errors_gracefully() {
    let invalid = json!({ "id": "test", "scenarios": [] });
    let result = validate_custom_agent(
        &invalid,
        Some(&ValidationContext {
            source: Some("test"),
            file_path: Some("test.yaml"),
        }),
    );
    // In warning-only mode, data is still returned
    assert_eq!(result.data, invalid);
    assert!(!result.success);
    let errors = result.formatted_errors();
    assert_eq!(errors.len(), 3);
    assert_eq!(errors[0].path, "name");
    assert_eq!(errors[0].message, "Required");
}

#[test]
fn summary_counts() {
    let results = validate_custom_agents(&[json!({ "id": "x" }), full_agent()], Some("test"));
    let s = crate::validation::get_validation_summary(&results);
    assert_eq!((s.total, s.valid, s.invalid), (2, 1, 1));
    assert_eq!(s.error_count, 4);
}

// describe('File Statistics')
#[test]
fn validates_all_directory_agents_successfully() {
    let files = yaml_files();
    assert!(!files.is_empty());
    let agents: Vec<Value> = files.iter().map(|f| load(f)).collect();
    let results = validate_custom_agents(&agents, Some("directory-agents"));
    assert!(results.iter().all(|r| r.success));
}

// ---- directory-agents-validation.test.ts ------------------------------------------------------

#[test]
fn directory_agents_environment_context_settings() {
    let files = yaml_files();
    assert!(!files.is_empty());
    for file in files {
        let agent = load(&file);
        let settings = agent
            .get("environmentContextSettings")
            .unwrap_or_else(|| panic!("{file}: environmentContextSettings missing"));
        // should not contain todoListInstruction property
        assert!(settings.get("todoListInstruction").is_none(), "{file}");
        // should have valid environmentContextSettings structure
        assert!(
            environment_context_settings_schema()
                .safe_parse(settings)
                .is_ok(),
            "{file}"
        );
        // should have only projectRule and visualExpressionRules properties
        let keys: Vec<&String> = settings.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 2, "{file}");
        assert!(keys.iter().any(|k| *k == "projectRule"));
        assert!(keys.iter().any(|k| *k == "visualExpressionRules"));
        // should have boolean values for all properties
        assert!(settings["projectRule"].is_boolean(), "{file}");
        assert!(settings["visualExpressionRules"].is_boolean(), "{file}");
        // and the serde type accepts it
        let _: EnvironmentContextSettings = serde_json::from_value(settings.clone()).unwrap();
    }
}

#[test]
fn directory_agents_deserialize_into_the_serde_type() {
    for file in yaml_files() {
        let agent = load(&file);
        let parsed = custom_agent_schema().safe_parse(&agent).unwrap();
        let typed: CustomAgent =
            serde_json::from_value(parsed.clone()).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(serde_json::to_value(&typed).unwrap(), parsed, "{file}");
    }
}
