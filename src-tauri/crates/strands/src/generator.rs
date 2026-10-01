//! Port of `src/main/services/strandsAgentsConverter/codeGenerator.ts` (`CodeGenerator`).
//!
//! Agents are the renderer's `CustomAgent` JSON. Fields are read the way the TS read them, so a
//! missing `description` renders as `undefined`, as it did through a template string.

use crate::template::{
    combine_special_setup_code, generate_environment_setup, generate_mcp_client_setup,
    generate_mcp_context_manager, generate_mcp_dependencies, generate_mcp_tools_collection,
    generate_tools_setup_code, generate_yaml_list, js_trim, render_template, CONFIG_TEMPLATE,
    MCP_INTEGRATED_TEMPLATE, README_TEMPLATE, REQUIREMENTS_TEMPLATE,
};
use crate::tool_mapper::{
    generate_import_statement, generate_special_setup_code, strands_tool, StrandsTool,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

/// Default Bedrock model in the generated `agent.py`.
const DEFAULT_MODEL: &str = "us.anthropic.claude-sonnet-4-20250514-v1:0";
/// Region of the generated boto3 session.
const DEFAULT_AWS_REGION: &str = "us-east-1";

/// A supported tool: the agent's tool name and its Strands counterpart.
#[derive(Debug, Clone, PartialEq)]
pub struct SupportedTool {
    pub original_name: String,
    pub strands_tool: StrandsTool,
}

/// A tool that has no Strands counterpart, with the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct UnsupportedTool {
    pub original_name: String,
    pub reason: String,
}

/// `ToolMappingResult` (the unused `imports` set is omitted).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolMapping {
    pub supported_tools: Vec<SupportedTool>,
    pub unsupported_tools: Vec<UnsupportedTool>,
    /// `(toolName, setupCode)`
    pub special_setup: Vec<(String, String)>,
}

/// One entry of `McpServerMappingResult.servers`.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerMapping {
    pub original: Value,
    pub strands_code: String,
    pub client_var_name: String,
}

/// `AgentConfig`
#[derive(Debug, Clone, PartialEq)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub model_provider: String,
    pub tools_used: Vec<String>,
    pub unsupported_tools: Vec<String>,
    /// Ordered `(key, value)` pairs.
    pub environment: Vec<(String, String)>,
    pub mcp_servers: Vec<String>,
}

/// `StrandsAgentOutput`
#[derive(Debug, Clone, PartialEq)]
pub struct StrandsAgentOutput {
    pub python_code: String,
    pub config: AgentConfig,
    pub tool_mapping: ToolMapping,
    pub mcp_servers: Vec<McpServerMapping>,
    pub requirements_text: String,
    pub readme_text: String,
    pub config_yaml_text: String,
}

/// `String(value)` / `${value}` for a JSON field; `None` is `undefined`.
pub(crate) fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::Null => String::new(),
                other => js_string(Some(other)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// JS truthiness of a JSON field.
fn js_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

/// `JSON.stringify(value)` in a template string (`undefined` when absent).
fn json_compact(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(v) => serde_json::to_string(v).unwrap_or_default(),
    }
}

/// `JSON.stringify(value, null, 8)`.
fn json_pretty8(v: &Value) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"        ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    if v.serialize(&mut ser).is_err() {
        return String::new();
    }
    String::from_utf8(buf).unwrap_or_default()
}

/// `name.toLowerCase().replace(/[^a-z0-9]/g, '_')`; the regex works on UTF-16 code units, so a
/// character outside the BMP becomes two underscores.
pub fn sanitize_var_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('_');
            }
        }
    }
    out
}

/// `processSystemPrompt`: escape for a Python `"""` string, then trim.
pub fn process_system_prompt(system_prompt: &str) -> String {
    let escaped = system_prompt
        .replace('\\', "\\\\")
        .replace("\"\"\"", "\\\"\\\"\\\"")
        .replace('\n', "\\n");
    js_trim(&escaped).to_string()
}

