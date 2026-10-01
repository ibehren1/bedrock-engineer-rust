//! Port of `src/main/api/bedrock/services/subAgent/SubAgentRunner.ts`.
//!
//! Policy layer for agent-to-agent delegation (the `invokeAgent` tool). The [`AgentService`]
//! already knows how to run an agent headlessly; this type owns everything specific to
//! delegation: allowlist / lineage / depth validation, ephemeral session lifecycle, wall-clock
//! timeout, concurrency limits, and reducing a full run down to the flat result the calling
//! model sees.

use crate::engine::{AgentRunConfig, AgentRunOptions, AgentService, ChatResult};
use crate::error::{Error, Result};
use crate::session::SessionMeta;
use crate::store::{default_model_id, StoreReader};
use async_trait::async_trait;
use common::delegation::{
    can_delegate, filter_delegation_targets, MAX_DELEGATION_DEPTH, MAX_DELEGATION_RESULT_CHARS,
};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tools::{StoppedReason, SubAgentInvokeParams, SubAgentInvokeResult, SubAgentInvoker};

/// Wall clock for one delegation. One inner tool can legitimately take 300 s.
pub const SUB_AGENT_TIMEOUT_MS: u64 = 10 * 60 * 1000;

/// Tool budget for one delegation. The engine default of 500 is far too high for a blocking call.
pub const SUB_AGENT_MAX_TOOL_EXECUTIONS: usize = 30;

/// Concurrent delegations. The caller can fan out tool calls, and each one costs money.
pub const SUB_AGENT_MAX_CONCURRENT: usize = 2;

/// Runs delegated agents through an [`AgentService`].
pub struct SubAgentRunner {
    service: Arc<dyn AgentService>,
    store: Arc<dyn StoreReader>,
    active: AtomicUsize,
    /// Sessions whose run outlived the timeout; they are deleted again once the run settles.
    abandoned: Arc<Mutex<HashSet<String>>>,
}

fn policy(message: impl Into<String>) -> Error {
    Error::Policy(message.into())
}

/// JS `Math.round` for non-negative values.
fn js_round(v: f64) -> u64 {
    (v + 0.5).floor() as u64
}

/// The sub-agent does not see the caller's conversation, so everything it needs has to be in
/// this one message (`buildPrompt`).
pub fn build_prompt(params: &SubAgentInvokeParams) -> String {
    let mut sections = vec![params.task.trim().to_string()];
    if let Some(c) = params
        .context
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        sections.push(format!("## Context\n\n{c}"));
    }
    if let Some(e) = params
        .expected_output
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        sections.push(format!("## Expected output\n\n{e}"));
    }
    sections.join("\n\n")
}

/// `extractText`: the text blocks of the final response, joined by newlines and trimmed.
pub fn extract_text(result: &ChatResult) -> String {
    result
        .response
        .content
        .iter()
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// `distinctToolNames`, in first-use order.
pub fn distinct_tool_names(result: &ChatResult) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for e in result.tool_executions.iter().flatten() {
        if !e.tool_name.is_empty() && !names.contains(&e.tool_name) {
            names.push(e.tool_name.clone());
        }
    }
    names
}

/// The first `max` UTF-16 code units of `text` (JS `slice(0, max)`), and whether it was cut.
fn truncate_utf16(text: &str, max: usize) -> (String, bool) {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= max {
        return (text.to_string(), false);
    }
    (String::from_utf16_lossy(&units[..max]), true)
}

fn str_field(agent: &Value, key: &str) -> Option<String> {
    agent.get(key).and_then(Value::as_str).map(str::to_string)
}

