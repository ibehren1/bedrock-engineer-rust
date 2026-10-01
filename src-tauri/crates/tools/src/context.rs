//! Execution context handed to every tool call.
//!
//! The TS tools read `ConfigStore` directly and receive an optional `context` argument
//! (`{ sessionId }` from the chat loop). In Rust the app builds a [`ToolContext`] per call
//! from a store snapshot ([`ToolSettings::from_store`], typically `store.all()`) plus the
//! per-call session id, and plugs in the services that live in other crates through the
//! extension traits below.

use crate::agent::SubAgentInvoker;
use crate::command::CommandSandbox;
use crate::filesystem::DocumentReader;
use serde_json::Value;
use std::sync::Arc;

/// `aws.proxyConfig` (`ProxyConfiguration`).
#[derive(Clone, PartialEq, Default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    fn from_value(v: &Value) -> Option<ProxyConfig> {
        let o = v.as_object()?;
        let s = |k: &str| {
            o.get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Some(ProxyConfig {
            enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            host: s("host"),
            port: o
                .get("port")
                .and_then(Value::as_u64)
                .and_then(|p| u16::try_from(p).ok())
                .filter(|p| *p != 0),
            protocol: s("protocol"),
            username: s("username"),
            password: s("password"),
        })
    }

    /// `createProxyAgents`: the proxy URL when enabled with a host, else `None`.
    /// `${protocol || 'http'}://${host}:${port || 8080}` with credentials when both set.
    pub fn proxy_url(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let host = self.host.as_deref()?;
        let mut url = url::Url::parse(&format!(
            "{}://{}:{}",
            self.protocol.as_deref().unwrap_or("http"),
            host,
            self.port.unwrap_or(8080)
        ))
        .ok()?;
        if let (Some(u), Some(p)) = (&self.username, &self.password) {
            let _ = url.set_username(u);
            let _ = url.set_password(Some(p));
        }
        Some(url.to_string())
    }
}

/// `CommandPatternConfig` — one entry of an agent's `allowedCommands`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandPatternConfig {
    pub pattern: String,
    #[serde(default)]
    pub description: String,
}

/// Agent `tavilySearchConfig`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TavilyDomainConfig {
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
}

/// The slice of an agent definition the tools need.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentToolConfig {
    pub id: String,
    pub name: Option<String>,
    pub allowed_commands: Vec<CommandPatternConfig>,
    /// Enabled tool names (`agent.tools`), used to detect `dockerSandbox`.
    pub tools: Vec<String>,
}

impl AgentToolConfig {
    /// Parse the relevant fields of an agent JSON object (custom agent from the store, or
    /// a shared/default agent already loaded as JSON).
    pub fn from_agent_json(v: &Value) -> Option<AgentToolConfig> {
        let o = v.as_object()?;
        let allowed_commands = o
            .get("allowedCommands")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|c| {
                        Some(CommandPatternConfig {
                            pattern: c.get("pattern")?.as_str()?.to_string(),
                            description: c
                                .get("description")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let tools = o
            .get("tools")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Some(AgentToolConfig {
            id: o.get("id")?.as_str()?.to_string(),
            name: o.get("name").and_then(Value::as_str).map(str::to_string),
            allowed_commands,
            tools,
        })
    }
}

/// Extension point for `findAgentById` (store `customAgents` + `sharedAgents`); implemented by
/// `agents::AgentCatalog`.
/// Without a resolver, [`ToolContext::find_agent`] searches the store's `customAgents`.
pub trait AgentResolver: Send + Sync {
    fn find_agent(&self, agent_id: &str) -> Option<AgentToolConfig>;
}

/// Store-derived settings the tools read (`this.store.get(...)` in TS).
#[derive(Clone, Default)]
pub struct ToolSettings {
    /// `projectPath`.
    pub project_path: Option<String>,
    /// `userDataPath` (todo storage root).
    pub user_data_path: Option<String>,
    /// `shell`.
    pub shell: Option<String>,
    /// `selectedAgentId`.
    pub selected_agent_id: Option<String>,
    /// `agentChatConfig.ignoreFiles` (listFiles default ignore patterns).
    pub ignore_files: Vec<String>,
    /// `inferenceParams.maxTokens` (result chunking).
    pub max_tokens: Option<f64>,
    /// `llm.maxTokensLimit` (fetchWebsite truncation).
    pub llm_max_tokens_limit: Option<f64>,
    /// `tavilySearch.apikey`.
    pub tavily_api_key: Option<String>,
    /// `aws.proxyConfig`.
    pub proxy: Option<ProxyConfig>,
    /// `customAgents`, raw.
    pub custom_agents: Vec<Value>,
    /// What the Bedrock services read from the store (`aws`, `inferenceParams`,
    /// `thinkingMode`, `interleaveThinking`, `guardrailSettings`, `bedrockSettings`).
    pub converse: bedrock::ConverseSettings,
    /// `generateImageTool.modelId`.
    pub generate_image_model_id: Option<String>,
    /// `recognizeImageTool.modelId`.
    pub recognize_image_model_id: Option<String>,
    /// `generateVideoTool.s3Uri`.
    pub generate_video_s3_uri: Option<String>,
}

/// Shown in place of secrets in `Debug` output, so settings can be logged safely.
fn redact<T>(v: &Option<T>) -> Option<&'static str> {
    v.as_ref().map(|_| "<redacted>")
}

impl std::fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConfig")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("protocol", &self.protocol)
            .field("username", &self.username)
            .field("password", &redact(&self.password))
            .finish()
    }
}

