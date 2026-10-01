//! OS notifications with a click action (Electron `new Notification(...).on('click')`).
//!
//! `tauri-plugin-notification` has no click callback on desktop, so:
//!
//! - **macOS**: `mac-notification-sys` (what the plugin uses underneath) with
//!   `wait_for_click`, on a helper thread that blocks until the notification is clicked,
//!   dismissed or removed from Notification Center.
//! - **Linux / BSD**: `notify-rust` over D-Bus, waiting for the `default` action on a helper
//!   thread.
//! - **Windows**: the plugin (no click action).
//!
//! The helper threads end when the notification goes away; one that is left unread in
//! Notification Center keeps its (idle) thread until it is cleared or the app quits.

use tauri::AppHandle;

/// Show a notification; `on_click` runs (on a helper thread) when the user clicks it.
pub fn show_with_click(
    app: &AppHandle,
    title: &str,
    body: &str,
    on_click: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let (title, body) = (title.to_string(), body.to_string());
        let bundle = if tauri::is_dev() {
            // An unbundled binary has no bundle id of its own (the plugin does the same).
            "com.apple.Terminal".to_string()
        } else {
            app.config().identifier.clone()
        };
        std::thread::Builder::new()
            .name("notification".into())
            .spawn(move || {
                // Fails harmlessly once the application has been set.
                let _ = mac_notification_sys::set_application(&bundle);
                let response = mac_notification_sys::Notification::new()
                    .title(&title)
                    .message(&body)
                    .default_sound()
                    .wait_for_click(true)
                    .send();
                match response {
                    Ok(mac_notification_sys::NotificationResponse::Click)
                    | Ok(mac_notification_sys::NotificationResponse::ActionButton(_)) => on_click(),
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "Failed to show notification"),
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = app;
        let handle = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .auto_icon()
            .action("default", "default")
            .show()
            .map_err(|e| e.to_string())?;
        std::thread::Builder::new()
            .name("notification".into())
            .spawn(move || {
                handle.wait_for_action(|action| {
                    if action == "default" {
                        on_click();
                    }
                })
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(windows)]
    {
        use tauri_plugin_notification::NotificationExt;
        drop(on_click);
        app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }
}