impl SubAgentRunner {
    /// `store` supplies the fallback model (`store.get('llm')?.modelId`).
    pub fn new(service: Arc<dyn AgentService>, store: Arc<dyn StoreReader>) -> Self {
        SubAgentRunner {
            service,
            store,
            active: AtomicUsize::new(0),
            abandoned: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Delegations currently running.
    pub fn active_count(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    /// Validate the request against the delegation policy; returns the permitted target ids.
    fn check_policy(params: &SubAgentInvokeParams) -> Result<Vec<String>> {
        let permitted = filter_delegation_targets(
            &params.allowed_agent_ids,
            &params.lineage,
            params.caller_agent_id.as_deref(),
        );

        if !can_delegate(params.depth, permitted.len()) {
            if params.depth >= MAX_DELEGATION_DEPTH {
                return Err(policy(format!(
                    "Delegation depth limit reached (max {MAX_DELEGATION_DEPTH}). Complete this task yourself."
                )));
            }
            return Err(policy(
                "No agents are permitted for delegation in this request. The user must @mention an agent first.",
            ));
        }

        if !permitted.contains(&params.agent_id) {
            if params.lineage.contains(&params.agent_id) {
                return Err(policy(format!(
                    "Delegation loop blocked: \"{}\" is already in the caller chain ({}).",
                    params.agent_id,
                    params.lineage.join(" -> ")
                )));
            }
            return Err(policy(format!(
                "agentId \"{}\" is not permitted in this request. The user must @mention it. Permitted: {}.",
                params.agent_id,
                permitted.join(", ")
            )));
        }
        Ok(permitted)
    }

    /// Reserve a concurrency slot; returns the number of delegations active before this one.
    fn acquire_slot(&self) -> Result<usize> {
        self.active
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < SUB_AGENT_MAX_CONCURRENT).then_some(n + 1)
            })
            .map_err(|_| {
                policy(format!(
                    "Too many concurrent delegations (max {SUB_AGENT_MAX_CONCURRENT}). Retry after the current ones finish."
                ))
            })
    }

    /// `invoke(params)`.
    pub async fn invoke(&self, params: SubAgentInvokeParams) -> Result<SubAgentInvokeResult> {
        let started = Instant::now();
        let started_at_ms = chrono::Utc::now().timestamp_millis();

        let permitted = Self::check_policy(&params)?;

        let agent = self
            .service
            .get_all_agents()
            .await
            .into_iter()
            .find(|a| a.get("id").and_then(Value::as_str) == Some(params.agent_id.as_str()))
            .ok_or_else(|| policy(format!("Agent not found: {}", params.agent_id)))?;

        let active_before = self.acquire_slot()?;

        // Run with the caller's model; fall back to the configured model.
        let model_id = params
            .model_id
            .clone()
            .filter(|m| !m.is_empty())
            .or_else(|| default_model_id(&self.store.snapshot()));
        let Some(model_id) = model_id else {
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err(policy(
                "No model is configured, so the sub-agent cannot be started. Select a model in settings.",
            ));
        };

        let depth = params.depth + 1;
        let session_id = format!(
            "subagent-{}-{}-{}-{}",
            params
                .caller_agent_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("chat"),
            params.agent_id,
            started_at_ms,
            active_before
        );
        let options = params.options.clone().unwrap_or_default();
        let timeout_ms = options.timeout_ms.unwrap_or(SUB_AGENT_TIMEOUT_MS);
        let max_tool_executions = options
            .max_tool_executions
            .unwrap_or(SUB_AGENT_MAX_TOOL_EXECUTIONS);

        let mut lineage = params.lineage.clone();
        lineage.push(params.agent_id.clone());
        let config = AgentRunConfig {
            model_id: model_id.clone(),
            agent_id: params.agent_id.clone(),
            project_directory: params.project_directory.clone(),
            delegation_depth: Some(depth),
            delegation_lineage: Some(lineage),
            allowed_delegation_agent_ids: Some(permitted),
            ..Default::default()
        };

        let outcome = self
            .run(
                &session_id,
                &params,
                config,
                max_tool_executions,
                timeout_ms,
            )
            .await;

        self.active.fetch_sub(1, Ordering::SeqCst);
        if !self.abandoned.lock().unwrap().contains(&session_id) {
            self.service.delete_session(&session_id);
        }
        let result = outcome?;

        let duration_ms = started.elapsed().as_millis() as u64;
        let final_text = extract_text(&result);
        let tool_call_count = result.tool_executions.as_ref().map_or(0, Vec::len);
        let stopped_reason = if tool_call_count >= max_tool_executions {
            StoppedReason::MaxToolExecutions
        } else {
            StoppedReason::Completed
        };

        tracing::info!(
            session_id = %session_id,
            agent_id = %params.agent_id,
            depth,
            duration_ms,
            tool_call_count,
            ?stopped_reason,
            final_text_length = final_text.len(),
            "Sub-agent completed"
        );

        let (text, truncated) = truncate_utf16(&final_text, MAX_DELEGATION_RESULT_CHARS);
        Ok(SubAgentInvokeResult {
            success: true,
            agent_id: params.agent_id.clone(),
            agent_name: str_field(&agent, "name").unwrap_or_default(),
            agent_icon: str_field(&agent, "icon"),
            agent_icon_color: str_field(&agent, "iconColor"),
            task: params.task.clone(),
            final_text: if text.is_empty() {
                "(the sub-agent produced no text output)".to_string()
            } else {
                text
            },
            truncated: truncated.then_some(true),
            tool_call_count,
            tool_names: distinct_tool_names(&result),
            duration_ms,
            depth,
            stopped_reason,
            usage: None,
            session_id,
            error: None,
        })
    }