/// `analyzeAndMapTools`
pub fn analyze_and_map_tools(tools: &[Value]) -> ToolMapping {
    let mut mapping = ToolMapping::default();
    for tool in tools {
        let name = js_string(Some(tool));
        let built_in = tool
            .as_str()
            .filter(|n| common::tool_names::is_built_in_tool(n))
            .and_then(strands_tool);
        let Some(strands) = built_in else {
            // Skip MCP tools (currently not supported)
            mapping.unsupported_tools.push(UnsupportedTool {
                original_name: name,
                reason: "MCP tools are currently not supported".into(),
            });
            continue;
        };
        if strands.supported {
            mapping.supported_tools.push(SupportedTool {
                original_name: name.clone(),
                strands_tool: *strands,
            });
            let setup = generate_special_setup_code(&name, strands);
            if !setup.is_empty() {
                mapping.special_setup.push((name, setup));
            }
        } else {
            mapping.unsupported_tools.push(UnsupportedTool {
                original_name: name,
                reason: strands.reason.unwrap_or("Not supported").to_string(),
            });
        }
    }
    mapping
}

/// `analyzeMcpServers`. Errors where the TS threw (a server without a string `name`).
pub fn analyze_mcp_servers(servers: &[Value]) -> Result<Vec<McpServerMapping>, String> {
    servers
        .iter()
        .map(|server| {
            let name = match server.get("name") {
                Some(Value::String(s)) => s,
                None | Some(Value::Null) => {
                    return Err(format!(
                        "Cannot read properties of {} (reading 'toLowerCase')",
                        if server.get("name").is_some() {
                            "null"
                        } else {
                            "undefined"
                        }
                    ))
                }
                Some(_) => return Err("name.toLowerCase is not a function".into()),
            };
            let client_var_name = sanitize_var_name(name);
            let strands_code = generate_mcp_client_code(server, &client_var_name);
            Ok(McpServerMapping {
                original: server.clone(),
                strands_code,
                client_var_name,
            })
        })
        .collect()
}

/// `generateMcpClientCode`
fn generate_mcp_client_code(server: &Value, client_var_name: &str) -> String {
    let env_setup = match server.get("env") {
        env @ Some(v) if js_truthy(env) => format!(
            ",\n        env={}",
            json_pretty8(v).replace('\n', "\n        ")
        ),
        _ => String::new(),
    };
    let comment = if js_truthy(server.get("description")) {
        js_string(server.get("description"))
    } else {
        js_string(server.get("name"))
    };
    format!(
        "# {comment}\n{client_var_name}_client = MCPClient(lambda: stdio_client(\n    StdioServerParameters(\n        command=\"{command}\",\n        args={args}{env_setup}\n    )\n))",
        command = js_string(server.get("command")),
        args = json_compact(server.get("args")),
    )
}

fn first_seen<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// `generateMcpIntegratedCode`
fn generate_mcp_integrated_code(
    agent: &Value,
    mapping: &ToolMapping,
    servers: &[McpServerMapping],
    processed_prompt: &str,
    now: DateTime<Utc>,
) -> String {
    let supported: Vec<StrandsTool> = mapping
        .supported_tools
        .iter()
        .map(|t| t.strands_tool)
        .collect();
    let mut imports: Vec<String> = vec![
        "from strands import Agent".into(),
        "import boto3".into(),
        "from strands.models import BedrockModel".into(),
    ];
    imports.extend(generate_import_statement(&supported));
    if !servers.is_empty() {
        imports.push("from mcp import stdio_client, StdioServerParameters".into());
        imports.push("from strands.tools.mcp import MCPClient".into());
    }

    let basic_tools = first_seen(supported.iter().map(|t| t.strands_name));
    let special: Vec<String> = mapping
        .special_setup
        .iter()
        .map(|(_, c)| c.clone())
        .collect();
    let codes: Vec<&str> = servers.iter().map(|s| s.strands_code.as_str()).collect();
    let client_names: Vec<String> = servers
        .iter()
        .map(|s| format!("{}_client", s.client_var_name))
        .collect();
    let collection: Vec<(String, String)> = servers
        .iter()
        .map(|s| (js_string(s.original.get("name")), s.client_var_name.clone()))
        .collect();

    render_template(
        MCP_INTEGRATED_TEMPLATE,
        &[
            ("agentName", js_string(agent.get("name"))),
            ("agentDescription", js_string(agent.get("description"))),
            ("imports", imports.join("\n")),
            ("basicToolsSetup", generate_tools_setup_code(&basic_tools)),
            ("specialSetupCode", combine_special_setup_code(&special)),
            ("mcpClientSetup", generate_mcp_client_setup(&codes)),
            (
                "mcpContextManager",
                generate_mcp_context_manager(&client_names),
            ),
            (
                "mcpToolsCollection",
                generate_mcp_tools_collection(&collection),
            ),
            ("systemPrompt", processed_prompt.to_string()),
            ("modelConfig", DEFAULT_MODEL.to_string()),
            ("awsRegion", DEFAULT_AWS_REGION.to_string()),
            ("generationDate", iso_string(now)),
        ],
    )
}

