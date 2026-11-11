//! `window.api.knowledge.*` → `knowledge_*`: the local knowledgebases, shared by every agent
//! that attaches them ([`knowledge::service::KnowledgeService`]). Main window only (`capabilities/main-window.json`).
//! Files are added only through the native picker ([`knowledge_pick_files`]), so the renderer
//! never names a path to read.
//!
//! All commands are async so their SQLite and file work stays off the main thread.
//!
//! Progress of ingestion jobs goes out as `knowledge:progress` events and every change to a
//! knowledgebase as `knowledge:changed`.

use crate::backend::{store_snapshot, Backend};
use knowledge::types::{
    JobInfo, KbGrant, KbInfo, SearchHit, SearchOrigin, SourceInfo, StorageOverview, UrlOptions,
};
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[tauri::command]
pub async fn knowledge_list(backend: State<'_, Backend>) -> Result<Vec<KbInfo>, String> {
    backend.knowledge.list()
}

/// Created with the current global embedding and chunking settings.
#[tauri::command]
pub async fn knowledge_create(
    backend: State<'_, Backend>,
    name: String,
    description: Option<String>,
) -> Result<KbInfo, String> {
    backend
        .knowledge
        .create(&name, description.as_deref().unwrap_or_default())
}

#[tauri::command]
pub async fn knowledge_update(
    backend: State<'_, Backend>,
    kb_id: String,
    name: String,
    description: Option<String>,
) -> Result<KbInfo, String> {
    backend
        .knowledge
        .update(&kb_id, &name, description.as_deref().unwrap_or_default())
}

#[tauri::command]
pub async fn knowledge_delete(backend: State<'_, Backend>, kb_id: String) -> Result<(), String> {
    backend.knowledge.delete(&kb_id).await
}

/// Re-embed with the current global embedding model. Returns the job id.
#[tauri::command]
pub async fn knowledge_rebuild(
    backend: State<'_, Backend>,
    kb_id: String,
) -> Result<String, String> {
    backend.knowledge.rebuild(&kb_id)
}

#[tauri::command]
pub async fn knowledge_list_sources(
    backend: State<'_, Backend>,
    kb_id: String,
) -> Result<Vec<SourceInfo>, String> {
    backend.knowledge.list_sources(&kb_id)
}

/// Pick files in the native dialog and add them; `[]` when canceled.
#[tauri::command]
pub async fn knowledge_pick_files(
    window: WebviewWindow,
    backend: State<'_, Backend>,
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
    backend.knowledge.add_files(&kb_id, &paths)
}

/// Pick a folder in the native dialog and add it as one source that follows the folder's
/// contents. A large folder is confirmed in a native dialog first. `None` when canceled.
#[tauri::command]
pub async fn knowledge_pick_folder(
    window: WebviewWindow,
    backend: State<'_, Backend>,
    kb_id: String,
) -> Result<Option<SourceInfo>, String> {
    let Some(dir) = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Add a folder to the knowledge base")
        .blocking_pick_folder()
        .and_then(crate::commands::file::file_path_string)
    else {
        return Ok(None);
    };
    let svc = backend.knowledge.clone();
    // `add_folder` walks the folder, so off the async runtime; the confirmation blocks too.
    tauri::async_runtime::spawn_blocking(move || {
        let confirm = |count: usize| {
            let name = std::path::Path::new(&dir)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.clone());
            window
                .dialog()
                .message(format!(
                    "\u{201c}{name}\u{201d} has {count} files to index. Reading and embedding them \
                     takes a while and, with a Bedrock embedding model, costs embedding tokens. \
                     Add the folder?"
                ))
                .title("Add a large folder?")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Add folder".into(),
                    "Cancel".into(),
                ))
                .parent(&window)
                .blocking_show()
        };
        svc.add_folder(&kb_id, &dir, confirm)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn knowledge_add_url(
    backend: State<'_, Backend>,
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
    backend.knowledge.add_url(&kb_id, &url, options)
}

/// New crawl options for a website source (used from its next sync).
#[tauri::command]
pub async fn knowledge_update_source_options(
    backend: State<'_, Backend>,
    kb_id: String,
    source_id: String,
    options: UrlOptions,
) -> Result<SourceInfo, String> {
    backend
        .knowledge
        .update_source_options(&kb_id, &source_id, options)
}

/// Returns the job id.
#[tauri::command]
pub async fn knowledge_resync_source(
    backend: State<'_, Backend>,
    kb_id: String,
    source_id: String,
) -> Result<String, String> {
    backend.knowledge.resync_source(&kb_id, &source_id)
}

#[tauri::command]
pub async fn knowledge_remove_source(
    backend: State<'_, Backend>,
    kb_id: String,
    source_id: String,
) -> Result<(), String> {
    backend.knowledge.remove_source(&kb_id, &source_id).await
}

