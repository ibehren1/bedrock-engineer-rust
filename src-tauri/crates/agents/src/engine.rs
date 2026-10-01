//! The Rust-side agent loop — `chat` / `executeToolsRecursively` / `executeTool` of
//! `src/main/api/bedrock/services/backgroundAgent/BackgroundAgentService.ts`.
//!
//! converse → (tool use) → execute tools → tool results → converse → … until the model stops
//! asking for tools or the round budget is spent. Sub-agent delegation
//! ([`crate::SubAgentRunner`]) drives it today; background agents (Task 11) reuse it with a
//! persistent [`SessionStore`] and a [`SessionListener`].
//!
//! Differences from the TS:
//!
//! * Tools run in-process through [`ToolRegistry`] instead of being sent to the renderer's preload
//!   over `preload-tool-request` (no 300 s IPC timeout, no "no active window" failure). Results are
//!   mapped the way the preload response was: a string result is a success, an object result is a
//!   `ToolResult` whose `result` becomes the output, and a thrown error becomes
//!   `{ success: false, error: error.message }`.
//! * Static tool specs come from the registry rather than the preload `ToolMetadataCollector`.
//! * The agent is looked up once per run (the TS looked it up again before the tool loop).

use crate::catalog::AgentCatalog;
use crate::converse::ConverseBackend;
use crate::error::{Error, Result};
use crate::prompt::{build_system_prompt, user_message_text, working_directory};
use crate::session::{
    AgentMessage, InMemorySessionStore, SessionListener, SessionMeta, SessionStore,
};
use crate::store::{prompt_cache_enabled, StoreReader};
use crate::tool_specs::{generate_tool_specs, tool_config, McpToolSpecProvider};
use async_trait::async_trait;
use bedrock::{ConverseRequest, InferenceConfig};
use common::agent::McpServerConfig;
use common::delegation::DelegationContext;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;
use tools::{
    AgentResolver, CallerMetadata, SubAgentInvoker, ToolContext, ToolOutput, ToolRegistry,
};

/// `BackgroundAgentConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunConfig {
    pub model_id: String,
    /// Kept for the stored task shape; the run always uses the agent's own system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
    /// Depth this agent runs at (0 = top-level chat). Unset for plain background runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_depth: Option<u32>,
    /// Agent ids from the root down to and including this agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_lineage: Option<Vec<String>>,
    /// Agent ids the user permitted via @mention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_delegation_agent_ids: Option<Vec<String>>,
}

/// `BackgroundAgentOptions` with the TS defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunOptions {
    pub enable_tool_execution: bool,
    /// Tool rounds (converse calls after the first), not individual tool calls.
    pub max_tool_executions: usize,
    /// Applies to the first Converse call only, as in TS.
    pub timeout: Duration,
}

/// Default `maxToolExecutions`.
pub const DEFAULT_MAX_TOOL_EXECUTIONS: usize = 500;
/// Default `timeoutMs` (three hours).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(10_800_000);

