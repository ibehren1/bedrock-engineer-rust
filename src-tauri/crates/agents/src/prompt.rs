//! System prompt and first-message assembly for Rust-side agent runs — the
//! `buildSystemPrompt` / `buildContext` / `buildMessages` helpers of
//! `src/main/api/bedrock/services/backgroundAgent/BackgroundAgentService.ts`.

use common::tool_rules::{generate_environment_context, ContextSettings};
use serde_json::Value;
use tools::util::js::truthy;

/// `projectDirectory || store.get('projectPath') || ''`.
pub fn working_directory(project_directory: Option<&str>, store: &Value) -> String {
    project_directory
        .filter(|s| !s.is_empty())
        .or_else(|| crate::store::project_path(store))
        .unwrap_or_default()
        .to_string()
}

/// `JSON.stringify(agent[key] || [])`.
fn json_list(agent: &Value, key: &str) -> String {
    match agent.get(key) {
        v @ Some(_) if truthy(v) => v.map(Value::to_string).unwrap_or_default(),
        _ => "[]".to_string(),
    }
}

/// `replacePlaceholders(text, { projectPath, allowedCommands, allowedWindows, allowedCameras,
/// knowledgeBases, bedrockAgents, flows })` with every list taken from `agent` (`|| []`).
///
/// Same substitutions, in the same order, as `common::placeholders::replace_placeholders`, but
/// the lists are serialized from the agent JSON as stored rather than through the typed
/// `PlaceholderValues`, so entries with missing or extra fields render exactly as
/// `JSON.stringify` rendered them.
pub fn replace_agent_placeholders(text: &str, agent: &Value, project_path: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    replace_agent_placeholders_with_date(text, agent, project_path, &today)
}

/// [`replace_agent_placeholders`] with an explicit `yyyy-MM-dd` date.
pub fn replace_agent_placeholders_with_date(
    text: &str,
    agent: &Value,
    project_path: &str,
    yyyy_mm_dd: &str,
) -> String {
    text.replace("{{projectPath}}", project_path)
        .replace("{{date}}", yyyy_mm_dd)
        .replace("{{allowedCommands}}", &json_list(agent, "allowedCommands"))
        .replace("{{allowedWindows}}", &json_list(agent, "allowedWindows"))
        .replace("{{allowedCameras}}", &json_list(agent, "allowedCameras"))
        .replace("{{knowledgeBases}}", &json_list(agent, "knowledgeBases"))
        .replace("{{bedrockAgents}}", &json_list(agent, "bedrockAgents"))
        .replace("{{flows}}", &json_list(agent, "flows"))
}

/// `SystemPromptBuilder.generateEnvironmentContext(agent.environmentContextSettings)`.
pub fn environment_context(agent: &Value) -> String {
    let settings = agent
        .get("environmentContextSettings")
        .filter(|v| truthy(Some(v)))
        .map(|s| ContextSettings {
            project_rule: Some(truthy(s.get("projectRule"))),
            visual_expression_rules: Some(truthy(s.get("visualExpressionRules"))),
            act_plan_mode_rules: Some(truthy(s.get("actPlanModeRules"))),
        });
    generate_environment_context(settings.as_ref(), None)
}

/// `buildSystemPrompt(agent, projectDirectory)`: `''` when the agent has no system prompt,
/// otherwise `agent.system + '\n\n' + environmentContext` with placeholders replaced.
/// `working_directory` is [`working_directory`]`(projectDirectory, store)`.
pub fn build_system_prompt(agent: &Value, working_directory: &str) -> String {
    let system = match agent.get("system").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s,
        _ => return String::new(),
    };
    let full = format!("{system}\n\n{}", environment_context(agent));
    replace_agent_placeholders(&full, agent, working_directory)
}

/// JS `String(new Date())` in local time, e.g. `Wed Sep 30 2026 09:55:00 GMT+0900`.
/// The trailing `(Zone Name)` JS appends is not available portably and is omitted.
pub fn js_date_string(now: chrono::DateTime<chrono::Local>) -> String {
    now.format("%a %b %d %Y %H:%M:%S GMT%z").to_string()
}

/// `buildContext()`: the date block appended to every user message of a run. The unclosed
/// `<context>` tag is verbatim from the TS.
pub fn build_context(now: chrono::DateTime<chrono::Local>) -> String {
    format!("<context>\nDate: {}\n<context>\n", js_date_string(now))
}

/// The user message text `buildMessages` sends: `userMessage + '\n' + buildContext()`.
pub fn user_message_text(user_message: &str) -> String {
    format!("{user_message}\n{}", build_context(chrono::Local::now()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn working_directory_prefers_the_run_directory() {
        let store = json!({"projectPath": "/store"});
        assert_eq!(working_directory(Some("/run"), &store), "/run");
        assert_eq!(working_directory(Some(""), &store), "/store");
        assert_eq!(working_directory(None, &json!({})), "");
    }

    #[test]
    fn placeholders_use_the_agent_lists_verbatim() {
        let agent = json!({
            "allowedCommands": [{"pattern": "ls *"}],
            "flows": null
        });
        let out = replace_agent_placeholders_with_date(
            "{{projectPath}}|{{date}}|{{allowedCommands}}|{{flows}}|{{knowledgeBases}}",
            &agent,
            "/p",
            "2026-09-30",
        );
        assert_eq!(out, r#"/p|2026-09-30|[{"pattern":"ls *"}]|[]|[]"#);
    }

    #[test]
    fn system_prompt_is_empty_without_agent_system() {
        assert_eq!(build_system_prompt(&json!({"system": ""}), "/p"), "");
        assert_eq!(build_system_prompt(&json!({}), "/p"), "");
    }

    #[test]
    fn system_prompt_appends_environment_context_and_replaces_placeholders() {
        let agent = json!({
            "system": "You work in {{projectPath}}.",
            "environmentContextSettings": {"projectRule": true, "visualExpressionRules": false}
        });
        let out = build_system_prompt(&agent, "/proj");
        assert!(out.starts_with(
            "You work in /proj.\n\n**<context>**\n\n- working directory: /proj\n- date: "
        ));
        assert!(out.contains("<project rule>"));
        assert!(out.contains("/proj/.bedrock-engineer/rules"));
        assert!(!out.contains("<visual expression rule>"));
        assert!(!out.contains("<ACT MODE V.S. PLAN MODE>"));
        assert!(!out.contains("{{"));
    }

    #[test]
    fn system_prompt_without_settings_includes_every_section() {
        let out = build_system_prompt(&json!({"system": "S"}), "/p");
        assert!(out.contains("<ACT MODE V.S. PLAN MODE>"));
        assert!(out.contains("<project rule>"));
        assert!(out.contains("<visual expression rule>"));
    }

    #[test]
    fn context_block_matches_js_date_string() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 9, 30, 9, 5, 7)
            .unwrap();
        let ctx = build_context(now);
        assert!(ctx.starts_with("<context>\nDate: Wed Sep 30 2026 09:05:07 GMT"));
        assert!(ctx.ends_with("\n<context>\n"));
        assert!(user_message_text("hi").starts_with("hi\n<context>\nDate: "));
    }
}
