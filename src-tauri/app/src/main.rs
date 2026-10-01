//! Bedrock Engineer — Tauri v2 backend entry point.
//!
//! Replaces the Electron main + preload processes. Commands live in [`commands`], one module per
//! renderer namespace; startup mirrors `app.whenReady()` in `src/main/index.ts`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod commands;
mod errors;
mod files;
mod html_preview;
mod menu;
mod notify;
mod pdf;
mod security;
mod settings;
mod state;
mod store_sync;
mod webview_proxy;
mod window_startup;
mod window_state;

use state::{AppPaths, HistoryState};
use std::sync::Mutex;
use store::Store;
use tauri::webview::{NewWindowResponse, PermissionKind, PermissionResponse};
use tauri::{Manager, RunEvent, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_opener::OpenerExt;

/// App name; the config dir is `<config dir>/<APP_NAME>`.
const APP_NAME: &str = "Bedrock Engineer";
/// The fork's earlier name. Its config dir is moved to [`APP_NAME`] on first launch so settings
/// and chat history carry over.
const LEGACY_APP_NAME: &str = "Behrens AI";

fn main() {
    // fix-path: child processes (MCP stdio servers, commands) get the login shell's PATH, not the
    // minimal one a Finder/Dock/desktop-launcher start has. First, while this is the only thread
    // (it sets PATH); the result is logged once the logger exists.
    let path_fix = common::shell_env::fix_path();
    // Runs before the logger exists (the logger writes into this dir); the result is logged below.
    let adopted = store::adopt_legacy_app_dir(LEGACY_APP_NAME, APP_NAME);
    let config_path = store::default_path(APP_NAME).expect("config dir");
    // Electron's `app.getPath('userData')`: the directory config.json lives in.
    let user_data = config_path
        .parent()
        .expect("config.json has a parent dir")
        .to_path_buf();

    // initLoggerConfig(userDataPath) + initLogger(), before anything logs.
    let logger = common::logger::init_logger(common::logger::LoggerConfig::for_user_data(
        &user_data,
        cfg!(debug_assertions),
    ))
    .map_err(|e| eprintln!("failed to initialize logger: {e}"))
    .ok();
    // registerGlobalErrorHandlers(): panics go to the log file too (then to stderr as usual).
    common::logger::install_panic_hook();
    match adopted {
        Ok(true) => tracing::info!(
            from = LEGACY_APP_NAME,
            to = APP_NAME,
            "Moved config dir to the new app name"
        ),
        Ok(false) => {}
        Err(e) => {
            tracing::error!(from = LEGACY_APP_NAME, to = APP_NAME, error = %e, "Failed to move config dir to the new app name")
        }
    }
    match path_fix {
        Ok(common::shell_env::FixPathOutcome::Updated { shell, old, new }) => {
            tracing::info!(shell, old, new, "Set PATH from the login shell")
        }
        Ok(common::shell_env::FixPathOutcome::Unchanged { shell }) => {
            tracing::debug!(shell, "PATH already matches the login shell")
        }
        Ok(common::shell_env::FixPathOutcome::Skipped) => {}
        Err(e) => {
            tracing::warn!(error = %e, "Could not read PATH from the login shell; keeping the inherited PATH")
        }
    }

    // A config.json that doesn't parse is moved aside by the store (never overwritten); one that
    // can't be read at all is logged and the app starts on defaults without touching it.
    let mut store = Store::open(&config_path).unwrap_or_else(|e| {
        tracing::error!(path = %config_path.display(), error = %e, "Failed to open config store; starting with defaults");
        Store::detached(&config_path, &store::Env::current())
    });
    // `store.set('userDataPath', userDataPath)` — ChatSessionManager and friends read it.
    if let Err(e) = store.set(
        "userDataPath",
        serde_json::Value::String(user_data.to_string_lossy().into_owned()),
    ) {
        tracing::error!(error = %e, "Failed to write userDataPath");
    }

    // A history that can't be opened is logged; the app starts and history commands fail.
    let history = HistoryState::open(&user_data);
    // setupSessionProxy(): every webview of this run uses the proxy stored at launch.
    let webview_proxy = webview_proxy::WebviewProxy::from_store(&store.all());

    // The app version comes from tauri.conf.json (-> package.json, stamped from git), not the
    // Cargo package version, so logs, bundles and the About version agree.
    let context = tauri::generate_context!();
    tracing::info!(
        version = %context.package_info().version,
        platform = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        user_data_path = %user_data.display(),
        "Application started"
    );

    tauri::Builder::default()
        // Registered first: a second launch hands over to this instance and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            store_sync::focus_main_window(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Mutex::new(store))
        .manage(webview_proxy::WebviewProxyState::new(webview_proxy))
        .manage(history)
        .manage(AppPaths {
            user_data: user_data.clone(),
        })
        .manage(state::LoggerState(logger))
        .manage(commands::camera::CameraState::default())
        .manage(store_sync::FlushState::default())
        // Reopen windows where they were last closed.
        .manage(window_state::Remembered::open(&user_data))
        // Application menu (Electron createMenu): every platform, not only Tauri's macOS default.
        .manage(menu::ZoomState::default())
        .menu(menu::build)
        .on_menu_event(menu::on_menu_event)
        // getUserMedia (cameraCapture, camera previews): the camera is granted to the main and
        // camera preview webviews while they show an app page; the OS camera prompt
        // (NSCameraUsageDescription in Info.plist) still applies. Anything else: platform default.
        .on_permission_request(|webview, kind| {
            let url = webview.url().ok();
            let dev_url = security::dev_url(&webview);
            if security::allow_permission(&kind, webview.label(), url.as_ref(), dev_url.as_ref()) {
                PermissionResponse::Allow
            } else {
                if matches!(kind, PermissionKind::Camera) {
                    tracing::warn!(
                        webview = webview.label(),
                        "Denied camera permission request"
                    );
                }
                PermissionResponse::Default
            }
        })
        .register_uri_scheme_protocol(
            commands::chat_history::SESSION_PROTOCOL,
            commands::chat_history::session_protocol,
        )
        // Documents being printed by the PDF chat export.
        .register_uri_scheme_protocol(pdf::PROTOCOL, pdf::protocol)
        // Sandboxed origin for the chat's HTML previews.
        .register_uri_scheme_protocol(html_preview::PROTOCOL, html_preview::protocol)
        .invoke_handler(commands::handler())
        .setup(move |app| {
            // Bedrock / tools / MCP / agents / background / Docker services need the AppHandle
            // (events, store access), so they are built here rather than before the builder.
            // Before anything else writes to the store, so every write reaches every window.
            store_sync::install_store_listener(app.handle());
            let backend = backend::Backend::build(app.handle(), &user_data)?;
            // initializeBackgroundAgentScheduler(): only when tasks are saved.
            backend.background.initialize_scheduler();
            app.manage(backend);

            // The main window is declared in tauri.conf.json with `create: false` so it can be
            // built here with a new-window handler: like Electron's `setWindowOpenHandler`,
            // `window.open` / `target="_blank"` links open in the default browser.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .expect("main window config");
            let handle = app.handle().clone();
            // Electron's `show: false` + `ready-to-show`: built hidden, shown once the page has
            // loaded (or after a timeout), at 1800x1340 fitted to the screen.
            let reveal = window_startup::ShowWhenLoaded::new();
            // Before the first webview is created (macOS sets it on the shared data store).
            webview_proxy::install(app.handle());
            let builder = webview_proxy::configure(
                app.handle(),
                WebviewWindowBuilder::from_config(app.handle(), &config)?,
            );
            let window = reveal
                .attach(builder)
                .on_new_window(move |url, _features| {
                    if let Err(e) = handle.opener().open_url(url.as_str(), None::<&str>) {
                        tracing::error!(url = %url, error = %e, "Failed to open external URL");
                    }
                    NewWindowResponse::Deny
                })
                // Keep the window on the app's pages; other links open in the browser.
                .on_navigation(security::navigation_guard(app.handle()))
                .build()?;
            webview_proxy::attach(&window);
            // Where it was last closed, or — the first time, or when that no longer fits the
            // screen — the configured size, centered.
            let restored = match app
                .state::<window_state::Remembered>()
                .bounds(&config.label)
            {
                Some(bounds) => window_state::restore(&window.as_ref().window(), bounds),
                None => false,
            };
            if !restored {
                window_startup::fit_to_screen(
                    &window,
                    (config.width, config.height),
                    (
                        config.min_width.unwrap_or(config.width),
                        config.min_height.unwrap_or(config.height),
                    ),
                );
            }
            // Baseline for the next launch: moving or resizing the window updates it, but a window
            // left untouched gets no such event.
            app.state::<window_state::Remembered>()
                .record(&window.as_ref().window());
            reveal.arm_fallback(&window);
            Ok(())
        })
        .on_window_event(|window, event| {
            store_sync::on_window_event(window, event);
            menu::on_window_event(window, event);
            window_state::on_window_event(window, event);
            // pubSubManager.unsubscribeAll(webContents) when a window goes away.
            if let WindowEvent::Destroyed = event {
                if let Some(backend) = window.try_state::<backend::Backend>() {
                    backend.background.pubsub().unsubscribe_all(window.label());
                }
            }
        })
        .build(context)
        .expect("error while building Bedrock Engineer")
        .run(|app, event| {
            store_sync::on_run_event(app, &event);
            // Quit: record every tracked window while it still exists (destroying it writes the
            // file, but by then its geometry can no longer be read).
            if let RunEvent::ExitRequested { .. } = event {
                if let Some(remembered) = app.try_state::<window_state::Remembered>() {
                    for label in window_state::TRACKED {
                        if let Some(window) = app.get_webview_window(label) {
                            remembered.record(&window.as_ref().window());
                        }
                    }
                }
            }
            // Electron `before-quit` cleanup.
            if let RunEvent::Exit = event {
                if let Some(remembered) = app.try_state::<window_state::Remembered>() {
                    remembered.persist();
                }
                if let Some(backend) = app.try_state::<backend::Backend>() {
                    backend.shutdown();
                }
            }
        });
}