impl Default for AgentRunOptions {
    fn default() -> Self {
        AgentRunOptions {
            enable_tool_execution: true,
            max_tool_executions: DEFAULT_MAX_TOOL_EXECUTIONS,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// One entry of `BackgroundChatResult.toolExecutions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub tool_name: String,
    pub input: Value,
    pub output: Value,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `BackgroundChatResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResult {
    pub response: AgentMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_executions: Option<Vec<ToolExecution>>,
}

/// Builds the [`ToolContext`] for one tool call from a store snapshot. The engine fills in
/// `agents` (its [`AgentCatalog`]) and `sub_agents` when the factory leaves them unset.
pub type ToolContextFactory = Arc<dyn Fn(&Value) -> ToolContext + Send + Sync>;

/// The agent loop and its dependencies.
pub struct AgentEngine {
    converse: Arc<dyn ConverseBackend>,
    registry: Arc<ToolRegistry>,
    store: Arc<dyn StoreReader>,
    catalog: Arc<AgentCatalog>,
    sessions: Arc<dyn SessionStore>,
    mcp: Option<Arc<dyn McpToolSpecProvider>>,
    tool_context: ToolContextFactory,
    listener: Option<Arc<dyn SessionListener>>,
    sub_agents: OnceLock<Weak<dyn SubAgentInvoker>>,
    /// `cachePointMap`: session id → index of the last message that received a cache point.
    cache_points: Mutex<HashMap<String, usize>>,
}

/// JS `block.toolUse` truthiness check used by `'toolUse' in block`.
fn tool_use_of(block: &Value) -> Option<&Map<String, Value>> {
    block.get("toolUse").and_then(Value::as_object)
}

/// `deduplicateToolUseIds`: make `toolUseId`s unique within one assistant message (`id_1`, …).
/// Some models number tool uses per call, and Bedrock rejects duplicate ids on the next request.
pub fn deduplicate_tool_use_ids(mut content: Vec<Value>) -> Vec<Value> {
    let mut seen: HashSet<String> = HashSet::new();
    for block in content.iter_mut() {
        let Some(tool_use) = block.get_mut("toolUse").and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(id) = tool_use
            .get("toolUseId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let mut id_final = id.clone();
        if seen.contains(&id) {
            let mut suffix = 1;
            while seen.contains(&format!("{id}_{suffix}")) {
                suffix += 1;
            }
            id_final = format!("{id}_{suffix}");
            tool_use.insert("toolUseId".into(), Value::String(id_final.clone()));
        }
        seen.insert(id_final);
    }
    content
}

fn response_content(response: &Value) -> Vec<Value> {
    response
        .pointer("/output/message/content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn stop_reason(response: &Value) -> Option<&str> {
    response.get("stopReason").and_then(Value::as_str)
}

/// Map a registry result to a tool execution record, as the `preload-tool-request` handler and
/// `executeTool` did.
fn record_execution(
    tool_name: &str,
    input: Value,
    result: std::result::Result<ToolOutput, tools::ToolError>,
) -> ToolExecution {
    let failed = |error: String| ToolExecution {
        tool_name: tool_name.to_string(),
        input: input.clone(),
        output: Value::Null,
        success: false,
        error: Some(error),
    };
    match result {
        Ok(ToolOutput::Text(s)) => ToolExecution {
            tool_name: tool_name.to_string(),
            input: input.clone(),
            output: Value::String(s),
            success: true,
            error: None,
        },
        Ok(ToolOutput::Json(v)) => {
            if tools::util::js::truthy(v.get("success")) {
                ToolExecution {
                    tool_name: tool_name.to_string(),
                    input: input.clone(),
                    output: v.get("result").cloned().unwrap_or(Value::Null),
                    success: true,
                    error: None,
                }
            } else {
                let error = match v.get("error") {
                    Some(Value::String(s)) if !s.is_empty() => s.clone(),
                    Some(e) if tools::util::js::truthy(Some(e)) => e.to_string(),
                    _ => "Tool execution failed".to_string(),
                };
                failed(error)
            }
        }
        Err(e) => failed(if e.message.is_empty() {
            "Tool execution failed".to_string()
        } else {
            e.message
        }),
    }
}

/// The text of the `toolResult` block for one execution.
fn tool_result_text(execution: &ToolExecution) -> String {
    if execution.success {
        if execution.output.is_null() {
            json!({
                "success": true,
                "message": "Tool executed successfully but no output returned",
                "toolName": execution.tool_name
            })
            .to_string()
        } else {
            execution.output.to_string()
        }
    } else {
        execution
            .error
            .clone()
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "Tool execution failed".to_string())
    }
}

impl AgentEngine {
    /// An engine with an [`InMemorySessionStore`], no MCP specs, no listener, and tool contexts
    /// built by `ToolContext::from_store(snapshot, None)`.
    pub fn new(
        converse: Arc<dyn ConverseBackend>,
        registry: Arc<ToolRegistry>,
        store: Arc<dyn StoreReader>,
    ) -> Self {
        AgentEngine {
            converse,
            registry,
            catalog: Arc::new(AgentCatalog::new(store.clone())),
            store,
            sessions: Arc::new(InMemorySessionStore::new()),
            mcp: None,
            tool_context: Arc::new(|store: &Value| ToolContext::from_store(store, None)),
            listener: None,
            sub_agents: OnceLock::new(),
            cache_points: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_sessions(mut self, sessions: Arc<dyn SessionStore>) -> Self {
        self.sessions = sessions;
        self
    }

    pub fn with_mcp_specs(mut self, mcp: Arc<dyn McpToolSpecProvider>) -> Self {
        self.mcp = Some(mcp);
        self
    }

    /// Plug in sandbox / document readers etc. for tool calls.
    pub fn with_tool_context(mut self, factory: ToolContextFactory) -> Self {
        self.tool_context = factory;
        self
    }

    pub fn with_listener(mut self, listener: Arc<dyn SessionListener>) -> Self {
        self.listener = Some(listener);
        self
    }

    /// Route `invokeAgent` calls made by agents of this engine (nested delegation). Held weakly
    /// because the runner owns the engine. Only the first call has an effect.
    pub fn set_sub_agent_invoker(&self, invoker: Weak<dyn SubAgentInvoker>) {
        let _ = self.sub_agents.set(invoker);
    }

    pub fn catalog(&self) -> &Arc<AgentCatalog> {
        &self.catalog
    }

    pub fn sessions(&self) -> &Arc<dyn SessionStore> {
        &self.sessions
    }

    /// `getAllAgents()`.
    pub fn get_all_agents(&self) -> Vec<Value> {
        self.catalog.all_agents()
    }

    /// `createSession(sessionId, options)`.
    pub fn create_session(&self, session_id: &str, meta: SessionMeta) -> Result<()> {
        self.sessions
            .create_session(session_id, meta)
            .map_err(Error::Session)
    }

    /// `deleteSession(sessionId)` (also forgets the session's cache point).
    pub fn delete_session(&self, session_id: &str) -> bool {
        self.cache_points.lock().unwrap().remove(session_id);
        self.sessions.delete_session(session_id)
    }

    /// `getTaskSystemPrompt` building block: the agent's system prompt with environment context
    /// and placeholders, for `projectDirectory` (or the store's project path).
    pub fn system_prompt_for(
        &self,
        agent_id: &str,
        project_directory: Option<&str>,
    ) -> Result<String> {
        let agent = self
            .catalog
            .find_agent_by_id(agent_id)
            .ok_or_else(|| Error::AgentNotFound(agent_id.to_string()))?;
        let store = self.store.snapshot();
        Ok(build_system_prompt(
            &agent,
            &working_directory(project_directory, &store),
        ))
    }

    fn publish(&self, session_id: &str, message: &AgentMessage) {
        if let Some(l) = &self.listener {
            l.message_published(session_id, message);
        }
    }

    fn history_updated(&self, session_id: &str) {
        if let Some(l) = &self.listener {
            l.history_updated(session_id, self.sessions.history(session_id).len());
        }
    }

    fn update_cache_point(&self, session_id: &str, processed: &[Value]) {
        let Some(last) = processed.last() else { return };
        let has_cache_point = last
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| {
                blocks.iter().any(|b| {
                    tools::util::js::truthy(b.get("cachePoint").and_then(|c| c.get("type")))
                })
            });
        if has_cache_point {
            self.cache_points
                .lock()
                .unwrap()
                .insert(session_id.to_string(), processed.len() - 1);
        }
    }

    async fn call_converse(&self, request: ConverseRequest) -> Result<Value> {
        self.converse
            .converse(request)
            .await
            .map_err(Error::Converse)
    }

    /// `chat(sessionId, config, userMessage, options)`.
    pub async fn chat(
        &self,
        session_id: &str,
        config: &AgentRunConfig,
        user_message: &str,
        options: &AgentRunOptions,
    ) -> Result<ChatResult> {
        let agent = self
            .catalog
            .find_agent_by_id(&config.agent_id)
            .ok_or_else(|| Error::AgentNotFound(config.agent_id.clone()))?;
        let store = self.store.snapshot();
        let workdir = working_directory(config.project_directory.as_deref(), &store);

        let delegation = DelegationContext {
            depth: config.delegation_depth.unwrap_or(0),
            lineage: config.delegation_lineage.clone().unwrap_or_default(),
            allowed_agent_ids: config
                .allowed_delegation_agent_ids
                .clone()
                .unwrap_or_default(),
        };
        let specs = generate_tool_specs(
            &agent,
            &self.registry.tool_specs(),
            self.mcp.as_deref(),
            &delegation,
            &workdir,
            || self.catalog.all_agents(),
        )
        .await;

        let history = self.sessions.history(session_id);
        if !self.sessions.has_session(session_id) {
            self.create_session(
                session_id,
                SessionMeta {
                    task_id: None,
                    agent_id: Some(config.agent_id.clone()),
                    model_id: Some(config.model_id.clone()),
                    project_directory: config.project_directory.clone(),
                },
            )?;
        }

        tracing::info!(
            model_id = %config.model_id,
            agent_id = %config.agent_id,
            tool_count = specs.len(),
            history_length = history.len(),
            "Starting background agent chat"
        );

        let mut messages = history;
        let user = AgentMessage::new(
            "user",
            vec![json!({ "text": user_message_text(user_message) })],
        );
        self.sessions
            .add_message(session_id, &user)
            .map_err(|e| Error::Session(format!("Failed to save user message: {e}")))?;
        self.publish(session_id, &user);
        self.history_updated(session_id);
        messages.push(user);

        let system_prompt = build_system_prompt(&agent, &workdir);
        let system: Vec<Value> = if system_prompt.is_empty() {
            Vec::new()
        } else {
            vec![json!({ "text": system_prompt })]
        };
        let tools_cfg = tool_config(&specs);

        let converse_messages: Vec<Value> =
            messages.iter().map(AgentMessage::to_converse).collect();
        let (processed_messages, processed_system, processed_tools) =
            if prompt_cache_enabled(&store) {
                let cache = models::PromptCacheManager::new(&config.model_id);
                let first = self.cache_points.lock().unwrap().get(session_id).copied();
                let m = cache.add_cache_points_to_messages(&converse_messages, first);
                let s = if system.is_empty() {
                    system.clone()
                } else {
                    cache.add_cache_point_to_system(&system)
                };
                let t = match &tools_cfg {
                    Some(t) => cache.add_cache_point_to_tools(Some(t)),
                    None => None,
                };
                (m, s, t)
            } else {
                (converse_messages, system.clone(), tools_cfg.clone())
            };

        let request = ConverseRequest {
            model_id: config.model_id.clone(),
            messages: processed_messages.clone(),
            system: (!processed_system.is_empty()).then_some(processed_system),
            tool_config: processed_tools,
            inference_config: config.inference_config.clone(),
            ..Default::default()
        };
        let response = tokio::time::timeout(options.timeout, self.call_converse(request))
            .await
            .map_err(|_| Error::ChatTimeout)??;

        if prompt_cache_enabled(&store) {
            self.update_cache_point(session_id, &processed_messages);
        }

        let mut response_message = AgentMessage::new(
            "assistant",
            deduplicate_tool_use_ids(response_content(&response)),
        );
        if let Some(usage) = response.get("usage").filter(|u| !u.is_null()) {
            response_message.metadata = Some(json!({
                "converseMetadata": { "usage": usage, "stopReason": response.get("stopReason") }
            }));
        }

        let result = if stop_reason(&response) == Some("tool_use") && options.enable_tool_execution
        {
            tracing::info!("Tool execution required, processing tools");
            self.sessions
                .add_message(session_id, &response_message)
                .map_err(Error::Session)?;
            messages.push(response_message);
            self.execute_tools_recursively(
                session_id,
                config,
                &agent,
                messages,
                options.max_tool_executions,
                tools_cfg,
                system,
            )
            .await?
        } else {
            self.sessions
                .add_message(session_id, &response_message)
                .map_err(|e| Error::Session(format!("Failed to save assistant response: {e}")))?;
            self.publish(session_id, &response_message);
            self.history_updated(session_id);
            ChatResult {
                response: response_message,
                tool_executions: None,
            }
        };

        tracing::info!(
            session_id,
            tool_execution_count = result.tool_executions.as_ref().map_or(0, Vec::len),
            "Background agent chat completed successfully"
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_tools_recursively(
        &self,
        session_id: &str,
        config: &AgentRunConfig,
        agent: &Value,
        mut messages: Vec<AgentMessage>,
        max_executions: usize,
        tools_cfg: Option<Value>,
        system: Vec<Value>,
    ) -> Result<ChatResult> {
        let mut executions: Vec<ToolExecution> = Vec::new();
        let mut count = 0;

        while count < max_executions {
            let Some(last) = messages.last().filter(|m| m.role == "assistant") else {
                break;
            };
            let tool_uses: Vec<Map<String, Value>> = last
                .content
                .iter()
                .filter_map(tool_use_of)
                .cloned()
                .collect();
            if tool_uses.is_empty() {
                break;
            }
            tracing::debug!(
                "Executing {} tools (execution {}/{max_executions})",
                tool_uses.len(),
                count + 1
            );

            let mut results = Vec::new();
            for tool_use in &tool_uses {
                let execution = self.execute_tool(tool_use, agent, config, session_id).await;
                results.push(json!({
                    "toolResult": {
                        "toolUseId": tool_use.get("toolUseId").cloned().unwrap_or(Value::Null),
                        "content": [{ "text": tool_result_text(&execution) }],
                        "status": if execution.success { "success" } else { "error" }
                    }
                }));
                executions.push(execution);
            }

            let result_message = AgentMessage::new("user", results);
            if let Err(error) = self.sessions.add_message(session_id, &result_message) {
                tracing::error!(session_id, %error, "Failed to save tool result message to session");
            } else {
                self.publish(session_id, &result_message);
            }
            messages.push(result_message);

            let request = ConverseRequest {
                model_id: config.model_id.clone(),
                messages: messages.iter().map(AgentMessage::to_converse).collect(),
                system: (!system.is_empty()).then(|| system.clone()),
                tool_config: tools_cfg.clone(),
                inference_config: config.inference_config.clone(),
                ..Default::default()
            };
            let next = self.call_converse(request).await?;
            let next_message = AgentMessage::new(
                "assistant",
                deduplicate_tool_use_ids(response_content(&next)),
            );
            self.sessions
                .add_message(session_id, &next_message)
                .map_err(Error::Session)?;
            self.publish(session_id, &next_message);
            messages.push(next_message.clone());
            count += 1;

            if stop_reason(&next) != Some("tool_use") {
                return Ok(ChatResult {
                    response: next_message,
                    tool_executions: Some(executions),
                });
            }
        }

        tracing::warn!(max_executions, "Maximum tool executions reached");
        let response = messages
            .last()
            .cloned()
            .unwrap_or_else(|| AgentMessage::new("assistant", Vec::new()));
        Ok(ChatResult {
            response,
            tool_executions: Some(executions),
        })
    }

    /// `executeTool(toolUse, agent, config, sessionId)`.
    async fn execute_tool(
        &self,
        tool_use: &Map<String, Value>,
        agent: &Value,
        config: &AgentRunConfig,
        session_id: &str,
    ) -> ToolExecution {
        let name = tool_use
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let model_input = tool_use.get("input").cloned().unwrap_or(Value::Null);

        // The run metadata travels in the context, not the input, so the model cannot forge
        // it; the registry strips any `_`-prefixed keys the model wrote into the input.
        let mut input = model_input.as_object().cloned().unwrap_or_default();
        input.insert("type".into(), json!(name));

        let store = self.store.snapshot();
        let mut ctx = (self.tool_context)(&store);
        ctx.session_id = Some(session_id.to_string());
        ctx.caller = CallerMetadata {
            agent_id: Some(config.agent_id.clone()),
            // From the agent's stored definition, never from the model.
            mcp_servers: agent.get("mcpServers").filter(|v| !v.is_null()).map(|v| {
                serde_json::from_value::<Vec<McpServerConfig>>(v.clone()).unwrap_or_else(|error| {
                    tracing::warn!(%error, "Ignoring malformed mcpServers on agent");
                    Vec::new()
                })
            }),
            delegation_depth: Some(config.delegation_depth.unwrap_or(0)),
            delegation_lineage: Some(
                config
                    .delegation_lineage
                    .clone()
                    .unwrap_or_else(|| vec![config.agent_id.clone()]),
            ),
            allowed_agent_ids: Some(
                config
                    .allowed_delegation_agent_ids
                    .clone()
                    .unwrap_or_default(),
            ),
            model_id: Some(config.model_id.clone()),
        };
        if ctx.agents.is_none() {
            ctx.agents = Some(self.catalog.clone() as Arc<dyn AgentResolver>);
        }
        if ctx.sub_agents.is_none() {
            ctx.sub_agents = self.sub_agents.get().and_then(Weak::upgrade);
        }

        tracing::debug!(tool_name = %name, "Executing tool via tool registry");
        let result = self.registry.execute(Value::Object(input), &ctx).await;
        let execution = record_execution(&name, model_input, result);
        if execution.success {
            tracing::debug!(tool_name = %name, "Tool execution completed");
        } else {
            tracing::warn!(tool_name = %name, error = ?execution.error, "Tool execution failed");
        }
        execution
    }
}

/// What [`crate::SubAgentRunner`] needs from the service that runs agents (the TS runner took a
/// `() => BackgroundAgentService`). [`AgentEngine`] implements it; tests script it.
#[async_trait]
pub trait AgentService: Send + Sync {
    async fn get_all_agents(&self) -> Vec<Value>;
    async fn create_session(&self, session_id: &str, meta: SessionMeta) -> Result<()>;
    fn delete_session(&self, session_id: &str) -> bool;
    async fn chat(
        &self,
        session_id: &str,
        config: AgentRunConfig,
        user_message: String,
        options: AgentRunOptions,
    ) -> Result<ChatResult>;
}

#[async_trait]
impl AgentService for AgentEngine {
    async fn get_all_agents(&self) -> Vec<Value> {
        AgentEngine::get_all_agents(self)
    }
    async fn create_session(&self, session_id: &str, meta: SessionMeta) -> Result<()> {
        AgentEngine::create_session(self, session_id, meta)
    }
    fn delete_session(&self, session_id: &str) -> bool {
        AgentEngine::delete_session(self, session_id)
    }
    async fn chat(
        &self,
        session_id: &str,
        config: AgentRunConfig,
        user_message: String,
        options: AgentRunOptions,
    ) -> Result<ChatResult> {
        AgentEngine::chat(self, session_id, &config, &user_message, &options).await
    }
}
