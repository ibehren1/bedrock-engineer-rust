//! Agent types and schemas — port of `src/types/agent-chat.ts`, `src/types/agent-chat.schema.ts`
//! and `src/types/agent.ts`.
//!
//! The serde types serialize to the same camelCase JSON as the TS types. The Zod schemas are
//! reproduced as [`crate::zod::Schema`] values (see the `*_schema()` functions) so runtime
//! validation accepts and rejects exactly what the TS app does.

use crate::zod::{array, object, opt, record, req, strict_object, Schema};
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandConfig {
    pub pattern: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBase {
    pub knowledge_base_id: String,
    pub description: String,
}

/// `src/types/agent.ts` `BedrockAgent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockAgent {
    pub agent_id: String,
    pub alias_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowConfig {
    pub flow_identifier: String,
    pub flow_alias_identifier: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<InputType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    Command,
    Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<ConnectionType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TavilySearchConfig {
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentContextSettings {
    pub project_rule: bool,
    pub visual_expression_rules: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentCategory {
    General,
    Coding,
    Design,
    Data,
    Business,
    Custom,
    All,
    Diagram,
    Website,
}

/// `CustomAgent` (`CustomAgentSchema`, which extends `BaseAgentSchema`).
///
/// Field order matches the Zod schema's shape order, so serializing a parsed agent yields the
/// same key order as Zod's output.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system: String,
    pub scenarios: Vec<Scenario>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_custom: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_shared: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    /// Absolute path of the file a shared agent was loaded from; never written back out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<AgentCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_commands: Option<Vec<CommandConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_windows: Option<Vec<WindowConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_cameras: Option<Vec<CameraConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bedrock_agents: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge_bases: Option<Vec<KnowledgeBase>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flows: Option<Vec<FlowConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_tools: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tavily_search_config: Option<TavilySearchConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_instruction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_context_settings: Option<EnvironmentContextSettings>,
}

/// `AgentChatConfig`
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_files: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_prompt_cache: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_timeout: Option<u64>,
}

/// `SendMsgKey`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SendMsgKey {
    #[serde(rename = "Enter")]
    Enter,
    #[serde(rename = "Cmd+Enter")]
    CmdEnter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Config {
    pub bucket: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    pub region: String,
}

/// `OrganizationConfig`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationConfig {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub s3_config: S3Config,
}

// ---------------------------------------------------------------------------------------------
// Zod schemas
// ---------------------------------------------------------------------------------------------

/// `CuratedAgentIconSchema` values.
pub const CURATED_AGENT_ICONS: &[&str] = &[
    "robot",
    "brain",
    "chat",
    "bulb",
    "books",
    "pencil",
    "messages",
    "puzzle",
    "world",
    "happy",
    "kid",
    "moon",
    "sun",
    "calendar-stats",
    "star",
    "heart",
    "smile",
    "sad",
    "angry",
    "question",
    "info",
    "warning",
    "code",
    "terminal",
    "terminal2",
    "keyboard",
    "bug",
    "test",
    "api",
    "database",
    "architecture",
    "design",
    "diagram",
    "settings",
    "tool",
    "python",
    "java",
    "rust",
    "go",
    "php",
    "swift",
    "react",
    "vue",
    "angular",
    "nodejs",
    "npm",
    "webpack",
    "aws",
    "cloud",
    "server",
    "network",
    "laptop",
    "microchip",
    "docker",
    "kubernetes",
    "terraform",
    "git",
    "github",
    "kanban",
    "azure",
    "google-cloud",
    "digitalocean",
    "lambda",
    "bucket",
    "container",
    "database-cloud",
    "cloud-computing",
    "jenkins",
    "gitlab",
    "circleci",
    "ansible",
    "pipeline",
    "automation",
    "security",
    "lock",
    "shield",
    "bank",
    "search",
    "chart",
    "grafana",
    "prometheus",
    "firewall",
    "key",
    "certificate",
    "fingerprint",
    "scan",
    "dashboard",
    "alert",
    "report",
    "analytics",
    "logs",
    "home",
    "house-door",
    "sofa",
    "laundry",
    "wash-machine",
    "tv",
    "plant",
    "calendar-event",
    "calendar-check",
    "calendar-time",
    "clock",
    "alarm",
    "family",
    "parent",
    "baby",
    "baby-carriage",
    "child",
    "dog",
    "cat",
    "pets",
    "clothes",
    "light-bulb",
    "fan",
    "thermostat",
    "speaker",
    "vacuum",
    "doorbell",
    "lock-smart",
    "garden",
    "heartbeat",
    "activity",
    "stethoscope",
    "pill",
    "vaccine",
    "medical-cross",
    "first-aid",
    "first-aid-box",
    "hospital",
    "hospital-fill",
    "wheelchair",
    "weight",
    "run",
    "running",
    "yoga",
    "fitness",
    "swimming",
    "clipboard-pulse",
    "mental-health",
    "nutrition",
    "sleep",
    "meditation",
    "dental",
    "eye",
    "therapy",
    "school",
    "ballpen",
    "book",
    "bookshelf",
    "journal",
    "math",
    "abacus",
    "calculator",
    "language",
    "palette",
    "music",
    "open-book",
    "teacher",
    "graduate",
    "science",
    "chemistry",
    "physics",
    "biology",
    "online-learning",
    "certificate-education",
    "plane",
    "map",
    "compass",
    "camping",
    "mountain",
    "hiking",
    "car",
    "bicycle",
    "bike",
    "train",
    "bus",
    "walk",
    "camera",
    "movie",
    "gamepad",
    "tv-old",
    "guitar",
    "tennis",
    "hotel",
    "luggage",
    "passport",
    "ticket",
    "beach",
    "mountain-view",
    "cooker",
    "microwave",
    "kitchen",
    "chef",
    "cooking-pot",
    "grill",
    "fast-food",
    "restaurant",
    "menu",
    "salad",
    "meat",
    "bread",
    "coffee",
    "egg",
    "noodles",
    "cupcake",
    "tea",
    "juice",
    "pizza",
    "sushi",
    "ice-cream",
    "wine",
    "credit-card",
    "receipt",
    "coin",
    "cash",
    "currency-yen",
    "wallet",
    "money",
    "shopping-cart",
    "shopping-bag",
    "shopping-bag-solid",
    "shopping-basket",
    "gift",
    "truck",
    "store",
    "shop",
    "web",
    "barcode",
    "qr-code",
    "package",
    "tag",
    "discount",
    "online-payment",
];

/// `AgentCategorySchema` values.
pub const AGENT_CATEGORIES: &[&str] = &[
    "general", "coding", "design", "data", "business", "custom", "all", "diagram", "website",
];

/// `InputTypeSchema` values.
pub const INPUT_TYPES: &[&str] = &["string", "number", "boolean", "object", "array"];

/// Pattern for `LibraryAgentIconSchema` (an Iconify id like `tabler:rocket`).
pub const LIBRARY_AGENT_ICON_PATTERN: &str = r"^[a-z][a-z0-9-]*:[a-z0-9][a-z0-9-]*$";
const LIBRARY_AGENT_ICON_MESSAGE: &str = "Expected an Iconify icon id like \"tabler:rocket\"";

fn build_command_config() -> Schema {
    object(vec![
        req("pattern", Schema::String),
        req("description", Schema::String),
    ])
}

fn build_window_config() -> Schema {
    object(vec![
        req("id", Schema::String),
        req("name", Schema::String),
        req("enabled", Schema::Boolean),
    ])
}

fn build_scenario() -> Schema {
    object(vec![
        req("title", Schema::String),
        req("content", Schema::String),
    ])
}

fn build_agent_icon() -> Schema {
    Schema::Union(vec![
        Schema::Enum(CURATED_AGENT_ICONS.to_vec()),
        Schema::Regex(
            Regex::new(LIBRARY_AGENT_ICON_PATTERN).expect("valid icon regex"),
            LIBRARY_AGENT_ICON_MESSAGE,
        ),
    ])
}

fn build_knowledge_base() -> Schema {
    object(vec![
        req("knowledgeBaseId", Schema::String),
        req("description", Schema::String),
    ])
}

fn build_flow_config() -> Schema {
    object(vec![
        req("flowIdentifier", Schema::String),
        req("flowAliasIdentifier", Schema::String),
        req("description", Schema::String),
        opt("inputType", Schema::Enum(INPUT_TYPES.to_vec())),
        opt("schema", Schema::Any),
    ])
}

fn build_mcp_server_config() -> Schema {
    object(vec![
        req("name", Schema::String),
        req("description", Schema::String),
        opt("connectionType", Schema::Enum(vec!["command", "url"])),
        opt("command", Schema::String),
        opt("args", array(Schema::String)),
        opt("env", record(Schema::String)),
        opt("url", Schema::String),
        opt("headers", record(Schema::String)),
    ])
}

fn build_tavily_search_config() -> Schema {
    object(vec![
        req("includeDomains", array(Schema::String)),
        req("excludeDomains", array(Schema::String)),
    ])
}

fn build_environment_context_settings() -> Schema {
    strict_object(vec![
        req("projectRule", Schema::Boolean),
        req("visualExpressionRules", Schema::Boolean),
    ])
}

fn base_agent_fields() -> Vec<crate::zod::Field> {
    vec![
        req("id", Schema::String),
        req("name", Schema::String),
        req("description", Schema::String),
        req("system", Schema::String),
        req("scenarios", array(build_scenario())),
        opt("icon", build_agent_icon()),
        opt("iconColor", Schema::String),
        opt("tags", array(Schema::String)),
        opt("author", Schema::String),
    ]
}

fn build_custom_agent() -> Schema {
    let mut fields = base_agent_fields();
    fields.extend(vec![
        opt("isCustom", Schema::Boolean),
        opt("isShared", Schema::Boolean),
        opt("directoryOnly", Schema::Boolean),
        opt("organizationId", Schema::String),
        opt("sharedFilePath", Schema::String),
        opt("tools", array(Schema::String)),
        opt("category", Schema::Enum(AGENT_CATEGORIES.to_vec())),
        opt("allowedCommands", array(build_command_config())),
        opt("allowedWindows", array(build_window_config())),
        opt("allowedCameras", array(build_window_config())),
        opt("bedrockAgents", array(Schema::Any)),
        opt("knowledgeBases", array(build_knowledge_base())),
        opt("flows", array(build_flow_config())),
        opt("mcpServers", array(build_mcp_server_config())),
        opt("mcpTools", array(Schema::Any)),
        opt("tavilySearchConfig", build_tavily_search_config()),
        opt("additionalInstruction", Schema::String),
        opt(
            "environmentContextSettings",
            build_environment_context_settings(),
        ),
    ]);
    object(fields)
}

macro_rules! cached_schema {
    ($(#[$m:meta])* $name:ident, $build:ident) => {
        $(#[$m])*
        pub fn $name() -> &'static Schema {
            static CELL: OnceLock<Schema> = OnceLock::new();
            CELL.get_or_init($build)
        }
    };
}

cached_schema!(
    /// `CommandConfigSchema`
    command_config_schema,
    build_command_config
);
cached_schema!(
    /// `WindowConfigSchema`
    window_config_schema,
    build_window_config
);
cached_schema!(
    /// `CameraConfigSchema` (same shape as `WindowConfigSchema`)
    camera_config_schema,
    build_window_config
);
cached_schema!(
    /// `ScenarioSchema`
    scenario_schema,
    build_scenario
);
cached_schema!(
    /// `AgentIconSchema`
    agent_icon_schema,
    build_agent_icon
);
cached_schema!(
    /// `KnowledgeBaseSchema`
    knowledge_base_schema,
    build_knowledge_base
);
cached_schema!(
    /// `FlowConfigSchema`
    flow_config_schema,
    build_flow_config
);
cached_schema!(
    /// `McpServerConfigSchema`
    mcp_server_config_schema,
    build_mcp_server_config
);
cached_schema!(
    /// `TavilySearchConfigSchema`
    tavily_search_config_schema,
    build_tavily_search_config
);
cached_schema!(
    /// `EnvironmentContextSettingsSchema` (strict)
    environment_context_settings_schema,
    build_environment_context_settings
);
cached_schema!(
    /// `CustomAgentSchema`
    custom_agent_schema,
    build_custom_agent
);
