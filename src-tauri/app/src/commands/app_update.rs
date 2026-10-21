//! `window.api.appUpdate.*`: in-app updates from the GitHub releases, over
//! `tauri-plugin-updater`.
//!
//! The manifest (`latest.json`) and the bundles it points at are signed in CI with the update
//! key; the plugin checks every download against the public key in `tauri.conf.json` before
//! anything is installed. Only the main window's capability grants these commands.
//!
//! On macOS and Linux the downloaded bundle replaces the installed app right away and takes
//! effect on the next start. On Windows installing means running the NSIS installer, which
//! closes the app, so the download is kept until `app_update_restart` (or quit) installs it.

use crate::errors;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The update found by the last check, and its downloaded bundle while it awaits installing.
#[derive(Default)]
pub struct UpdateState(Mutex<Pending>);

#[derive(Default)]
struct Pending {
    update: Option<Update>,
    /// Windows: the downloaded installer, run by restart or on quit.
    bytes: Option<Vec<u8>>,
    /// The new version is in place and needs only a restart.
    ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

/// The running version (from tauri.conf.json, so it matches the release tags).
#[tauri::command]
pub fn app_update_current_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Ask the release endpoint whether a newer version exists. Downloads nothing.
#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateCheck, String> {
    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(errors::plain)?
        .check()
        .await
        .map_err(errors::plain)?;
    let result = UpdateCheck {
        current_version,
        available: update.is_some(),
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.as_ref().and_then(|u| u.body.clone()),
    };
    let mut pending = state.0.lock().map_err(errors::plain)?;
    // A newer release than the one already downloaded replaces it.
    let same = matches!((&pending.update, &update), (Some(a), Some(b)) if a.version == b.version);
    if !same {
        *pending = Pending {
            update,
            ..Pending::default()
        };
    }
    Ok(result)
}

/// Download the update from the last check (verifying its signature) and put it in place:
/// installed on macOS and Linux, held for installing on Windows. Returns the new version.
#[tauri::command]
pub async fn app_update_download(state: State<'_, UpdateState>) -> Result<String, String> {
    let update = {
        let pending = state.0.lock().map_err(errors::plain)?;
        if pending.ready {
            if let Some(update) = &pending.update {
                return Ok(update.version.clone());
            }
        }
        pending
            .update
            .clone()
            .ok_or_else(|| "No update to download; check first".to_string())?
    };
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(errors::plain)?;
    let mut held = None;
    if cfg!(windows) {
        held = Some(bytes);
    } else {
        update.install(bytes).map_err(errors::plain)?;
    }
    let mut pending = state.0.lock().map_err(errors::plain)?;
    pending.bytes = held;
    pending.ready = true;
    Ok(update.version.clone())
}

/// Restart into the downloaded version (on Windows this runs its installer, which restarts
/// the app when it is done).
#[tauri::command]
pub fn app_update_restart(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    install_held(&state)?;
    app.restart()
}

/// Windows: run a downloaded-but-not-installed update. Called on quit, so "Later" still ends
/// up on the new version. A no-op everywhere else.
pub fn install_on_exit(app: &AppHandle) {
    if let Some(state) = app.try_state::<UpdateState>() {
        if let Err(e) = install_held(&state) {
            tracing::warn!(error = %e, "Could not install the downloaded update on quit");
        }
    }
}

fn install_held(state: &UpdateState) -> Result<(), String> {
    let mut pending = state.0.lock().map_err(errors::plain)?;
    if let (Some(update), Some(bytes)) = (pending.update.clone(), pending.bytes.take()) {
        update.install(bytes).map_err(errors::plain)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    /// `plugins.updater` in tauri.conf.json, as the plugin itself reads it at startup.
    fn updater_config() -> tauri_plugin_updater::Config {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        serde_json::from_value(conf["plugins"]["updater"].clone()).unwrap()
    }

    #[test]
    fn updater_config_parses_and_points_at_the_github_release() {
        let config = updater_config();
        assert!(!config.dangerous_insecure_transport_protocol);
        assert!(!config.dangerous_accept_invalid_certs);
        assert_eq!(config.endpoints.len(), 1);
        assert_eq!(
            config.endpoints[0].as_str(),
            "https://github.com/ibehren1/bedrock-engineer-rust/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn updater_pubkey_is_a_minisign_public_key() {
        // The .pub file's contents, base64-encoded, as `tauri signer generate` writes them.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(updater_config().pubkey)
            .unwrap();
        let text = String::from_utf8(decoded).unwrap();
        let mut lines = text.lines();
        assert!(lines
            .next()
            .unwrap()
            .starts_with("untrusted comment: minisign public key"));
        // Ed25519 public key: "Ed" + 8-byte key id + 32-byte key = 42 bytes.
        let key = base64::engine::general_purpose::STANDARD
            .decode(lines.next().unwrap())
            .unwrap();
        assert_eq!(key.len(), 42);
        assert_eq!(&key[..2], b"Ed");
    }
}
