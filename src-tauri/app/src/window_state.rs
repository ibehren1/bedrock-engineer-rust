//! Reopening a window at the size and position it was last closed at.
//!
//! The geometry is kept in `window-state.json` next to `config.json` (one entry per window label),
//! written when the window closes and when the app is asked to exit. It is deliberately not in
//! `config.json`: it is incidental window state rather than a setting, and the renderer's settings
//! cache has no use for it.
//!
//! Saved sizes and positions are physical pixels, so a window keeps the size it had on the monitor
//! it was closed on. Before being applied they are checked against the monitors that exist now
//! ([`fit_to_monitors`]): a window that would open off-screen is moved into view, and one too large
//! for any monitor is not used at all. That case, like having nothing saved yet, leaves the caller
//! to size the window itself with [`crate::window_startup::fit_to_screen`].

use common::json_file::ConfFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Runtime, Window, WindowEvent};

/// File holding every remembered window, in the user data directory.
pub const FILE_NAME: &str = "window-state.json";

/// Window labels whose geometry is remembered. The camera preview and PDF export windows are
/// deliberately absent: they are positioned by the app and not meant to be moved.
pub const TRACKED: &[&str] = &["main"];

/// A window's outer position and inner size in physical pixels, as last seen neither maximized nor
/// full screen, plus whether it was maximized when it closed.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bounds {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub maximized: bool,
}

/// A monitor's usable area (excluding the menu bar / taskbar) in physical pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Work {
    pub pos: (i32, i32),
    pub size: (u32, u32),
}

/// Pixels of `outer` (a window's outer rect) that lie inside `work`.
fn overlap(outer: (i32, i32, u32, u32), work: &Work) -> u64 {
    let span = |a: i32, a_len: u32, b: i32, b_len: u32| -> u64 {
        let start = a.max(b);
        let end = (a.saturating_add_unsigned(a_len)).min(b.saturating_add_unsigned(b_len));
        u64::from(end.saturating_sub(start).max(0) as u32)
    };
    span(outer.0, outer.2, work.pos.0, work.size.0)
        * span(outer.1, outer.3, work.pos.1, work.size.1)
}

/// Kept free around a restored window, in physical pixels: a window asked to fill its work area
/// exactly is treated as zoomed by macOS, which then snaps it back to its previous frame, so
/// geometry that large is rejected rather than squeezed in.
const MARGIN: u32 = 8;

/// Assumed title bar height, in logical pixels, for the margin check. A window's chrome is the
/// difference between its outer and inner size, which is 0 until the window is first shown — and
/// [`Remembered`] restores it while it is still hidden.
const ASSUMED_TITLE_BAR: f64 = 28.0;

/// `bounds` as they can be used on the monitors that exist now: unchanged when they fit on one,
/// moved the least needed to bring the whole window into view when they don't, and `None` when the
/// window is too large for every monitor — the caller then opens at its default size instead. The
/// size is never changed, so no monitor's dimensions leak into the window's. With no monitors to
/// check, `bounds` are used as they are.
pub fn fit_to_monitors(bounds: Bounds, chrome: (u32, u32), works: &[Work]) -> Option<Bounds> {
    let outer = (
        bounds.x,
        bounds.y,
        bounds.width.saturating_add(chrome.0),
        bounds.height.saturating_add(chrome.1),
    );
    let Some(work) = works
        .iter()
        .max_by_key(|w| overlap(outer, w))
        .filter(|w| overlap(outer, w) > 0)
        .or_else(|| works.first())
    else {
        return Some(bounds);
    };
    if outer.2.saturating_add(MARGIN) > work.size.0 || outer.3.saturating_add(MARGIN) > work.size.1
    {
        return None;
    }
    let place = |pos: i32, outer: u32, work_pos: i32, work: u32| {
        pos.clamp(work_pos, work_pos.saturating_add_unsigned(work - outer))
    };
    Some(Bounds {
        x: place(bounds.x, outer.2, work.pos.0, work.size.0),
        y: place(bounds.y, outer.3, work.pos.1, work.size.1),
        ..bounds
    })
}

/// The remembered geometry of every [`TRACKED`] window.
///
/// A window's geometry is recorded in memory as it moves and resizes, and written to disk when it
/// closes: at that point — and on an app quit, where the window may already be gone — the geometry
/// can no longer be read from the window itself. While a window is maximized or full screen its
/// recorded bounds are left alone, so they still describe the window un-maximized, which is what
/// un-maximizing it after the next launch restores.
pub struct Remembered {
    file: ConfFile,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Current geometry per window label.
    live: HashMap<String, Bounds>,
    /// What [`Remembered::persist`] last wrote, so an unchanged window writes no file.
    written: HashMap<String, Bounds>,
}

