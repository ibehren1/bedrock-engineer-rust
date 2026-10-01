//! [`ScreenSource`] over the `xcap` crate (Electron's `desktopCapturer`).

use super::screen::{ScreenSource, SourceInfo, SourceKind};
use image::RgbaImage;

/// Smallest window edge worth listing (menu-bar extras, invisible helper windows).
const MIN_WINDOW_EDGE: u32 = 40;

/// macOS system processes whose on-screen windows (Dock, widgets, notification banners) are
/// not app windows; Electron's `desktopCapturer` only listed normal (layer 0) windows.
#[cfg(target_os = "macos")]
const SYSTEM_OWNERS: &[&str] = &[
    "Dock",
    "Notification Center",
    "UserNotificationCenter",
    "Window Server",
    "Control Center",
    "SystemUIServer",
    "WindowManager",
    "Spotlight",
    "TextInputMenuAgent",
];

/// Screens and windows through `xcap`.
pub struct XcapSource;

fn err(e: xcap::XCapError) -> String {
    e.to_string()
}

#[cfg(target_os = "macos")]
mod mac {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    /// Screen Recording permission. When it is missing, ask once per process so macOS shows
    /// its prompt and lists the app under Privacy & Security > Screen Recording (preflight
    /// alone never does).
    pub fn has_permission() -> bool {
        static REQUESTED: std::sync::Once = std::sync::Once::new();
        // SAFETY: plain CoreGraphics calls without arguments.
        if unsafe { CGPreflightScreenCaptureAccess() } {
            return true;
        }
        let mut granted = false;
        REQUESTED.call_once(|| granted = unsafe { CGRequestScreenCaptureAccess() });
        granted
    }
}

impl ScreenSource for XcapSource {
    fn has_permission(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            mac::has_permission()
        }
        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    fn screens(&self) -> Result<Vec<SourceInfo>, String> {
        let mut monitors = xcap::Monitor::all().map_err(err)?;
        // Primary first (`sources[0]` was the primary display).
        monitors.sort_by_key(|m| !m.is_primary().unwrap_or(false));
        Ok(monitors
            .iter()
            .filter_map(|m| {
                let id = m.id().ok()?;
                Some(SourceInfo {
                    kind: SourceKind::Screen,
                    id: format!("screen:{id}:0"),
                    name: m
                        .friendly_name()
                        .or_else(|_| m.name())
                        .unwrap_or_else(|_| format!("Screen {id}")),
                    app_name: String::new(),
                })
            })
            .collect())
    }

    fn windows(&self) -> Result<Vec<SourceInfo>, String> {
        let windows = xcap::Window::all().map_err(err)?;
        Ok(windows
            .iter()
            .filter_map(|w| {
                if w.is_minimized().unwrap_or(false) {
                    return None;
                }
                if w.width().unwrap_or(0) < MIN_WINDOW_EDGE
                    || w.height().unwrap_or(0) < MIN_WINDOW_EDGE
                {
                    return None;
                }
                let id = w.id().ok()?;
                let title = w.title().unwrap_or_default();
                let app_name = w.app_name().unwrap_or_default();
                #[cfg(target_os = "macos")]
                if SYSTEM_OWNERS.contains(&app_name.as_str()) {
                    return None;
                }
                let name = if title.trim().is_empty() {
                    app_name.clone()
                } else {
                    title
                };
                if name.trim().is_empty() {
                    return None;
                }
                Some(SourceInfo {
                    kind: SourceKind::Window,
                    id: format!("window:{id}:0"),
                    name,
                    app_name,
                })
            })
            .collect())
    }

    fn capture(&self, source: &SourceInfo) -> Result<RgbaImage, String> {
        let id = parse_id(&source.id).ok_or_else(|| format!("Invalid source id: {}", source.id))?;
        match source.kind {
            SourceKind::Screen => xcap::Monitor::all()
                .map_err(err)?
                .into_iter()
                .find(|m| m.id().ok() == Some(id))
                .ok_or_else(|| format!("Screen not found: {}", source.name))?
                .capture_image()
                .map_err(err),
            SourceKind::Window => xcap::Window::all()
                .map_err(err)?
                .into_iter()
                .find(|w| w.id().ok() == Some(id))
                .ok_or_else(|| format!("Window not found: {}", source.name))?
                .capture_image()
                .map_err(err),
        }
    }
}

/// `screen:<id>:0` / `window:<id>:0` → `<id>`.
fn parse_id(id: &str) -> Option<u32> {
    id.split(':').nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids() {
        assert_eq!(parse_id("window:42:0"), Some(42));
        assert_eq!(parse_id("screen:1:0"), Some(1));
        assert_eq!(parse_id("window:x:0"), None);
        assert_eq!(parse_id("42"), None);
    }

    /// Real capture of the primary screen into a temp dir. Skips (passes) without the macOS
    /// Screen Recording permission, so it never prompts in CI.
    /// `cargo test -p tools native_capture_smoke -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn native_capture_smoke() {
        use super::super::screen::{ScreenCaptureOptions, ScreenService};
        #[cfg(target_os = "macos")]
        {
            #[link(name = "CoreGraphics", kind = "framework")]
            unsafe extern "C" {
                fn CGPreflightScreenCaptureAccess() -> bool;
            }
            if !unsafe { CGPreflightScreenCaptureAccess() } {
                eprintln!("skipped: no Screen Recording permission");
                return;
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let svc = ScreenService::new(std::sync::Arc::new(XcapSource), dir.path().to_path_buf());
        assert!(svc.check_permissions().has_permission);
        let r = svc.capture(&ScreenCaptureOptions::default()).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        assert!(r.metadata.width > 0 && r.metadata.width <= 1920);
        assert!(r.metadata.height > 0 && r.metadata.height <= 1080);
        assert!(r.metadata.file_size > 0);
        let windows = svc.list_available_windows();
        eprintln!(
            "{} windows: {:?}",
            windows.len(),
            windows.iter().map(|w| &w.name).collect::<Vec<_>>()
        );
    }
}
