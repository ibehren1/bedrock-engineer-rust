//! The application menu, About dialog, zoom and the Copy/Paste context menu: the port of
//! `createMenu`, `showAboutDialog`, the `before-input-event` zoom/reload keys and the
//! `context-menu` handler in Electron's `src/main/index.ts`.
//!
//! Tauri builds a default menu on macOS only, so the menu is set explicitly on every platform.
//! On Windows and Linux it is the menu bar of every window (as Electron's application menu was).
//!
//! Where a muda predefined item is unsupported on a platform (most edit items on Linux,
//! paste-and-match-style / delete everywhere through Tauri's API), a custom item performs the
//! same action, so the menu has the same entries as Electron's template.

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::menu::{
    Menu, MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder,
    HELP_SUBMENU_ID, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

/// Help > GitHub Repository.
pub const REPOSITORY_URL: &str = "https://github.com/ibehren1/bedrock-engineer-rust";

/// Zoom In / Zoom Out step (Electron: `getZoomFactor() ± 0.1`).
const ZOOM_STEP: f64 = 0.1;
/// Zoom Out floor (Electron: `Math.max(0.1, …)`).
const ZOOM_MIN: f64 = 0.1;
/// Zoom In ceiling. Electron had none, but Chromium caps the factor at 5.0 (500%); without a cap
/// the tracked factor would drift away from what the webview actually shows.
const ZOOM_MAX: f64 = 5.0;
/// Reset Zoom (and every new webview's starting factor).
const ZOOM_DEFAULT: f64 = 1.0;

/// A menu item (or keyboard shortcut) handled in Rust. Items that are muda predefined items on a
/// platform are run by the OS and never reach [`run_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    About,
    OpenRepository,
    Reload,
    ForceReload,
    ToggleDevtools,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    /// Custom on Windows / Linux (the predefined item is macOS-only).
    ToggleFullscreen,
    /// Edit items that are custom where muda has no predefined item.
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteAndMatchStyle,
    Delete,
    SelectAll,
    /// Window items that are custom on Linux (GTK 3 has no predefined ones).
    Minimize,
    Maximize,
    CloseWindow,
}

/// Every action and its menu id.
const ACTIONS: &[(MenuAction, &str)] = &[
    (MenuAction::About, "app-menu:about"),
    (MenuAction::OpenRepository, "app-menu:open-repository"),
    (MenuAction::Reload, "app-menu:reload"),
    (MenuAction::ForceReload, "app-menu:force-reload"),
    (MenuAction::ToggleDevtools, "app-menu:toggle-devtools"),
    (MenuAction::ZoomIn, "app-menu:zoom-in"),
    (MenuAction::ZoomOut, "app-menu:zoom-out"),
    (MenuAction::ResetZoom, "app-menu:reset-zoom"),
    (MenuAction::ToggleFullscreen, "app-menu:toggle-fullscreen"),
    (MenuAction::Undo, "app-menu:undo"),
    (MenuAction::Redo, "app-menu:redo"),
    (MenuAction::Cut, "app-menu:cut"),
    (MenuAction::Copy, "app-menu:copy"),
    (MenuAction::Paste, "app-menu:paste"),
    (
        MenuAction::PasteAndMatchStyle,
        "app-menu:paste-and-match-style",
    ),
    (MenuAction::Delete, "app-menu:delete"),
    (MenuAction::SelectAll, "app-menu:select-all"),
    (MenuAction::Minimize, "app-menu:minimize"),
    (MenuAction::Maximize, "app-menu:maximize"),
    (MenuAction::CloseWindow, "app-menu:close-window"),
];