/// `new Date().toISOString()`
fn iso_string(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// `generateConfig`
fn generate_config(
    agent: &Value,
    mapping: &ToolMapping,
    servers: &[McpServerMapping],
) -> AgentConfig {
    let mut environment = vec![("AWS_REGION".to_string(), "us-west-2".to_string())];
    let has_aws_tools = mapping.supported_tools.iter().any(|t| {
        ["use_aws", "retrieve", "generate_image_stability"].contains(&t.strands_tool.strands_name)
    });
    if has_aws_tools {
        environment.push(("AWS_PROFILE".into(), "default".into()));
    }
    AgentConfig {
        name: js_string(agent.get("name")),
        description: js_string(agent.get("description")),
        model_provider: "bedrock".into(),
        tools_used: mapping
            .supported_tools
            .iter()
            .map(|t| t.original_name.clone())
            .collect(),
        unsupported_tools: mapping
            .unsupported_tools
            .iter()
            .map(|t| t.original_name.clone())
            .collect(),
        environment,
        mcp_servers: servers
            .iter()
            .map(|s| js_string(s.original.get("name")))
            .collect(),
    }
}

/// `generateRequirements`
fn generate_requirements(mapping: &ToolMapping, servers: &[McpServerMapping]) -> String {
    let names: Vec<&str> = mapping
        .supported_tools
        .iter()
        .map(|t| t.strands_tool.strands_name)
        .collect();
    let mut deps = Vec::new();
    if names.contains(&"use_aws") {
        deps.push("# AWS CLI operations");
    }
    if names.contains(&"generate_image_stability") {
        deps.push("# Stability AI image generation");
    }
    if names.contains(&"code_interpreter") {
        deps.push("# Code interpreter functionality");
    }
    render_template(
        REQUIREMENTS_TEMPLATE,
        &[
            (
                "mcpDependencies",
                generate_mcp_dependencies(!servers.is_empty()),
            ),
            ("additionalDependencies", deps.join("\n")),
        ],
    )
}

/// `generateReadme`
fn generate_readme(
    agent: &Value,
    mapping: &ToolMapping,
    servers: &[McpServerMapping],
    config: &AgentConfig,
    now: DateTime<Utc>,
) -> String {
    let supported_list = mapping
        .supported_tools
        .iter()
        .map(|t| {
            format!(
                "- **{}** → {}",
                t.original_name, t.strands_tool.strands_name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let unsupported_list = mapping
        .unsupported_tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.original_name, t.reason))
        .collect::<Vec<_>>()
        .join("\n");

    let mut mcp_info = String::new();
    if !servers.is_empty() {
        mcp_info = String::from("\n\n## MCP Servers\n\n")
            + &servers
                .iter()
                .map(|s| {
                    let o = &s.original;
                    let description = if js_truthy(o.get("description")) {
                        js_string(o.get("description"))
                    } else {
                        "No description".into()
                    };
                    // `args?.join(' ') || ''`
                    let args = match o.get("args") {
                        Some(Value::Array(a)) => a
                            .iter()
                            .map(|x| match x {
                                Value::Null => String::new(),
                                other => js_string(Some(other)),
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        _ => String::new(),
                    };
                    format!(
                        "- **{}**: {}\n  - Command: `{} {}`",
                        js_string(o.get("name")),
                        description,
                        js_string(o.get("command")),
                        args
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
    }

    let total = mapping.supported_tools.len() + mapping.unsupported_tools.len();
    render_template(
        README_TEMPLATE,
        &[
            ("agentName", js_string(agent.get("name"))),
            (
                "agentDescription",
                js_string(agent.get("description")) + &mcp_info,
            ),
            (
                "toolsList",
                if supported_list.is_empty() {
                    "(None)".into()
                } else {
                    supported_list
                },
            ),
            (
                "unsupportedToolsList",
                if unsupported_list.is_empty() {
                    "(None)".into()
                } else {
                    unsupported_list
                },
            ),
            (
                "environmentSetup",
                generate_environment_setup(&config.environment),
            ),
            ("conversionDate", iso_string(now)),
            (
                "supportedToolsCount",
                mapping.supported_tools.len().to_string(),
            ),
            ("totalToolsCount", total.to_string()),
        ],
    )
}

/// `generateYamlConfig`
fn generate_yaml_config(
    config: &AgentConfig,
    mapping: &ToolMapping,
    servers: &[McpServerMapping],
) -> String {
    let supported = generate_yaml_list(&config.tools_used);
    let unsupported = generate_yaml_list(
        &mapping
            .unsupported_tools
            .iter()
            .map(|t| format!("{}: {}", t.original_name, t.reason))
            .collect::<Vec<_>>(),
    );
    let env = generate_yaml_list(
        &config
            .environment
            .iter()
            .map(|(k, v)| format!("{k}: \"{v}\""))
            .collect::<Vec<_>>(),
    );

    let mut mcp_yaml = String::new();
    if !servers.is_empty() {
        mcp_yaml = String::from("\n\nmcp_servers:\n")
            + &servers
                .iter()
                .map(|s| {
                    let o = &s.original;
                    let headers = if js_truthy(o.get("headers")) {
                        format!("\n    headers: {}", json_compact(o.get("headers")))
                    } else {
                        String::new()
                    };
                    format!(
                        "  - name: \"{}\"\n    command: \"{}\"\n    args: {}{}",
                        js_string(o.get("name")),
                        js_string(o.get("command")),
                        json_compact(o.get("args")),
                        headers
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
    }

    render_template(
        CONFIG_TEMPLATE,
        &[
            ("agentName", config.name.clone()),
            ("agentDescription", config.description.clone() + &mcp_yaml),
            ("modelProvider", config.model_provider.clone()),
            ("supportedTools", supported),
            ("unsupportedTools", unsupported),
            ("environmentVars", env),
        ],
    )
}

fn array_field(agent: &Value, key: &str) -> Vec<Value> {
    agent
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// `generateStrandsAgent`: the complete conversion. `now` stamps the generation date.
pub fn generate_strands_agent(
    agent: &Value,
    now: DateTime<Utc>,
) -> Result<StrandsAgentOutput, String> {
    let tool_mapping = analyze_and_map_tools(&array_field(agent, "tools"));
    let mcp_servers = analyze_mcp_servers(&array_field(agent, "mcpServers"))?;
    let system = agent.get("system").and_then(Value::as_str).unwrap_or("");
    let processed_prompt = process_system_prompt(system);

    let python_code =
        generate_mcp_integrated_code(agent, &tool_mapping, &mcp_servers, &processed_prompt, now);
    let config = generate_config(agent, &tool_mapping, &mcp_servers);
    let requirements_text = generate_requirements(&tool_mapping, &mcp_servers);
    let readme_text = generate_readme(agent, &tool_mapping, &mcp_servers, &config, now);
    let config_yaml_text = generate_yaml_config(&config, &tool_mapping, &mcp_servers);

    Ok(StrandsAgentOutput {
        python_code,
        config,
        tool_mapping,
        mcp_servers,
        requirements_text,
        readme_text,
        config_yaml_text,
    })
}