impl std::fmt::Debug for ToolSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSettings")
            .field("project_path", &self.project_path)
            .field("user_data_path", &self.user_data_path)
            .field("shell", &self.shell)
            .field("selected_agent_id", &self.selected_agent_id)
            .field("ignore_files", &self.ignore_files)
            .field("max_tokens", &self.max_tokens)
            .field("llm_max_tokens_limit", &self.llm_max_tokens_limit)
            .field("tavily_api_key", &redact(&self.tavily_api_key))
            .field("proxy", &self.proxy)
            .field("custom_agents", &self.custom_agents.len())
            .field("converse", &self.converse)
            .field("generate_image_model_id", &self.generate_image_model_id)
            .field("recognize_image_model_id", &self.recognize_image_model_id)
            .field("generate_video_s3_uri", &self.generate_video_s3_uri)
            .finish()
    }
}

/// [`bedrock::ConverseSettings`] from the whole store object. Each key is read on its own, so
/// one malformed value falls back to its default instead of discarding the rest.
pub fn converse_settings_from_store(store: &Value) -> bedrock::ConverseSettings {
    fn parse<T: serde::de::DeserializeOwned>(key: &str, v: Option<&Value>) -> Option<T> {
        let v = v.filter(|v| !v.is_null())?;
        serde_json::from_value(v.clone())
            .map_err(|e| tracing::warn!(key, error = %e, "Ignoring malformed store value"))
            .ok()
    }
    let field = |key: &str| store.get(key);
    // electron-store keeps plain JS numbers; accept `4096.0` for the integer field.
    let mut inference = field("inferenceParams").cloned();
    if let Some(n) = inference.as_mut().and_then(|v| v.get_mut("maxTokens")) {
        if let Some(f) = n.as_f64().filter(|_| !n.is_i64() && !n.is_u64()) {
            *n = Value::from(f as i64);
        }
    }
    bedrock::ConverseSettings {
        aws: parse("aws", field("aws")).unwrap_or_default(),
        inference_params: parse("inferenceParams", inference.as_ref()).unwrap_or_default(),
        thinking_mode: parse("thinkingMode", field("thinkingMode")),
        interleave_thinking: parse("interleaveThinking", field("interleaveThinking"))
            .unwrap_or(false),
        guardrail_settings: parse("guardrailSettings", field("guardrailSettings")),
        bedrock_settings: parse("bedrockSettings", field("bedrockSettings")),
    }
}

