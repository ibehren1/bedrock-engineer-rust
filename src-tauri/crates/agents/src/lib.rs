//! Agents: lookup, system prompts, the Rust-side agent loop, and sub-agent delegation.
//!
//! Port of the agent-run half of `src/main/api/bedrock/services/backgroundAgent/
//! BackgroundAgentService.ts` (`chat`, tool loop, tool specs, system prompt),
//! `MainToolSpecProvider.ts`, `src/main/api/bedrock/services/subAgent/SubAgentRunner.ts`, the
//! `sub-agent:invoke` handler, and `src/preload/helpers/agent-helpers.ts`. The scheduler, the
//! persistent session manager and the background-agent IPC surface are Task 11; they plug in
//! through [`SessionStore`] / [`SessionListener`] and call [`AgentEngine::chat`].
//!
//! # Pieces
//!
//! * [`AgentCatalog`] — agent lookup by id (store `customAgents` + shared agents); implements
//!   [`tools::AgentResolver`].
//! * [`prompt`] — system prompt (agent system + environment context + placeholders) and the
//!   per-message `<context>` date block.
//! * [`tool_specs`] — tool configuration for a run, including the narrowed `invokeAgent` spec.
//! * [`AgentEngine`] — converse → tools → results → converse loop over a [`ConverseBackend`]
//!   and a [`tools::ToolRegistry`].
//! * [`SubAgentRunner`] — delegation policy, timeout and concurrency; implements
//!   [`tools::SubAgentInvoker`], which the `invokeAgent` tool calls.
//!
//! # Wiring (app crate)
//!
//! ```ignore
//! let store: Arc<dyn agents::StoreReader> = Arc::new(move || store_state.lock().unwrap().all());
//! let registry = Arc::new(tools::create_builtin_registry(todo.clone())); // + MCP adapter, bedrock tools
//! let converse = Arc::new(agents::BedrockConverseBackend::new(
//!     converse_service.clone(),                         // Arc<bedrock::ConverseService>
//!     Arc::new(move || converse_settings_from_store()), // bedrock::ConverseSettings per call
//! ));
//! let engine = Arc::new(
//!     agents::AgentEngine::new(converse, registry.clone(), store.clone())
//!         .with_mcp_specs(mcp_spec_provider)                 // impl McpToolSpecProvider (mcp crate)
//!         .with_tool_context(Arc::new(|snapshot| {           // sandbox / document readers
//!             let mut ctx = tools::ToolContext::from_store(snapshot, None);
//!             ctx.sandbox = Some(sandbox.clone());
//!             ctx
//!         })),
//! );
//! let runner = agents::wire_sub_agents(&engine, store.clone()); // Arc<SubAgentRunner>
//! let catalog = engine.catalog().clone();                        // Arc<AgentCatalog>
//!
//! // Renderer tool calls (`bedrock_execute_tool` / `tools_execute`):
//! let mut ctx = tools::ToolContext::from_store(&store.snapshot(), session_id);
//! ctx.agents = Some(catalog.clone());
//! ctx.sub_agents = Some(runner.clone());
//! registry.execute(input, &ctx).await;
//!
//! // `api.subAgent.invoke` -> `sub_agent_invoke` command (params object as-is):
//! #[tauri::command]
//! async fn sub_agent_invoke(runner: State<'_, Arc<SubAgentRunner>>, params: SubAgentInvokeParams)
//!     -> Result<SubAgentInvokeResult, String> {
//!     runner.invoke(params).await.map_err(|e| e.to_string())
//! }
//! ```
//!
//! Covered TS tests: `SubAgentRunner.test.ts` (`sub_agent_tests.rs`, test for test).

pub mod catalog;
pub mod converse;
pub mod engine;
pub mod error;
pub mod prompt;
pub mod session;
pub mod store;
pub mod sub_agent;
pub mod tool_specs;

pub use catalog::AgentCatalog;
pub use converse::{BedrockConverseBackend, ConverseBackend, SettingsSource};
pub use engine::{
    deduplicate_tool_use_ids, AgentEngine, AgentRunConfig, AgentRunOptions, AgentService,
    ChatResult, ToolContextFactory, ToolExecution,
};
pub use error::{Error, Result};
pub use session::{AgentMessage, InMemorySessionStore, SessionListener, SessionMeta, SessionStore};
pub use store::StoreReader;
pub use sub_agent::{
    SubAgentRunner, SUB_AGENT_MAX_CONCURRENT, SUB_AGENT_MAX_TOOL_EXECUTIONS, SUB_AGENT_TIMEOUT_MS,
};
pub use tool_specs::McpToolSpecProvider;

use std::sync::{Arc, Weak};

/// Build the [`SubAgentRunner`] for `engine` and register it with the engine, so agents run by
/// the engine can delegate further (`invokeAgent` inside a sub-agent). The engine keeps only a
/// weak reference; hold on to the returned runner.
pub fn wire_sub_agents(
    engine: &Arc<AgentEngine>,
    store: Arc<dyn StoreReader>,
) -> Arc<SubAgentRunner> {
    let runner = Arc::new(SubAgentRunner::new(engine.clone(), store));
    let weak: Weak<SubAgentRunner> = Arc::downgrade(&runner);
    engine.set_sub_agent_invoker(weak);
    runner
}

#[cfg(test)]
mod engine_tests;
