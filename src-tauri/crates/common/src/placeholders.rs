//! Port of `src/common/utils/placeholderUtils.ts`.

use crate::agent::{
    BedrockAgent, CameraConfig, CommandConfig, FlowConfig, KnowledgeBase, WindowConfig,
};
use serde::Serialize;

/// `PlaceholderValues`
#[derive(Debug, Clone, Default)]
pub struct PlaceholderValues {
    pub project_path: String,
    pub allowed_commands: Vec<CommandConfig>,
    pub allowed_windows: Vec<WindowConfig>,
    pub allowed_cameras: Vec<CameraConfig>,
    pub knowledge_bases: Vec<KnowledgeBase>,
    pub bedrock_agents: Vec<BedrockAgent>,
    pub flows: Vec<FlowConfig>,
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

/// `replacePlaceholders(text, placeholders)` using today's UTC date (`toISOString().slice(0, 10)`).
pub fn replace_placeholders(text: &str, placeholders: &PlaceholderValues) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    replace_placeholders_with_date(text, placeholders, &today)
}

/// [`replace_placeholders`] with an explicit `yyyy-MM-dd` date.
pub fn replace_placeholders_with_date(
    text: &str,
    placeholders: &PlaceholderValues,
    yyyy_mm_dd: &str,
) -> String {
    text.replace("{{projectPath}}", &placeholders.project_path)
        .replace("{{date}}", yyyy_mm_dd)
        .replace("{{allowedCommands}}", &json(&placeholders.allowed_commands))
        .replace("{{allowedWindows}}", &json(&placeholders.allowed_windows))
        .replace("{{allowedCameras}}", &json(&placeholders.allowed_cameras))
        .replace("{{knowledgeBases}}", &json(&placeholders.knowledge_bases))
        .replace("{{bedrockAgents}}", &json(&placeholders.bedrock_agents))
        .replace("{{flows}}", &json(&placeholders.flows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_every_placeholder() {
        let values = PlaceholderValues {
            project_path: "/p".into(),
            allowed_commands: vec![CommandConfig {
                pattern: "ls *".into(),
                description: "list".into(),
            }],
            ..Default::default()
        };
        let out = replace_placeholders_with_date(
            "{{projectPath}} {{projectPath}} {{date}} {{allowedCommands}} {{flows}} {{knowledgeBases}}",
            &values,
            "2026-09-30",
        );
        assert_eq!(
            out,
            r#"/p /p 2026-09-30 [{"pattern":"ls *","description":"list"}] [] []"#
        );
    }

    #[test]
    fn date_is_iso_day() {
        let out = replace_placeholders("{{date}}", &PlaceholderValues::default());
        assert_eq!(out.len(), 10);
        assert_eq!(&out[4..5], "-");
    }
}