impl ToolSettings {
    /// Build from the whole store object (`store.all()`).
    pub fn from_store(store: &Value) -> ToolSettings {
        let str_at = |ptr: &str| {
            store
                .pointer(ptr)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        ToolSettings {
            project_path: str_at("/projectPath"),
            user_data_path: str_at("/userDataPath"),
            shell: str_at("/shell"),
            selected_agent_id: str_at("/selectedAgentId"),
            ignore_files: store
                .pointer("/agentChatConfig/ignoreFiles")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            max_tokens: store
                .pointer("/inferenceParams/maxTokens")
                .and_then(Value::as_f64),
            llm_max_tokens_limit: store.pointer("/llm/maxTokensLimit").and_then(Value::as_f64),
            tavily_api_key: str_at("/tavilySearch/apikey"),
            proxy: store
                .pointer("/aws/proxyConfig")
                .and_then(ProxyConfig::from_value),
            custom_agents: store
                .get("customAgents")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            converse: converse_settings_from_store(store),
            generate_image_model_id: str_at("/generateImageTool/modelId"),
            recognize_image_model_id: str_at("/recognizeImageTool/modelId"),
            generate_video_s3_uri: str_at("/generateVideoTool/s3Uri"),
        }
    }

    /// `inferenceParams?.maxTokens || 4096`.
    pub fn max_tokens_or_default(&self) -> f64 {
        match self.max_tokens {
            Some(n) if n != 0.0 && !n.is_nan() => n,
            _ => 4096.0,
        }
    }

    /// The current custom agent's `tavilySearchConfig` (TS reads only `customAgents`).
    pub fn tavily_domain_config(&self) -> TavilyDomainConfig {
        let agent = self.selected_agent_id.as_deref().and_then(|id| {
            self.custom_agents
                .iter()
                .find(|a| a.get("id").and_then(Value::as_str) == Some(id))
        });
        let list = |cfg: &Value, key: &str| -> Vec<String> {
            cfg.get(key)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        match agent.and_then(|a| a.get("tavilySearchConfig")) {
            Some(cfg) if !cfg.is_null() => TavilyDomainConfig {
                include_domains: list(cfg, "includeDomains"),
                exclude_domains: list(cfg, "excludeDomains"),
            },
            _ => TavilyDomainConfig::default(),
        }
    }
}

/// Run metadata supplied by the trusted caller: the renderer's chat loop (through the tool
/// command's `context` argument) or the background agent engine.
///
/// The TS app smuggled this through the tool input as `_agentId`, `_mcpServers`,
/// `_delegationDepth`, ... keys, which the model could set itself. Tools read it from here
/// instead, and [`crate::ToolRegistry::execute`] strips those keys from the input.
#[derive(Clone, Default, PartialEq)]
pub struct CallerMetadata {
    /// The agent the call runs as: picks the command allowlist and the sandbox policy.
    pub agent_id: Option<String>,
    /// MCP servers of a background agent. Only the agent engine sets this, from the agent's
    /// stored definition; without it the MCP adapter uses the selected agent's servers.
    pub mcp_servers: Option<Vec<common::agent::McpServerConfig>>,
    /// `invokeAgent` delegation depth of the calling agent.
    pub delegation_depth: Option<u32>,
    /// Agent ids from the root caller down to the calling agent.
    pub delegation_lineage: Option<Vec<String>>,
    /// Agents the user @-mentioned this turn, which `invokeAgent` may call.
    pub allowed_agent_ids: Option<Vec<String>>,
    /// The calling agent's model, inherited by sub-agents.
    pub model_id: Option<String>,
}

impl std::fmt::Debug for CallerMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Server configs can hold secrets in `env`/`headers`, so only count them.
        f.debug_struct("CallerMetadata")
            .field("agent_id", &self.agent_id)
            .field("mcp_servers", &self.mcp_servers.as_ref().map(Vec::len))
            .field("delegation_depth", &self.delegation_depth)
            .field("delegation_lineage", &self.delegation_lineage)
            .field("allowed_agent_ids", &self.allowed_agent_ids)
            .field("model_id", &self.model_id)
            .finish()
    }
}

/// Per-call context passed to [`crate::Tool::execute_internal`].
#[derive(Clone, Default)]
pub struct ToolContext {
    pub settings: ToolSettings,
    /// `context.sessionId` from the chat loop (`executeTool(input, { sessionId })`).
    pub session_id: Option<String>,
    /// Trusted run metadata (agent id, MCP servers, delegation state).
    pub caller: CallerMetadata,
    /// `findAgentById` provider (Task 7). Falls back to `settings.custom_agents`.
    pub agents: Option<Arc<dyn AgentResolver>>,
    /// Docker sandbox command routing (Task 10).
    pub sandbox: Option<Arc<dyn CommandSandbox>>,
    /// PDF / DOCX text extraction (the `pdf-*` / `docx-*` IPC handlers).
    pub documents: Option<Arc<dyn DocumentReader>>,
    /// `api.subAgent.invoke` (the `invokeAgent` tool), implemented by the `agents` crate.
    pub sub_agents: Option<Arc<dyn SubAgentInvoker>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("settings", &self.settings)
            .field("session_id", &self.session_id)
            .field("caller", &self.caller)
            .field("agents", &self.agents.is_some())
            .field("sandbox", &self.sandbox.is_some())
            .field("documents", &self.documents.is_some())
            .field("sub_agents", &self.sub_agents.is_some())
            .finish()
    }
}