/// Running and queued jobs, optionally of one knowledgebase.
#[tauri::command]
pub async fn knowledge_jobs(
    backend: State<'_, Backend>,
    kb_id: Option<String>,
) -> Result<Vec<JobInfo>, String> {
    Ok(backend.knowledge.jobs(kb_id.as_deref()))
}

#[tauri::command]
pub async fn knowledge_cancel_job(
    backend: State<'_, Backend>,
    job_id: String,
) -> Result<bool, String> {
    Ok(backend.knowledge.cancel_job(&job_id))
}

/// The search the agents' MCP tool runs, for trying knowledgebases out: one (`kbId`) or all.
#[tauri::command]
pub async fn knowledge_search(
    backend: State<'_, Backend>,
    kb_id: Option<String>,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let svc = &backend.knowledge;
    let scope = match kb_id.as_deref() {
        Some(kb) => vec![kb.to_string()],
        None => svc.kb_ids(),
    };
    svc.search(
        &scope,
        kb_id.as_deref(),
        &query,
        top_k,
        knowledge::types::SearchOrigin::Test,
    )
    .await
}

/// The `searchKnowledgeBase` tool for a chat turn that `@mentions` knowledgebases: searches
/// only the granted ones and returns the same text as the built-in server. The
/// renderer builds `grants` from the user's message, never from the model's input.
#[tauri::command]
pub async fn knowledge_search_granted(
    backend: State<'_, Backend>,
    grants: Vec<KbGrant>,
    query: String,
    knowledge_base: Option<String>,
    top_k: Option<usize>,
) -> Result<String, String> {
    let top_k = top_k.map(|k| k.clamp(1, 20));
    let hits = backend
        .knowledge
        .search_granted(
            &grants,
            knowledge_base.as_deref(),
            &query,
            top_k,
            SearchOrigin::Agent,
        )
        .await?;
    Ok(knowledge::mcp_server::format_hits(query.trim(), &hits))
}

/// The `listKnowledgeBaseSources` tool for a chat turn that `@mentions` knowledgebases.
#[tauri::command]
pub async fn knowledge_list_sources_granted(
    backend: State<'_, Backend>,
    grants: Vec<KbGrant>,
    knowledge_base: Option<String>,
) -> Result<String, String> {
    let svc = backend.knowledge.clone();
    let mut out = String::new();
    for kb in svc.granted(&grants, knowledge_base.as_deref())? {
        let sources = svc.list_sources(&kb.id)?;
        out.push_str(&knowledge::mcp_server::format_sources(&kb.name, &sources));
    }
    Ok(out)
}

/// Opens a cited knowledgebase source (a local file or folder) with its default app. Only
/// paths that are a document or source of some knowledgebase open, so a link the model wrote
/// can't launch anything else; web pages open through the normal link handling.
#[tauri::command]
pub async fn knowledge_open_source(
    app: AppHandle,
    backend: State<'_, Backend>,
    uri: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let svc = backend.knowledge.clone();
    let known = {
        let uri = uri.clone();
        tauri::async_runtime::spawn_blocking(move || svc.is_known_uri(&uri))
            .await
            .map_err(|e| e.to_string())?
    };
    if !known {
        return Err(format!("{uri} is not a source in any knowledge base"));
    }
    if !std::path::Path::new(&uri).exists() {
        return Err(format!("{uri} no longer exists"));
    }
    app.opener()
        .open_path(uri, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Usage (searches, latency, last job) and health checks for one knowledgebase.
#[tauri::command]
pub async fn knowledge_stats(
    backend: State<'_, Backend>,
    kb_id: String,
) -> Result<knowledge::types::KbStats, String> {
    backend.knowledge.stats(&kb_id)
}

/// Compact the file, merge the keyword index and run an integrity check.
#[tauri::command]
pub async fn knowledge_optimize(
    backend: State<'_, Backend>,
    kb_id: String,
) -> Result<knowledge::types::OptimizeResult, String> {
    backend.knowledge.optimize(&kb_id).await
}

/// Knowledgebase id → the ids of the agents in `customAgents` that attach it.
pub fn attachments(store: &Value) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for agent in store
        .get("customAgents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(agent_id) = agent.get("id").and_then(Value::as_str) else {
            continue;
        };
        for kb in crate::backend::attached_kb_ids(agent) {
            let agents = out.entry(kb).or_default();
            if !agents.iter().any(|a| a == agent_id) {
                agents.push(agent_id.to_string());
            }
        }
    }
    out
}

#[tauri::command]
pub async fn knowledge_storage_overview(
    app: AppHandle,
    backend: State<'_, Backend>,
) -> Result<StorageOverview, String> {
    Ok(backend
        .knowledge
        .storage_overview(&attachments(&store_snapshot(&app))))
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
