//! Port of `src/common/agents/toolRuleGenerator.ts` (`SystemPromptBuilder`).

use crate::tool_names::READ_ONLY_TOOLS;
use serde::{Deserialize, Serialize};

/// The mode shown in the environment context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    #[serde(rename = "ACT MODE")]
    Act,
    #[serde(rename = "PLAN MODE")]
    Plan,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Act => "ACT MODE",
            Mode::Plan => "PLAN MODE",
        }
    }
}

/// `contextSettings` — when supplied, absent flags count as off (JS `undefined` is falsy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_rule: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_expression_rules: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act_plan_mode_rules: Option<bool>,
}

fn basic_environment_context(current_mode: &str) -> String {
    format!(
        "**<context>**\n\n- working directory: {{{{projectPath}}}}\n- date: {{{{date}}}}\n- current mode: {current_mode}\n\n**</context>**\n"
    )
}

const PROJECT_RULE: &str = "
**<project rule>**

- If there are files under {{projectPath}}/.bedrock-engineer/rules, make sure to load them before working on them.
  This folder contains project-specific rules.

**</project rule>**
";

const VISUAL_EXPRESSION_RULES: &str = "
**<visual expression rule>**

If you are acting as a voice chat, please ignore this illustration rule.

- Create Mermaid.js diagrams for visual explanations (maximum 2 per response unless specified)
- If a complex diagram is required  please express it in draw.io xml format.
- Ask user permission before generating images with Stable Diffusion
- Display images using Markdown syntax: `![image-name](url)`
  - (example) `![img]({{projectPath}}/generated_image.png)`
  - (example) `![img]({{projectPath}}/workspaces/workspace-20250529-session_1748509562336_4xe58p/generated_image.png)`
  - Do not start with file://. Start with /.
- Use KaTeX format for mathematical formulas
- For web applications, source images from Pexels or user-specified sources

**</visual expression rule>**
";

fn act_plan_mode_rules() -> String {
    let read_only_tools_list = READ_ONLY_TOOLS.join(", ");
    format!(
        "
**<ACT MODE V.S. PLAN MODE>**

In each user message, the <context> will specify the current mode. There are two modes:

ACT MODE: In this mode, you have access to all tools except read-only tools.
In ACT MODE, you use tools to accomplish the user's task.

PLAN MODE: In this special mode, you have access to these read-only tools: {read_only_tools_list}.
In PLAN MODE, the goal is to gather information and get context to create a detailed plan for accomplishing the task, which the user will review and approve before they switch you to ACT MODE to implement the solution.
In PLAN mode, even if you are asked to implement something, you should analyze and research it to come up with a plan and report back to us. Never provide a full code snippet as a chat response unless explicitly instructed to do so.

**What is PLAN MODE?**
While you are usually in ACT MODE, the user may switch to PLAN MODE in order to have a back and forth with you to plan how to best accomplish the task.
When starting in PLAN MODE, depending on the user's request, you may need to do some information gathering e.g. using readFiles or tavilySearch to get more context about the task. You may also ask the user clarifying questions to get a better understanding of the task.
Once you've gained more context about the user's request, you should architect a detailed plan for how you will accomplish the task.
Then you might ask the user if they are pleased with this plan, or if they would like to make any changes. Think of this as a brainstorming session where you can discuss the task and plan the best way to accomplish it.
Finally once it seems like you've reached a good plan, ask the user to switch you back to ACT MODE to implement the solution.

**</ACT MODE V.S. PLAN MODE>**

"
    )
}

/// `SystemPromptBuilder.generateEnvironmentContext(contextSettings?, currentMode?)`.
///
/// With no settings every section is included. Placeholders such as `{{projectPath}}` are left
/// for [`crate::placeholders::replace_placeholders`].
pub fn generate_environment_context(
    context_settings: Option<&ContextSettings>,
    current_mode: Option<Mode>,
) -> String {
    let all = ContextSettings {
        project_rule: Some(true),
        visual_expression_rules: Some(true),
        act_plan_mode_rules: Some(true),
    };
    let settings = context_settings.unwrap_or(&all);

    let mut context = basic_environment_context(current_mode.unwrap_or_default().as_str());
    if settings.act_plan_mode_rules.unwrap_or(false) {
        context.push_str(&act_plan_mode_rules());
    }
    if settings.project_rule.unwrap_or(false) {
        context.push_str(PROJECT_RULE);
    }
    if settings.visual_expression_rules.unwrap_or(false) {
        context.push_str(VISUAL_EXPRESSION_RULES);
    }
    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_includes_every_section_in_order() {
        let ctx = generate_environment_context(None, None);
        assert!(ctx.starts_with("**<context>**\n\n- working directory: {{projectPath}}\n- date: {{date}}\n- current mode: ACT MODE\n\n**</context>**\n"));
        let act = ctx.find("<ACT MODE V.S. PLAN MODE>").unwrap();
        let rule = ctx.find("<project rule>").unwrap();
        let visual = ctx.find("<visual expression rule>").unwrap();
        assert!(act < rule && rule < visual);
        assert!(ctx.contains("readFiles, listFiles, tavilySearch, fetchWebsite, recognizeImage, retrieve, invokeBedrockAgent, think."));
    }

    #[test]
    fn supplied_settings_treat_missing_flags_as_off() {
        let ctx = generate_environment_context(
            Some(&ContextSettings {
                project_rule: Some(true),
                ..Default::default()
            }),
            Some(Mode::Plan),
        );
        assert!(ctx.contains("current mode: PLAN MODE"));
        assert!(ctx.contains("<project rule>"));
        assert!(!ctx.contains("<visual expression rule>"));
        assert!(!ctx.contains("<ACT MODE V.S. PLAN MODE>"));
    }
}
