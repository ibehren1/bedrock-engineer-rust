//! Webview origin checks: which origins count as the app itself, where the app windows may
//! navigate, and which webviews get the camera.
//!
//! The app's own pages are served from `tauri://localhost` (macOS / Linux) or
//! `http(s)://tauri.localhost` (Windows), or from `build.devUrl` under `tauri dev`. Everything
//! else — preview iframes, remote sites a link points to — is some other origin and must not get
//! app privileges (IPC is separately limited by the capabilities in `capabilities/`).

use tauri::webview::PermissionKind;
use tauri::{Manager, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

/// The origins the bundled frontend is served from (`scheme://host[:port]`).
const APP_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];

/// Hosts that the renderer embeds in iframes. On macOS the navigation handler also sees
/// sub-frame navigations, so these must be let through (they're cross-origin frames with no IPC
/// access): sandpack's bundler for the Website Generator, and diagrams.net for draw.io.
const EMBED_HOST_SUFFIXES: &[&str] = &[".codesandbox.io"];
const EMBED_HOSTS: &[&str] = &["embed.diagrams.net"];

/// `build.devUrl` when running a debug build (`tauri dev` serves the UI from there).
pub fn dev_url<R: Runtime, M: Manager<R>>(manager: &M) -> Option<Url> {
    if cfg!(debug_assertions) {
        manager.config().build.dev_url.clone()
    } else {
        None
    }
}

/// `scheme://host[:port]` of `url` (`None` without a host, e.g. `about:` or `data:`). Built by
/// hand because `Url::origin` is opaque for non-special schemes like `tauri:`.
fn origin_of(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}

/// Whether the serialized origin `origin` (e.g. a request's `Origin` header) is the app's.
pub fn is_app_origin(origin: &str, dev_url: Option<&Url>) -> bool {
    let origin = origin.trim_end_matches('/');
    APP_ORIGINS.contains(&origin)
        || dev_url
            .and_then(origin_of)
            .is_some_and(|dev| dev.eq_ignore_ascii_case(origin))
}

/// Whether `url` is one of the app's own pages.
pub fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
    origin_of(url).is_some_and(|o| is_app_origin(&o, dev_url))
}

/// What to do with a navigation of an app window (main frame, or on macOS any frame).
#[derive(Debug, PartialEq, Eq)]
pub enum Navigation {
    /// App pages, `about:blank` / `about:srcdoc`, `blob:`, the HTML preview scheme and the
    /// embedded iframe hosts.
    Allow,
    /// A link to somewhere else: cancel it and open it in the default browser, like the
    /// `window.open` handler does.
    OpenExternally,
    /// Anything else (`file:`, `data:`, `javascript:`, unknown schemes): cancel.
    Block,
}

pub fn navigation(url: &Url, dev_url: Option<&Url>) -> Navigation {
    if is_app_url(url, dev_url) {
        return Navigation::Allow;
    }
    match url.scheme() {
        "about" | "blob" => return Navigation::Allow,
        s if s == crate::html_preview::PROTOCOL => return Navigation::Allow,
        _ => {}
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match url.scheme() {
        // Windows serves custom schemes as http://<scheme>.localhost.
        "http" if host == format!("{}.localhost", crate::html_preview::PROTOCOL) => {
            Navigation::Allow
        }
        "https"
            if EMBED_HOSTS.contains(&host.as_str())
                || EMBED_HOST_SUFFIXES.iter().any(|s| host.ends_with(s)) =>
        {
            Navigation::Allow
        }
        "http" | "https" | "mailto" => Navigation::OpenExternally,
        _ => Navigation::Block,
    }
}

/// `WebviewWindowBuilder::on_navigation` handler for the main and task history windows: keeps
/// them on the app's pages; external links open in the default browser instead.
pub fn navigation_guard<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> impl Fn(&Url) -> bool + Send + 'static {
    let app = app.clone();
    let dev = dev_url(&app);
    move |url| match navigation(url, dev.as_ref()) {
        Navigation::Allow => true,
        Navigation::OpenExternally => {
            tracing::info!(url = %url, "Opening navigation in the default browser");
            if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
                tracing::error!(url = %url, error = %e, "Failed to open external URL");
            }
            false
        }
        Navigation::Block => {
            tracing::warn!(url = %url, "Blocked navigation");
            false
        }
    }
}