    /// Create the session and run the chat, racing it against the wall clock. The run is not
    /// cancelled on timeout (neither was the TS `Promise.race`): it keeps going in the
    /// background and its session is deleted once it settles.
    async fn run(
        &self,
        session_id: &str,
        params: &SubAgentInvokeParams,
        config: AgentRunConfig,
        max_tool_executions: usize,
        timeout_ms: u64,
    ) -> Result<ChatResult> {
        self.service
            .create_session(
                session_id,
                SessionMeta {
                    task_id: None,
                    agent_id: Some(params.agent_id.clone()),
                    model_id: Some(config.model_id.clone()),
                    project_directory: params.project_directory.clone(),
                },
            )
            .await?;

        tracing::info!(
            session_id,
            caller_agent_id = ?params.caller_agent_id,
            agent_id = %params.agent_id,
            depth = config.delegation_depth,
            model_id = %config.model_id,
            task_length = params.task.len(),
            "Delegating to sub-agent"
        );

        let timeout = Duration::from_millis(timeout_ms);
        let options = AgentRunOptions {
            enable_tool_execution: true,
            max_tool_executions,
            timeout,
        };
        let service = self.service.clone();
        let sid = session_id.to_string();
        let prompt = build_prompt(params);
        let mut handle =
            tokio::spawn(async move { service.chat(&sid, config, prompt, options).await });

        match tokio::time::timeout(timeout, &mut handle).await {
            Ok(joined) => joined.map_err(|e| Error::Run(e.to_string()))?,
            Err(_) => {
                self.abandoned
                    .lock()
                    .unwrap()
                    .insert(session_id.to_string());
                let abandoned = self.abandoned.clone();
                let service = self.service.clone();
                let sid = session_id.to_string();
                tokio::spawn(async move {
                    let _ = handle.await;
                    abandoned.lock().unwrap().remove(&sid);
                    if !service.delete_session(&sid) {
                        tracing::debug!(session_id = %sid, "Abandoned sub-agent session already gone");
                    }
                });
                Err(policy(format!(
                    "Sub-agent \"{}\" timed out after {}s. Narrow the task and retry.",
                    params.agent_id,
                    js_round(timeout_ms as f64 / 1000.0)
                )))
            }
        }
    }
}

#[async_trait]
impl SubAgentInvoker for SubAgentRunner {
    async fn invoke(
        &self,
        params: SubAgentInvokeParams,
    ) -> std::result::Result<SubAgentInvokeResult, String> {
        tracing::debug!(
            caller_agent_id = ?params.caller_agent_id,
            agent_id = %params.agent_id,
            depth = params.depth,
            allowed_agent_ids = ?params.allowed_agent_ids,
            "Sub-agent invoke request"
        );
        SubAgentRunner::invoke(self, params)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "sub_agent_tests.rs"]
mod tests;