impl Remembered {
    /// Reads `window-state.json` from `user_data`. A missing or unreadable file (logged by
    /// [`ConfFile`]) simply means nothing is remembered yet; so does an entry that doesn't parse.
    pub fn open(user_data: &Path) -> Self {
        let file = ConfFile::new(user_data.join(FILE_NAME));
        let live: HashMap<String, Bounds> = file
            .read()
            .into_iter()
            .filter(|(label, _)| TRACKED.contains(&label.as_str()))
            .filter_map(|(label, value)| match serde_json::from_value(value) {
                Ok(bounds) => Some((label, bounds)),
                Err(e) => {
                    tracing::warn!(window = %label, error = %e, "Ignoring unreadable saved window state");
                    None
                }
            })
            .collect();
        let state = State {
            written: live.clone(),
            live,
        };
        Self {
            file,
            state: Mutex::new(state),
        }
    }

    /// The geometry remembered for `label`.
    pub fn bounds(&self, label: &str) -> Option<Bounds> {
        self.state.lock().ok()?.live.get(label).copied()
    }

    /// Replaces the geometry remembered for `label` in memory (written by [`Self::persist`]).
    pub fn set_bounds(&self, label: &str, bounds: Bounds) {
        match self.state.lock() {
            Ok(mut state) => {
                state.live.insert(label.to_string(), bounds);
            }
            Err(e) => {
                tracing::warn!(window = label, error = %e, "Window state cache lock poisoned")
            }
        }
    }

    /// Records `window`'s geometry in memory. Maximized or full screen, only the maximized flag is
    /// updated; minimized or sized to nothing, nothing is. Best effort: failures are logged.
    pub fn record<R: Runtime>(&self, window: &Window<R>) {
        let label = window.label();
        if !TRACKED.contains(&label) {
            return;
        }
        match self.current_bounds(window) {
            Ok(Some(bounds)) => self.set_bounds(label, bounds),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(window = label, error = %e, "Failed to read the window geometry");
            }
        }
    }

    /// `window`'s geometry to remember, or `None` when it should be left as it is.
    fn current_bounds<R: Runtime>(&self, window: &Window<R>) -> tauri::Result<Option<Bounds>> {
        let previous = self.bounds(window.label());
        let zoomed = window.is_maximized()? || window.is_fullscreen()?;
        if zoomed {
            return Ok(previous.map(|previous| Bounds {
                maximized: true,
                ..previous
            }));
        }
        let size = window.inner_size()?;
        if size.width == 0 || size.height == 0 || window.is_minimized()? {
            return Ok(None);
        }
        let position = window.outer_position()?;
        Ok(Some(Bounds {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
            maximized: false,
        }))
    }

    /// Writes the recorded geometry to disk, keeping any other keys the file has. Does nothing when
    /// nothing changed since the last write. Best effort: failures are logged.
    pub fn persist(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.live == state.written {
            return;
        }
        let entries: Vec<(String, serde_json::Value)> = state
            .live
            .iter()
            .filter_map(|(label, bounds)| {
                serde_json::to_value(bounds)
                    .ok()
                    .map(|value| (label.clone(), value))
            })
            .collect();
        let result = self.file.update(|map| {
            for (label, value) in entries {
                map.insert(label, value);
            }
        });
        match result {
            Ok(()) => state.written = state.live.clone(),
            Err(e) => {
                tracing::warn!(path = %self.file.path().display(), error = %e, "Failed to save the window position");
            }
        }
    }
}

/// Resize and move `window` (still hidden) to `bounds`, if they suit the monitors connected now.
/// Returns whether they were applied; `false` means the caller should size the window itself.
/// Best effort: failures are logged and treated as not applied.
pub fn restore<R: Runtime>(window: &Window<R>, bounds: Bounds) -> bool {
    let result = (|| -> tauri::Result<bool> {
        let (inner, outer) = (window.inner_size()?, window.outer_size()?);
        let chrome = (
            outer.width.saturating_sub(inner.width),
            match outer.height.saturating_sub(inner.height) {
                0 => (ASSUMED_TITLE_BAR * window.scale_factor()?).round() as u32,
                measured => measured,
            },
        );
        let works: Vec<Work> = window
            .available_monitors()?
            .iter()
            .map(|m| {
                let area = m.work_area();
                Work {
                    pos: (area.position.x, area.position.y),
                    size: (area.size.width, area.size.height),
                }
            })
            .collect();
        let Some(fitted) = fit_to_monitors(bounds, chrome, &works) else {
            tracing::info!(
                window = window.label(),
                ?bounds,
                "Last window size does not fit this screen; opening at the default size"
            );
            return Ok(false);
        };
        tracing::debug!(
            window = window.label(),
            ?bounds,
            ?fitted,
            "Restoring the window's last size and position"
        );
        window.set_size(PhysicalSize::new(fitted.width, fitted.height))?;
        window.set_position(PhysicalPosition::new(fitted.x, fitted.y))?;
        if fitted.maximized {
            window.maximize()?;
        }
        Ok(true)
    })();
    match result {
        Ok(applied) => applied,
        Err(e) => {
            tracing::warn!(window = window.label(), error = %e, "Failed to restore the window position");
            false
        }
    }
}

