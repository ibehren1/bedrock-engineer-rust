//! The agent-file methods of `window.file` (`src/main/handlers/agent-handlers.ts`): shared agents,
//! YAML export/import, and organization agents in S3. The file logic is
//! `common::agent_files`; this module adds the dialogs, the store, and the S3 calls.

use super::file::file_path_string;
use crate::state::{lock_store, project_path, StoreMutex};
use bedrock::AwsSettings;
use common::agent_files::{
    self as af, AgentFileFormat, AgentFileResult, AgentListResult, AGENT_FILE_EXTENSIONS,
};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

const CATEGORY: &str = "agents:ipc";

#[derive(Debug, Default, Deserialize)]
pub struct SaveOptions {
    #[serde(default)]
    format: Option<AgentFileFormat>,
}

fn format_of(options: Option<SaveOptions>) -> AgentFileFormat {
    options.and_then(|o| o.format).unwrap_or_default()
}

#[tauri::command]
pub async fn save_shared_agent(
    store: State<'_, StoreMutex>,
    agent: Value,
    options: Option<SaveOptions>,
) -> Result<AgentFileResult, String> {
    Ok(af::save_shared_agent(
        project_path(&store).as_deref(),
        &agent,
        format_of(options),
    ))
}

/// `delete-shared-agent`: confirm in a native warning dialog, then delete.
#[tauri::command]
pub async fn delete_shared_agent(
    window: WebviewWindow,
    store: State<'_, StoreMutex>,
    file_path: String,
) -> Result<AgentFileResult, String> {
    let project = project_path(&store);
    Ok(af::delete_shared_agent(
        project.as_deref(),
        Path::new(&file_path),
        |prompt| {
            window
                .dialog()
                .message(format!("{}\n\n{}", prompt.message, prompt.detail))
                .parent(&window)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Delete".into(),
                    "Cancel".into(),
                ))
                .blocking_show()
        },
    ))
}

/// `export-agent-yaml`: save dialog in Downloads, then write the portable YAML.
#[tauri::command]
pub async fn export_agent_yaml(
    window: WebviewWindow,
    agent: Option<Value>,
) -> Result<AgentFileResult, String> {
    let name = match af::check_exportable(agent.as_ref()) {
        Ok(name) => name.to_owned(),
        Err(result) => return Ok(*result),
    };
    let mut dialog = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Download agent YAML")
        .set_file_name(af::export_default_file_name(&name))
        .add_filter("YAML", &["yaml", "yml"]);
    if let Some(downloads) = dirs::download_dir() {
        dialog = dialog.set_directory(downloads);
    }
    let Some(target) = dialog.blocking_save_file().and_then(file_path_string) else {
        return Ok(AgentFileResult::canceled());
    };
    Ok(af::export_agent_yaml(
        agent.as_ref().expect("checked"),
        Path::new(&target),
    ))
}