impl MenuAction {
    /// The menu item id.
    pub fn id(self) -> &'static str {
        ACTIONS
            .iter()
            .find(|(a, _)| *a == self)
            .map(|(_, id)| *id)
            .expect("every MenuAction has an id")
    }

    /// The action for a menu item id; `None` for ids this module doesn't own (predefined items).
    pub fn from_id(id: &str) -> Option<Self> {
        ACTIONS.iter().find(|(_, i)| *i == id).map(|(a, _)| *a)
    }

    /// The actions the renderer may trigger with [`app_menu_action`] (keyboard shortcuts the menu
    /// accelerators can miss), by their camelCase name.
    pub fn from_shortcut_name(name: &str) -> Option<Self> {
        match name {
            "zoomIn" => Some(Self::ZoomIn),
            "zoomOut" => Some(Self::ZoomOut),
            "resetZoom" => Some(Self::ResetZoom),
            "reload" => Some(Self::Reload),
            _ => None,
        }
    }

    /// The accelerator shown on (and bound to) the item on this platform. Electron's roles
    /// supplied these implicitly.
    pub fn accelerator(self) -> Option<&'static str> {
        match self {
            Self::Reload => Some("CmdOrCtrl+R"),
            Self::ForceReload => Some("CmdOrCtrl+Shift+R"),
            Self::ToggleDevtools if cfg!(target_os = "macos") => Some("Alt+Cmd+I"),
            Self::ToggleDevtools => Some("Ctrl+Shift+I"),
            // Electron bound CommandOrControl+Plus and also accepted `=` in before-input-event;
            // muda has no Plus key, so the item is bound to `=` (the unshifted key) and the
            // renderer handles `+` (see `src/renderer/src/lib/nativeMenus.ts`).
            Self::ZoomIn => Some("CmdOrCtrl+="),
            Self::ZoomOut => Some("CmdOrCtrl+-"),
            Self::ResetZoom => Some("CmdOrCtrl+0"),
            Self::ToggleFullscreen => Some("F11"),
            Self::PasteAndMatchStyle => Some("Alt+Shift+Cmd+V"),
            // Linux edit items are custom; binding Ctrl+Z etc. as GTK accelerators would run
            // before the page sees the key (breaking e.g. the code editor's own undo).
            // WebKitGTK handles those keys itself.
            _ => None,
        }
    }
}

/// The zoom factor after `action` from `current`, or `None` if `action` isn't a zoom action.
/// Rounded to 2 decimals so repeated steps don't accumulate float error.
pub fn next_zoom(current: f64, action: MenuAction) -> Option<f64> {
    let next = match action {
        MenuAction::ZoomIn => (current + ZOOM_STEP).min(ZOOM_MAX),
        MenuAction::ZoomOut => (current - ZOOM_STEP).max(ZOOM_MIN),
        MenuAction::ResetZoom => ZOOM_DEFAULT,
        _ => return None,
    };
    Some((next * 100.0).round() / 100.0)
}

/// The zoom factor last set per window label (Tauri has no zoom getter). Missing = 1.0.
#[derive(Default)]
pub struct ZoomState(Mutex<HashMap<String, f64>>);

impl ZoomState {
    /// Apply `action` to the tracked factor of `label`; the new factor.
    fn step(&self, label: &str, action: MenuAction) -> Option<f64> {
        let mut levels = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let current = levels.get(label).copied().unwrap_or(ZOOM_DEFAULT);
        let next = next_zoom(current, action)?;
        levels.insert(label.to_string(), next);
        Some(next)
    }

    fn forget(&self, label: &str) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(label);
    }
}

/// The About dialog text (Electron: name, then `Version`, then the runtime versions).
pub fn about_text(
    name: &str,
    version: &str,
    tauri_version: &str,
    webview: &str,
    webview_version: &str,
) -> String {
    format!("{name}\n\nVersion: {version}\n\nTauri: {tauri_version}\n{webview}: {webview_version}")
}

/// The platform webview's name, for the About dialog.
const WEBVIEW_NAME: &str = if cfg!(target_os = "macos") {
    "WebKit"
} else if cfg!(windows) {
    "WebView2"
} else {
    "WebKitGTK"
};

/// `showAboutDialog()`.
fn show_about<R: Runtime>(app: &AppHandle<R>) {
    let info = app.package_info();
    let webview_version = tauri::webview_version().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Failed to read the webview version");
        "unknown".into()
    });
    let text = about_text(
        &info.name,
        &info.version.to_string(),
        tauri::VERSION,
        WEBVIEW_NAME,
        &webview_version,
    );
    app.dialog()
        .message(text)
        .title(format!("About {}", info.name))
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| {});
}

