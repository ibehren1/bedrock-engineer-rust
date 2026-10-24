//! `window.api.knowledge.*` → `knowledge_*`: agents' local knowledgebases
//! ([`knowledge::service::KnowledgeService`]). Main window only (`capabilities/main-window.json`).
//! Files are added only through the native picker ([`knowledge_pick_files`]), so the renderer
//! never names a path to read.
//!
//! All commands are async so their SQLite and file work stays off the main thread.
//!
//! Progress of ingestion jobs goes out as `knowledge:progress` events and every change to a
//! knowledgebase as `knowledge:changed`.

use crate::backend::{store_snapshot, Backend};
use knowledge::types::{JobInfo, KbInfo, SearchHit, SourceInfo, StorageOverview, UrlOptions};
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn knowledge_list(
    backend: State<'_, Backend>,
    agent_id: String,
) -> Result<Vec<KbInfo>, String> {
    backend.knowledge.list(&agent_id)
}

/// Created with the current global embedding and chunking settings.
#[tauri::command]
pub async fn knowledge_create(
    backend: State<'_, Backend>,
    agent_id: String,
    name: String,
    description: Option<String>,
) -> Result<KbInfo, String> {
    backend
        .knowledge
        .create(&agent_id, &name, description.as_deref().unwrap_or_default())
}

#[tauri::command]
pub async fn knowledge_update(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
    name: String,
    description: Option<String>,
) -> Result<KbInfo, String> {
    backend.knowledge.update(
        &agent_id,
        &kb_id,
        &name,
        description.as_deref().unwrap_or_default(),
    )
}