/// Window events: follow the window's geometry, and write it when the window goes away.
///
/// Both close events are handled because neither covers every exit: closing a window emits
/// `CloseRequested`, while quitting the app (Cmd+Q, the menu, an Apple event) only destroys its
/// windows — by which point the geometry has to come from what the move and resize events recorded.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    let Some(remembered) = window.try_state::<Remembered>() else {
        return;
    };
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => remembered.record(window),
        WindowEvent::CloseRequested { .. } => {
            remembered.record(window);
            remembered.persist();
        }
        WindowEvent::Destroyed => remembered.persist(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME: (u32, u32) = (0, 28);

    fn bounds(width: u32, height: u32, x: i32, y: i32) -> Bounds {
        Bounds {
            width,
            height,
            x,
            y,
            maximized: false,
        }
    }

    /// 2560x1440 external display, 25px menu bar, as the primary monitor.
    fn desk() -> Work {
        Work {
            pos: (0, 25),
            size: (2560, 1415),
        }
    }

    /// 1440x900 laptop screen.
    fn laptop() -> Work {
        Work {
            pos: (0, 25),
            size: (1440, 875),
        }
    }

    #[test]
    fn keeps_a_window_that_still_fits() {
        let saved = bounds(1800, 1200, 300, 120);
        assert_eq!(fit_to_monitors(saved, CHROME, &[desk()]), Some(saved));
    }

    #[test]
    fn rejects_a_window_saved_on_a_bigger_monitor() {
        assert_eq!(
            fit_to_monitors(bounds(2400, 1380, 0, 25), CHROME, &[laptop()]),
            None
        );
    }

    /// A window as tall as the work area is rejected: macOS treats one that fills its screen as
    /// zoomed and snaps it back, and the title bar is not measurable before the window is shown.
    #[test]
    fn rejects_a_window_that_would_fill_the_screen() {
        assert_eq!(
            fit_to_monitors(bounds(1440, 847, 0, 25), CHROME, &[laptop()]),
            None
        );
    }

    #[test]
    fn moves_a_window_hanging_off_the_edge_back_inside() {
        assert_eq!(
            fit_to_monitors(bounds(1200, 800, 1300, 600), CHROME, &[laptop()]),
            Some(bounds(1200, 800, 240, 72))
        );
    }

    #[test]
    fn falls_back_to_the_primary_monitor_when_the_saved_one_is_gone() {
        // Saved on a second display to the right of the primary one, now disconnected.
        assert_eq!(
            fit_to_monitors(bounds(1200, 800, 2700, 100), CHROME, &[laptop()]),
            Some(bounds(1200, 800, 240, 72))
        );
    }

    #[test]
    fn prefers_the_monitor_the_window_mostly_covers() {
        let second = Work {
            pos: (2560, 0),
            size: (1920, 1080),
        };
        // Straddling both, mostly on the second: brought fully onto the second rather than
        // jumping to the primary.
        assert_eq!(
            fit_to_monitors(bounds(1000, 700, 2500, 100), CHROME, &[desk(), second]),
            Some(bounds(1000, 700, 2560, 100))
        );
    }

    #[test]
    fn keeps_the_maximized_flag() {
        let saved = Bounds {
            maximized: true,
            ..bounds(1800, 1200, 300, 120)
        };
        assert_eq!(fit_to_monitors(saved, CHROME, &[desk()]), Some(saved));
    }

    #[test]
    fn without_monitors_nothing_is_changed() {
        let saved = bounds(1200, 800, -4000, -4000);
        assert_eq!(fit_to_monitors(saved, CHROME, &[]), Some(saved));
    }

    #[test]
    fn geometry_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let saved = Bounds {
            maximized: true,
            ..bounds(1500, 900, 40, 60)
        };
        let remembered = Remembered::open(dir.path());
        assert_eq!(remembered.bounds("main"), None);
        remembered.set_bounds("main", saved);
        remembered.persist();
        assert_eq!(Remembered::open(dir.path()).bounds("main"), Some(saved));
    }

    #[test]
    fn untracked_and_unreadable_entries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"main":{"nonsense":true},"camera-preview":{"width":1,"height":1,"x":0,"y":0}}"#,
        )
        .unwrap();
        let remembered = Remembered::open(dir.path());
        assert_eq!(remembered.bounds("main"), None);
        assert_eq!(remembered.bounds("camera-preview"), None);
    }

    /// Keys the app does not own are left alone, so a newer build's entries survive an older one.
    #[test]
    fn persist_keeps_other_keys_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, r#"{"future-window":{"width":1}}"#).unwrap();
        let remembered = Remembered::open(dir.path());
        remembered.set_bounds("main", bounds(1500, 900, 40, 60));
        remembered.persist();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["future-window"]["width"], 1);
        assert_eq!(written["main"]["width"], 1500);
    }

    #[test]
    fn bounds_without_a_maximized_flag_still_parse() {
        let parsed: Bounds =
            serde_json::from_str(r#"{"width":1200,"height":800,"x":10,"y":20}"#).unwrap();
        assert_eq!(parsed, bounds(1200, 800, 10, 20));
    }
}