fn item<R: Runtime>(
    app: &AppHandle<R>,
    action: MenuAction,
    text: &str,
) -> tauri::Result<MenuItem<R>> {
    let builder = MenuItemBuilder::with_id(action.id(), text);
    match action.accelerator() {
        Some(accelerator) => builder.accelerator(accelerator),
        None => builder,
    }
    .build(app)
}

/// Edit menu (Electron: undo, redo, | cut, copy, paste, then pasteAndMatchStyle, delete,
/// selectAll on macOS; delete, |, selectAll elsewhere).
fn edit_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let edit = SubmenuBuilder::new(app, "Edit");
    #[cfg(not(target_os = "linux"))]
    let edit = edit.undo().redo().separator().cut().copy().paste();
    #[cfg(target_os = "linux")]
    let edit = edit
        .item(&item(app, MenuAction::Undo, "Undo")?)
        .item(&item(app, MenuAction::Redo, "Redo")?)
        .separator()
        .item(&item(app, MenuAction::Cut, "Cut")?)
        .item(&item(app, MenuAction::Copy, "Copy")?)
        .item(&item(app, MenuAction::Paste, "Paste")?);
    #[cfg(target_os = "macos")]
    let edit = edit
        .item(&item(
            app,
            MenuAction::PasteAndMatchStyle,
            "Paste and Match Style",
        )?)
        .item(&item(app, MenuAction::Delete, "Delete")?)
        .select_all();
    #[cfg(windows)]
    let edit = edit
        .item(&item(app, MenuAction::Delete, "Delete")?)
        .separator()
        .select_all();
    #[cfg(target_os = "linux")]
    let edit = edit
        .item(&item(app, MenuAction::Delete, "Delete")?)
        .separator()
        .item(&item(app, MenuAction::SelectAll, "Select All")?);
    edit.build()
}

/// View menu: reload, force reload, devtools, | zoom in/out/reset, | fullscreen.
fn view_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let view = SubmenuBuilder::new(app, "View")
        .item(&item(app, MenuAction::Reload, "Reload")?)
        .item(&item(app, MenuAction::ForceReload, "Force Reload")?)
        .item(&item(
            app,
            MenuAction::ToggleDevtools,
            "Toggle Developer Tools",
        )?)
        .separator()
        .item(&item(app, MenuAction::ZoomIn, "Zoom In")?)
        .item(&item(app, MenuAction::ZoomOut, "Zoom Out")?)
        .item(&item(app, MenuAction::ResetZoom, "Reset Zoom")?)
        .separator();
    #[cfg(target_os = "macos")]
    let view = view.fullscreen_with_text("Toggle Full Screen");
    #[cfg(not(target_os = "macos"))]
    let view = view.item(&item(
        app,
        MenuAction::ToggleFullscreen,
        "Toggle Full Screen",
    )?);
    view.build()
}

/// Window menu. Its id makes it the macOS Window menu (the open-window list, Electron's
/// `{ role: 'window' }`).
fn window_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let window = SubmenuBuilder::with_id(app, WINDOW_SUBMENU_ID, "Window");
    #[cfg(target_os = "macos")]
    let window = window
        .minimize()
        .maximize_with_text("Zoom")
        // Electron bound Cmd+W to hiding the main window, but the port keeps closing it (the app
        // quits) because nothing re-shows a hidden window from the Dock yet.
        .close_window()
        .separator()
        .bring_all_to_front();
    #[cfg(windows)]
    let window = window.minimize().maximize().close_window();
    #[cfg(target_os = "linux")]
    let window = window
        .item(&item(app, MenuAction::Minimize, "Minimize")?)
        .item(&item(app, MenuAction::Maximize, "Maximize")?)
        .item(&item(app, MenuAction::CloseWindow, "Close")?);
    window.build()
}