#[tauri::command]
pub async fn knowledge_delete(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<(), String> {
    backend.knowledge.delete(&agent_id, &kb_id).await
}

/// Every knowledgebase of an agent (agent deleted, or a new agent discarded).
#[tauri::command]
pub async fn knowledge_delete_agent(
    backend: State<'_, Backend>,
    agent_id: String,
) -> Result<(), String> {
    backend.knowledge.delete_agent(&agent_id).await
}

/// Copies for a duplicated agent; returns the copies.
#[tauri::command]
pub async fn knowledge_copy_agent(
    backend: State<'_, Backend>,
    from_agent_id: String,
    to_agent_id: String,
) -> Result<Vec<KbInfo>, String> {
    backend
        .knowledge
        .copy_agent(&from_agent_id, &to_agent_id)
        .await
}

/// Re-embed with the current global embedding model. Returns the job id.
#[tauri::command]
pub async fn knowledge_rebuild(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<String, String> {
    backend.knowledge.rebuild(&agent_id, &kb_id)
}

#[tauri::command]
pub async fn knowledge_list_sources(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<Vec<SourceInfo>, String> {
    backend.knowledge.list_sources(&agent_id, &kb_id)
}

/// Pick files in the native dialog and add them; `[]` when canceled.
#[tauri::command]
pub async fn knowledge_pick_files(
    window: WebviewWindow,
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<Vec<SourceInfo>, String> {
    let extensions = knowledge::extract::supported_extensions();
    let picked = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Add files to the knowledge base")
        .add_filter("Documents", &extensions)
        .blocking_pick_files()
        .unwrap_or_default();
    let paths: Vec<String> = picked
        .into_iter()
        .filter_map(crate::commands::file::file_path_string)
        .collect();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    backend.knowledge.add_files(&agent_id, &kb_id, &paths)
}

#[tauri::command]
pub async fn knowledge_add_url(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
    url: String,
    options: Option<UrlOptions>,
) -> Result<SourceInfo, String> {
    let options = options.unwrap_or_else(|| {
        let c = backend.knowledge.settings().crawler;
        UrlOptions {
            max_depth: c.max_depth,
            max_pages: c.max_pages,
            path_prefix_only: true,
        }
    });
    backend.knowledge.add_url(&agent_id, &kb_id, &url, options)
}

/// New crawl options for a website source (used from its next sync).
#[tauri::command]
pub async fn knowledge_update_source_options(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
    source_id: String,
    options: UrlOptions,
) -> Result<SourceInfo, String> {
    backend
        .knowledge
        .update_source_options(&agent_id, &kb_id, &source_id, options)
}

/// Returns the job id.
#[tauri::command]
pub async fn knowledge_resync_source(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
    source_id: String,
) -> Result<String, String> {
    backend
        .knowledge
        .resync_source(&agent_id, &kb_id, &source_id)
}

#[tauri::command]
pub async fn knowledge_remove_source(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
    source_id: String,
) -> Result<(), String> {
    backend
        .knowledge
        .remove_source(&agent_id, &kb_id, &source_id)
        .await
}

/// Running and queued jobs, optionally for one agent.
#[tauri::command]
pub async fn knowledge_jobs(
    backend: State<'_, Backend>,
    agent_id: Option<String>,
) -> Result<Vec<JobInfo>, String> {
    Ok(backend.knowledge.jobs(agent_id.as_deref()))
}

#[tauri::command]
pub async fn knowledge_cancel_job(
    backend: State<'_, Backend>,
    job_id: String,
) -> Result<bool, String> {
    Ok(backend.knowledge.cancel_job(&job_id))
}

/// The search the agent's MCP tool runs, for trying a knowledgebase out.
#[tauri::command]
pub async fn knowledge_search(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: Option<String>,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    backend
        .knowledge
        .search(
            &agent_id,
            kb_id.as_deref(),
            &query,
            top_k,
            knowledge::types::SearchOrigin::Test,
        )
        .await
}

/// Usage (searches, latency, last job) and health checks for one knowledgebase.
#[tauri::command]
pub async fn knowledge_stats(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<knowledge::types::KbStats, String> {
    backend.knowledge.stats(&agent_id, &kb_id)
}

/// Compact the file, merge the keyword index and run an integrity check.
#[tauri::command]
pub async fn knowledge_optimize(
    backend: State<'_, Backend>,
    agent_id: String,
    kb_id: String,
) -> Result<knowledge::types::OptimizeResult, String> {
    backend.knowledge.optimize(&agent_id, &kb_id).await
}

/// Ids in `customAgents`: knowledgebases of any other agent are orphans.
fn known_agents(app: &AppHandle) -> HashSet<String> {
    store_snapshot(app)
        .get("customAgents")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("id").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub async fn knowledge_storage_overview(
    app: AppHandle,
    backend: State<'_, Backend>,
) -> Result<StorageOverview, String> {
    Ok(backend.knowledge.storage_overview(&known_agents(&app)))
}

/// Deletes the knowledgebases of agents that no longer exist; returns how many.
#[tauri::command]
pub async fn knowledge_purge_orphans(
    app: AppHandle,
    backend: State<'_, Backend>,
) -> Result<usize, String> {
    let known = known_agents(&app);
    if known.is_empty() && store_snapshot(&app).get("customAgents").is_none() {
        // Settings not readable: don't treat every knowledgebase as an orphan.
        return Err("Agent settings are not available".into());
    }
    backend.knowledge.purge_orphans(&known).await
}

#[tauri::command]
pub async fn knowledge_local_model_status(
    backend: State<'_, Backend>,
) -> Result<knowledge::local::LocalModelStatus, String> {
    Ok(backend.local_model.status())
}

/// Download the on-device embedding model (progress: `knowledge:model-download` events).
#[tauri::command]
pub async fn knowledge_download_local_model(
    app: AppHandle,
    backend: State<'_, Backend>,
) -> Result<(), String> {
    let client = crate::settings::http_client(&store_snapshot(&app));
    let emit = move |payload: Value| {
        if let Err(e) = app.emit(knowledge::local::DOWNLOAD_EVENT, payload) {
            tracing::warn!(error = %e, "Failed to emit model download progress");
        }
    };
    backend.local_model.download(&client, &emit).await
}

/// Delete the model files (stopping a download in progress).
#[tauri::command]
pub async fn knowledge_delete_local_model(backend: State<'_, Backend>) -> Result<(), String> {
    backend.local_model.delete()
}