/// `import-agent-file`: pick an agent file and check it.
#[tauri::command]
pub async fn import_agent_file(window: WebviewWindow) -> Result<AgentFileResult, String> {
    let picked = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Import agent")
        .add_filter("Agent", AGENT_FILE_EXTENSIONS)
        .blocking_pick_file()
        .and_then(file_path_string);
    Ok(match picked {
        Some(path) => af::import_agent_file(Path::new(&path)),
        None => AgentFileResult::canceled(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Config {
    bucket: String,
    #[serde(default)]
    prefix: Option<String>,
    region: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationConfig {
    id: String,
    s3_config: S3Config,
}

/// `createS3Client({ ...store.get('aws'), region })`, or `None` when no AWS settings are stored.
async fn s3_client(store: &StoreMutex, region: &str) -> Result<Option<aws_sdk_s3::Client>, String> {
    let aws = lock_store(store)?.get("aws");
    let Some(aws) = aws.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let settings: AwsSettings = serde_json::from_value(aws).map_err(|e| e.to_string())?;
    let config = bedrock::load_sdk_config(&settings.with_region(region))
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(aws_sdk_s3::Client::new(&config)))
}

fn error_message<E: std::error::Error>(e: E) -> String {
    aws_sdk_s3::error::DisplayErrorContext(e).to_string()
}

/// `load-organization-agents`: list the bucket prefix and load every agent file in it.
#[tauri::command]
pub async fn load_organization_agents(
    store: State<'_, StoreMutex>,
    organization_config: OrganizationConfig,
) -> Result<AgentListResult, String> {
    let org = organization_config;
    tracing::info!(category = CATEGORY, bucket = %org.s3_config.bucket, prefix = ?org.s3_config.prefix, region = %org.s3_config.region, "Loading organization agents from S3");
    let failed = |e: String| {
        tracing::error!(category = CATEGORY, error = %e, "Error loading organization agents");
        Ok(AgentListResult {
            agents: Vec::new(),
            error: Some(e),
        })
    };
    let client = match s3_client(&store, &org.s3_config.region).await {
        Ok(Some(c)) => c,
        Ok(None) => return failed("AWS credentials not configured".into()),
        Err(e) => return failed(e),
    };
    let listed = client
        .list_objects_v2()
        .bucket(&org.s3_config.bucket)
        .prefix(org.s3_config.prefix.clone().unwrap_or_default())
        .send()
        .await;
    let listed = match listed {
        Ok(l) => l,
        Err(e) => return failed(error_message(e)),
    };
    let keys: Vec<String> = listed
        .contents()
        .iter()
        .filter_map(|o| o.key())
        .filter(|k| af::is_agent_file_key(k))
        .map(str::to_owned)
        .collect();

    let mut agents = Vec::new();
    for key in keys {
        let loaded: Result<Option<Value>, String> = async {
            let object = client
                .get_object()
                .bucket(&org.s3_config.bucket)
                .key(&key)
                .send()
                .await
                .map_err(error_message)?;
            let bytes = object
                .body
                .collect()
                .await
                .map_err(|e| e.to_string())?
                .into_bytes();
            let content = String::from_utf8_lossy(&bytes);
            if content.is_empty() {
                tracing::warn!(category = CATEGORY, key = %key, "Empty file content");
                return Ok(None);
            }
            af::organization_agent_from_content(&content, &key, &org.id).map(Some)
        }
        .await;
        match loaded {
            Ok(Some(agent)) => agents.push(agent),
            Ok(None) => {}
            Err(e) => {
                tracing::error!(category = CATEGORY, key = %key, error = %e, "Error reading organization agent file")
            }
        }
    }
    Ok(AgentListResult {
        agents,
        error: None,
    })
}

/// `save-agent-to-organization`: upload the agent as `<prefix>/<slug>.<ext>`.
#[tauri::command]
pub async fn save_agent_to_organization(
    store: State<'_, StoreMutex>,
    agent: Value,
    organization_config: OrganizationConfig,
    options: Option<SaveOptions>,
) -> Result<AgentFileResult, String> {
    let org = organization_config;
    let format = format_of(options);
    let agent_name = agent
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    tracing::info!(category = CATEGORY, agent_name, bucket = %org.s3_config.bucket, prefix = ?org.s3_config.prefix, "Saving agent to organization S3");
    let failed = |e: String| {
        tracing::error!(category = CATEGORY, error = %e, "Error saving agent to organization");
        Ok(AgentFileResult::error(e))
    };
    let upload = match af::organization_agent_upload(
        &agent,
        &org.id,
        org.s3_config.prefix.as_deref(),
        format,
    ) {
        Ok(u) => u,
        Err(e) => return failed(e),
    };
    let client = match s3_client(&store, &org.s3_config.region).await {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(AgentFileResult::error("AWS credentials not configured")),
        Err(e) => return failed(e),
    };
    let sent = client
        .put_object()
        .bucket(&org.s3_config.bucket)
        .key(&upload.s3_key)
        .body(upload.body.into_bytes().into())
        .content_type(upload.content_type)
        .send()
        .await;
    match sent {
        Ok(_) => Ok(AgentFileResult {
            success: true,
            s3_key: Some(upload.s3_key),
            format: Some(format),
            ..Default::default()
        }),
        Err(e) => failed(error_message(e)),
    }
}