/// The application menu (`Builder::menu`).
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let name = app.package_info().name.clone();
    let about_label = format!("About {name}");
    let menu = MenuBuilder::new(app);

    #[cfg(target_os = "macos")]
    let menu = menu.item(
        &SubmenuBuilder::new(app, &name)
            .item(&item(app, MenuAction::About, &about_label)?)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?,
    );

    let help = SubmenuBuilder::with_id(app, HELP_SUBMENU_ID, "Help")
        .item(&item(app, MenuAction::About, &about_label)?)
        .separator()
        .item(&item(app, MenuAction::OpenRepository, "GitHub Repository")?)
        .build()?;

    menu.item(&edit_menu(app)?)
        .item(&view_menu(app)?)
        .item(&window_menu(app)?)
        .item(&help)
        .build()
}

/// The window a menu item applies to: the focused one, else the main window (e.g. on macOS
/// while a devtools window has focus).
fn target_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false))
        .or_else(|| app.get_webview_window("main"))
}

/// `Builder::on_menu_event`: app menu and context menu clicks on custom items.
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if let Some(action) = MenuAction::from_id(event.id().as_ref()) {
        run_action(app, action, None);
    }
}

/// Forget a closed window's zoom factor (a reused label starts at 1.0 again).
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::Destroyed = event {
        if let Some(zoom) = window.try_state::<ZoomState>() {
            zoom.forget(window.label());
        }
    }
}

/// Perform `action` on `window` (or the [`target_window`] when `None`).
pub fn run_action<R: Runtime>(
    app: &AppHandle<R>,
    action: MenuAction,
    window: Option<WebviewWindow<R>>,
) {
    match action {
        MenuAction::About => return show_about(app),
        MenuAction::OpenRepository => {
            if let Err(e) = app.opener().open_url(REPOSITORY_URL, None::<&str>) {
                tracing::error!(url = REPOSITORY_URL, error = %e, "Failed to open external URL");
            }
            return;
        }
        _ => {}
    }
    let Some(window) = window.or_else(|| target_window(app)) else {
        return;
    };
    let result = match action {
        MenuAction::Reload => window.reload(),
        MenuAction::ForceReload => force_reload(&window),
        MenuAction::ToggleDevtools => {
            // Windows: `is_devtools_open` is unsupported (always false) and devtools can't be
            // closed programmatically, so this only opens them there.
            if window.is_devtools_open() {
                window.close_devtools();
            } else {
                window.open_devtools();
            }
            Ok(())
        }
        MenuAction::ZoomIn | MenuAction::ZoomOut | MenuAction::ResetZoom => {
            match app
                .try_state::<ZoomState>()
                .and_then(|z| z.step(window.label(), action))
            {
                Some(factor) => window.set_zoom(factor),
                None => Ok(()),
            }
        }
        MenuAction::ToggleFullscreen => window
            .is_fullscreen()
            .and_then(|full| window.set_fullscreen(!full)),
        MenuAction::Minimize => window.minimize(),
        MenuAction::Maximize => window.is_maximized().and_then(|max| {
            if max {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }),
        MenuAction::CloseWindow => window.close(),
        MenuAction::Undo
        | MenuAction::Redo
        | MenuAction::Cut
        | MenuAction::Copy
        | MenuAction::Paste
        | MenuAction::PasteAndMatchStyle
        | MenuAction::Delete
        | MenuAction::SelectAll => edit_command(&window, action),
        MenuAction::About | MenuAction::OpenRepository => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!(?action, window = window.label(), error = %e, "Menu action failed");
    }
}

/// Force Reload: reload bypassing the cache. Windows has no WebView2 API for that short of the
/// DevTools protocol, so it is a plain reload there (app pages come from the asset protocol,
/// which isn't HTTP-cached, so this only matters for the dev server).
fn force_reload<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        window.with_webview(|webview| unsafe {
            let wk: &objc2_web_kit::WKWebView =
                &*(webview.inner() as *const objc2_web_kit::WKWebView);
            wk.reloadFromOrigin_(None);
        })
    }
    #[cfg(target_os = "linux")]
    {
        window.with_webview(|webview| {
            use webkit2gtk::WebViewExt;
            webview.inner().reload_bypass_cache();
        })
    }
    #[cfg(windows)]
    {
        window.reload()
    }
}