impl ToolContext {
    pub fn new(settings: ToolSettings) -> Self {
        ToolContext {
            settings,
            ..Default::default()
        }
    }

    /// Context from a store snapshot and optional chat session id.
    pub fn from_store(store: &Value, session_id: Option<String>) -> Self {
        ToolContext {
            settings: ToolSettings::from_store(store),
            session_id,
            ..Default::default()
        }
    }

    /// The agent the call runs as: the caller's agent, else the store's `selectedAgentId`.
    pub fn agent_id(&self) -> Option<String> {
        self.caller
            .agent_id
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| self.settings.selected_agent_id.clone())
    }

    /// `findAgentById(agentId)`.
    pub fn find_agent(&self, agent_id: &str) -> Option<AgentToolConfig> {
        if let Some(r) = &self.agents {
            return r.find_agent(agent_id);
        }
        self.settings
            .custom_agents
            .iter()
            .filter(|a| a.get("id").and_then(Value::as_str) == Some(agent_id))
            .find_map(AgentToolConfig::from_agent_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_from_store() {
        let store = json!({
            "projectPath": "/p",
            "userDataPath": "",
            "shell": "/bin/zsh",
            "selectedAgentId": "a1",
            "agentChatConfig": {"ignoreFiles": ["node_modules"]},
            "inferenceParams": {"maxTokens": 8192},
            "llm": {"maxTokensLimit": 1000},
            "tavilySearch": {"apikey": "k"},
            "aws": {"proxyConfig": {"enabled": true, "host": "proxy", "port": 3128, "username": "u", "password": "p"}},
            "customAgents": [{"id": "a1", "tavilySearchConfig": {"includeDomains": ["x.com"], "excludeDomains": []},
                              "allowedCommands": [{"pattern": "ls *"}], "tools": ["dockerSandbox"]}]
        });
        let s = ToolSettings::from_store(&store);
        assert_eq!(s.project_path.as_deref(), Some("/p"));
        assert_eq!(s.user_data_path, None);
        assert_eq!(s.ignore_files, vec!["node_modules"]);
        assert_eq!(s.max_tokens_or_default(), 8192.0);
        assert_eq!(s.tavily_domain_config().include_domains, vec!["x.com"]);
        assert_eq!(
            s.proxy.as_ref().unwrap().proxy_url().as_deref(),
            Some("http://u:p@proxy:3128/")
        );
        let ctx = ToolContext::new(s);
        let agent = ctx.find_agent("a1").unwrap();
        assert_eq!(agent.allowed_commands[0].pattern, "ls *");
        assert_eq!(agent.tools, vec!["dockerSandbox"]);
        assert!(ctx.find_agent("nope").is_none());
    }

    #[test]
    fn bedrock_settings_from_store() {
        let s = ToolSettings::from_store(&json!({
            "aws": {"region": "us-east-1", "accessKeyId": "A", "secretAccessKey": "S", "useProfile": false},
            "inferenceParams": {"maxTokens": 4096.0, "temperature": 0.5},
            "thinkingMode": "garbage",
            "bedrockSettings": {"enableRegionFailover": true, "availableFailoverRegions": ["us-west-2"]},
            "generateImageTool": {"modelId": "amazon.nova-canvas-v1:0"},
            "recognizeImageTool": {"modelId": ""}
        }));
        assert_eq!(s.converse.aws.region, "us-east-1");
        assert_eq!(s.converse.inference_params.max_tokens, Some(4096));
        assert_eq!(s.converse.inference_params.temperature, Some(0.5));
        assert_eq!(s.converse.thinking_mode, None);
        assert!(s.converse.bedrock_settings.unwrap().enable_region_failover);
        assert_eq!(
            s.generate_image_model_id.as_deref(),
            Some("amazon.nova-canvas-v1:0")
        );
        assert_eq!(s.recognize_image_model_id, None);
    }

    #[test]
    fn defaults() {
        let s = ToolSettings::from_store(&json!({}));
        assert_eq!(s.max_tokens_or_default(), 4096.0);
        assert_eq!(s.tavily_domain_config(), TavilyDomainConfig::default());
        assert!(ProxyConfig {
            enabled: false,
            host: Some("h".into()),
            ..Default::default()
        }
        .proxy_url()
        .is_none());
    }
}
