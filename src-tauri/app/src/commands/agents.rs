//! `window.api.subAgent.invoke` → `sub_agent_invoke` (the `sub-agent:invoke` handler). Runs on
//! the same `AgentEngine` as background agents; policy violations and run errors reject with
//! their message, which the calling tool reports to the model.

use super::ipc_params;
use crate::backend::Backend;
use crate::errors;
use tauri::ipc::Request;
use tauri::State;
use tools::{SubAgentInvokeParams, SubAgentInvokeResult};

/// Params object as-is (`SubAgentInvokeParams`).
#[tauri::command]
pub async fn sub_agent_invoke(
    backend: State<'_, Backend>,
    request: Request<'_>,
) -> Result<SubAgentInvokeResult, String> {
    let params: SubAgentInvokeParams =
        serde_json::from_value(ipc_params(&request)).map_err(errors::plain)?;
    tracing::debug!(
        category = "sub-agent:ipc",
        caller_agent_id = ?params.caller_agent_id,
        agent_id = %params.agent_id,
        depth = params.depth,
        "Sub-agent invoke request"
    );
    backend
        .sub_agents
        .invoke(params)
        .await
        .map_err(errors::plain)
}
