//! `window.api.aws.*` → `aws_*`: AWS sign-in helpers for Settings → AWS Authentication.

/// `api.aws.listProfiles()`: the profile names in the shared AWS config and credentials files
/// (`default` first, then alphabetical). Names only, never file contents; a missing file adds
/// nothing. Read-only, so every window may call it.
#[tauri::command]
pub async fn aws_list_profiles() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(bedrock::profiles::list_profiles)
        .await
        .map_err(|e| e.to_string())
}