/// Webview labels that use `getUserMedia`: the main window (cameraCapture frames) and the
/// camera preview windows.
fn uses_camera(label: &str) -> bool {
    label == "main" || label.starts_with(crate::commands::camera::PREVIEW_LABEL_PREFIX)
}

/// `on_permission_request`: camera access only for the webviews that use it, and only while
/// they show an app page. Everything else gets the platform default.
pub fn allow_permission(
    kind: &PermissionKind,
    label: &str,
    url: Option<&Url>,
    dev_url: Option<&Url>,
) -> bool {
    matches!(kind, PermissionKind::Camera)
        && uses_camera(label)
        && url.is_some_and(|u| is_app_url(u, dev_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn app_origins() {
        let dev = url("http://localhost:5173");
        for o in [
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
        ] {
            assert!(is_app_origin(o, None), "{o}");
        }
        assert!(is_app_origin("http://localhost:5173", Some(&dev)));
        assert!(!is_app_origin("http://localhost:5173", None));
        for o in [
            "null",
            "",
            "http://localhost",
            "http://localhost:5174",
            "https://evil.example",
            "tauri://localhost.evil",
            "http://tauri.localhost.evil.com",
            "chathistory://localhost",
        ] {
            assert!(!is_app_origin(o, Some(&dev)), "{o}");
        }
    }

    #[test]
    fn app_urls() {
        let dev = url("http://localhost:5173/");
        assert!(is_app_url(&url("tauri://localhost/index.html#/chat"), None));
        assert!(is_app_url(&url("http://tauri.localhost/"), None));
        assert!(is_app_url(&url("http://localhost:5173/#/"), Some(&dev)));
        assert!(!is_app_url(&url("http://localhost:5173/#/"), None));
        assert!(!is_app_url(&url("https://example.com/"), Some(&dev)));
        assert!(!is_app_url(&url("about:blank"), None));
    }

    #[test]
    fn navigation_decisions() {
        use Navigation::*;
        let dev = url("http://localhost:5173");
        let cases = [
            ("tauri://localhost/index.html#/setting", Allow),
            ("http://localhost:5173/#/chat", Allow),
            ("about:blank", Allow),
            ("about:srcdoc", Allow),
            ("htmlpreview://localhost/preview", Allow),
            ("http://htmlpreview.localhost/preview", Allow),
            ("https://2-19-8-abc-sandpack.codesandbox.io/", Allow),
            ("https://embed.diagrams.net/?embed=1", Allow),
            (
                "https://github.com/aws-samples/bedrock-engineer",
                OpenExternally,
            ),
            ("http://example.com/", OpenExternally),
            ("mailto:someone@example.com", OpenExternally),
            ("https://codesandbox.io.evil.com/", OpenExternally),
            ("https://notembed.diagrams.net/", OpenExternally),
            ("http://localhost:3000/", OpenExternally),
            ("file:///etc/passwd", Block),
            ("chathistory://localhost/session_1", Block),
            ("data:text/html,<script>1</script>", Block),
            ("javascript:alert(1)", Block),
        ];
        for (u, expected) in cases {
            assert_eq!(navigation(&url(u), Some(&dev)), expected, "{u}");
        }
        // Without a dev server, localhost is just another site.
        assert_eq!(
            navigation(&url("http://localhost:5173/"), None),
            OpenExternally
        );
    }

    #[test]
    fn camera_permission() {
        let app = url("tauri://localhost/index.html");
        let preview = url("tauri://localhost/camera-preview.html?deviceId=x");
        let remote = url("https://example.com/");
        assert!(allow_permission(
            &PermissionKind::Camera,
            "main",
            Some(&app),
            None
        ));
        assert!(allow_permission(
            &PermissionKind::Camera,
            "camera-preview-1",
            Some(&preview),
            None
        ));
        // Wrong window, wrong origin, unknown URL, other permissions.
        assert!(!allow_permission(
            &PermissionKind::Camera,
            "task-history",
            Some(&app),
            None
        ));
        assert!(!allow_permission(
            &PermissionKind::Camera,
            "pdf-export-1",
            Some(&app),
            None
        ));
        assert!(!allow_permission(
            &PermissionKind::Camera,
            "main",
            Some(&remote),
            None
        ));
        assert!(!allow_permission(
            &PermissionKind::Camera,
            "main",
            None,
            None
        ));
        assert!(!allow_permission(
            &PermissionKind::Microphone,
            "main",
            Some(&app),
            None
        ));
    }
}