/// Edit actions muda has no predefined item for on this platform.
fn edit_command<R: Runtime>(window: &WebviewWindow<R>, action: MenuAction) -> tauri::Result<()> {
    // macOS: send the AppKit action down the responder chain, as a predefined item would
    // (`pasteAsPlainText:` / `delete:` reach the focused WKWebView).
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        let selector = match action {
            MenuAction::PasteAndMatchStyle => objc2::sel!(pasteAsPlainText:),
            MenuAction::Delete => objc2::sel!(delete:),
            MenuAction::Undo => objc2::sel!(undo:),
            MenuAction::Redo => objc2::sel!(redo:),
            MenuAction::Cut => objc2::sel!(cut:),
            MenuAction::Copy => objc2::sel!(copy:),
            MenuAction::Paste => objc2::sel!(paste:),
            _ => objc2::sel!(selectAll:),
        };
        // Menu events are delivered on the main thread.
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            unsafe { app.sendAction_to_from(selector, None, None) };
        }
        Ok(())
    }
    // Linux: WebKitGTK editing commands on the window's webview.
    #[cfg(target_os = "linux")]
    {
        let command = match action {
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Cut => "Cut",
            MenuAction::Copy => "Copy",
            MenuAction::Paste => "Paste",
            MenuAction::PasteAndMatchStyle => "PasteAsPlainText",
            MenuAction::Delete => "Delete",
            _ => "SelectAll",
        };
        window.with_webview(move |webview| {
            use webkit2gtk::WebViewExt;
            webview.inner().execute_editing_command(command);
        })
    }
    // Windows: only Delete is custom (the rest are predefined items).
    #[cfg(windows)]
    {
        let command = match action {
            MenuAction::Undo => "undo",
            MenuAction::Redo => "redo",
            MenuAction::Cut => "cut",
            MenuAction::Copy => "copy",
            MenuAction::Paste => "paste",
            MenuAction::SelectAll => "selectAll",
            _ => "delete",
        };
        window.eval(format!("document.execCommand('{command}')"))
    }
}

/// Electron's context menu: only Copy and Paste, at the cursor. Linux uses custom items
/// (WebKitGTK editing commands) because muda's predefined ones need libxdo there.
fn context_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    #[cfg(not(target_os = "linux"))]
    {
        Menu::with_items(
            app,
            &[
                &tauri::menu::PredefinedMenuItem::copy(app, Some("Copy"))?,
                &tauri::menu::PredefinedMenuItem::paste(app, Some("Paste"))?,
            ],
        )
    }
    #[cfg(target_os = "linux")]
    {
        Menu::with_items(
            app,
            &[
                &item(app, MenuAction::Copy, "Copy")?,
                &item(app, MenuAction::Paste, "Paste")?,
            ],
        )
    }
}

/// Pop up the Copy/Paste context menu at the cursor in the calling window. The renderer calls
/// this from its `contextmenu` listener (`src/renderer/src/lib/nativeMenus.ts`). Async so the
/// (modal, on macOS) popup isn't run from a synchronous command.
#[tauri::command]
pub async fn context_menu_popup(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    let menu = context_menu(&app).map_err(crate::errors::plain)?;
    window.popup_menu(&menu).map_err(crate::errors::plain)
}

/// Run a keyboard-shortcut action (`zoomIn`, `zoomOut`, `resetZoom`, `reload`) on the calling
/// window: for keys the menu accelerators don't see (`+`, and every key on Windows when the
/// webview has focus).
#[tauri::command]
pub async fn app_menu_action(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
) -> Result<(), String> {
    let action = MenuAction::from_shortcut_name(&action)
        .ok_or_else(|| format!("unknown menu action `{action}`"))?;
    run_action(&app, action, Some(window));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_round_trip_and_are_unique() {
        let ids: HashSet<_> = ACTIONS.iter().map(|(_, id)| *id).collect();
        assert_eq!(ids.len(), ACTIONS.len());
        for (action, id) in ACTIONS {
            assert_eq!(action.id(), *id);
            assert_eq!(MenuAction::from_id(id), Some(*action));
        }
    }

    #[test]
    fn foreign_ids_are_ignored() {
        assert_eq!(MenuAction::from_id("copy"), None);
        assert_eq!(MenuAction::from_id(""), None);
        assert_eq!(MenuAction::from_id("app-menu:"), None);
    }

    #[test]
    fn shortcut_names_are_limited() {
        assert_eq!(
            MenuAction::from_shortcut_name("zoomIn"),
            Some(MenuAction::ZoomIn)
        );
        assert_eq!(
            MenuAction::from_shortcut_name("zoomOut"),
            Some(MenuAction::ZoomOut)
        );
        assert_eq!(
            MenuAction::from_shortcut_name("resetZoom"),
            Some(MenuAction::ResetZoom)
        );
        assert_eq!(
            MenuAction::from_shortcut_name("reload"),
            Some(MenuAction::Reload)
        );
        assert_eq!(MenuAction::from_shortcut_name("about"), None);
        assert_eq!(MenuAction::from_shortcut_name("toggleDevtools"), None);
    }

    #[test]
    fn accelerators() {
        assert_eq!(MenuAction::ZoomIn.accelerator(), Some("CmdOrCtrl+="));
        assert_eq!(MenuAction::ZoomOut.accelerator(), Some("CmdOrCtrl+-"));
        assert_eq!(MenuAction::ResetZoom.accelerator(), Some("CmdOrCtrl+0"));
        assert_eq!(MenuAction::Reload.accelerator(), Some("CmdOrCtrl+R"));
        assert_eq!(MenuAction::Copy.accelerator(), None);
        assert_eq!(MenuAction::Undo.accelerator(), None);
    }

    #[test]
    fn zoom_steps() {
        assert_eq!(next_zoom(1.0, MenuAction::ZoomIn), Some(1.1));
        assert_eq!(next_zoom(1.0, MenuAction::ZoomOut), Some(0.9));
        assert_eq!(next_zoom(2.3, MenuAction::ResetZoom), Some(1.0));
        assert_eq!(next_zoom(1.0, MenuAction::Reload), None);
    }

    #[test]
    fn zoom_is_clamped() {
        assert_eq!(next_zoom(0.1, MenuAction::ZoomOut), Some(0.1));
        assert_eq!(next_zoom(0.15, MenuAction::ZoomOut), Some(0.1));
        assert_eq!(next_zoom(5.0, MenuAction::ZoomIn), Some(5.0));
        assert_eq!(next_zoom(4.95, MenuAction::ZoomIn), Some(5.0));
    }

    #[test]
    fn zoom_steps_do_not_drift() {
        let mut z = 1.0;
        for _ in 0..7 {
            z = next_zoom(z, MenuAction::ZoomIn).unwrap();
        }
        assert_eq!(z, 1.7);
        for _ in 0..7 {
            z = next_zoom(z, MenuAction::ZoomOut).unwrap();
        }
        assert_eq!(z, 1.0);
    }

    #[test]
    fn zoom_state_is_per_window() {
        let state = ZoomState::default();
        assert_eq!(state.step("main", MenuAction::ZoomIn), Some(1.1));
        assert_eq!(state.step("main", MenuAction::ZoomIn), Some(1.2));
        assert_eq!(state.step("task-history", MenuAction::ZoomOut), Some(0.9));
        assert_eq!(state.step("main", MenuAction::ResetZoom), Some(1.0));
        assert_eq!(state.step("main", MenuAction::Reload), None);
        state.forget("task-history");
        assert_eq!(state.step("task-history", MenuAction::ZoomIn), Some(1.1));
    }

    #[test]
    fn about_text_lists_versions() {
        assert_eq!(
            about_text(
                "Bedrock Engineer",
                "2026.930.0",
                "2.12.0",
                "WebKit",
                "621.1"
            ),
            "Bedrock Engineer\n\nVersion: 2026.930.0\n\nTauri: 2.12.0\nWebKit: 621.1"
        );
    }
}
