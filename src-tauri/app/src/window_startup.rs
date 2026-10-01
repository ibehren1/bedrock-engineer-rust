//! Opening app windows without a blank flash, at a size that fits the screen.
//!
//! Electron created the main window with `show: false` and showed it on `ready-to-show`, at
//! 1800x1340 (shrunk by the OS to fit smaller screens). Here a window is built hidden, fitted to
//! its monitor's work area and centered on it, and shown when its page has finished loading —
//! or after [`SHOW_FALLBACK`] if the page never reports it, so a window can't stay invisible.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow, WebviewWindowBuilder,
};

/// Longest a window stays hidden waiting for its page to load.
pub const SHOW_FALLBACK: Duration = Duration::from_secs(5);

/// The inner size (logical) to open at: `desired`, shrunk so the window — inner size plus the
/// `chrome` the OS adds (title bar, borders) — fits in `work`, but never below `min`.
pub fn fit_inner_size(
    desired: (f64, f64),
    min: (f64, f64),
    work: (f64, f64),
    chrome: (f64, f64),
) -> (f64, f64) {
    let fit = |d: f64, m: f64, w: f64, c: f64| d.min(w - c).max(m);
    (
        fit(desired.0, min.0, work.0, chrome.0),
        fit(desired.1, min.1, work.1, chrome.1),
    )
}

/// The top-left position (physical) that centers a window of `outer` size in the work area at
/// `work_pos` / `work_size`; a window larger than the work area is pinned to its top-left.
pub fn centered_position(
    work_pos: (i32, i32),
    work_size: (u32, u32),
    outer: (u32, u32),
) -> (i32, i32) {
    let offset = |w: u32, o: u32| i32::try_from(w.saturating_sub(o) / 2).unwrap_or(0);
    (
        work_pos.0 + offset(work_size.0, outer.0),
        work_pos.1 + offset(work_size.1, outer.1),
    )
}

/// Resize `window` (still hidden) to `desired` / `min` (logical inner sizes) fitted to the work
/// area of the monitor it is on, and center it there. Best effort: failures are logged.
pub fn fit_to_screen<R: Runtime>(window: &WebviewWindow<R>, desired: (f64, f64), min: (f64, f64)) {
    let result = (|| -> tauri::Result<()> {
        let monitor = match window.current_monitor()? {
            Some(m) => Some(m),
            None => window.primary_monitor()?,
        };
        let Some(monitor) = monitor else {
            return Ok(());
        };
        let scale = monitor.scale_factor();
        let (inner, outer) = (window.inner_size()?, window.outer_size()?);
        let chrome_phys = (
            outer.width.saturating_sub(inner.width),
            outer.height.saturating_sub(inner.height),
        );
        let work = monitor.work_area();
        let (w, h) = fit_inner_size(
            desired,
            min,
            (
                work.size.width as f64 / scale,
                work.size.height as f64 / scale,
            ),
            (chrome_phys.0 as f64 / scale, chrome_phys.1 as f64 / scale),
        );
        let target = PhysicalSize::new((w * scale).round() as u32, (h * scale).round() as u32);
        if target != inner {
            window.set_size(target)?;
        }
        let (x, y) = centered_position(
            (work.position.x, work.position.y),
            (work.size.width, work.size.height),
            (target.width + chrome_phys.0, target.height + chrome_phys.1),
        );
        window.set_position(PhysicalPosition::new(x, y))
    })();
    if let Err(e) = result {
        tracing::warn!(window = window.label(), error = %e, "Failed to fit window to the screen");
    }
}

/// Shows a window once, when its page first finishes loading or after [`SHOW_FALLBACK`].
#[derive(Clone, Default)]
pub struct ShowWhenLoaded(Arc<AtomicBool>);

impl ShowWhenLoaded {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the window hidden and show it on the first page-load-finished event.
    pub fn attach<'a, R: Runtime, M: Manager<R>>(
        &self,
        builder: WebviewWindowBuilder<'a, R, M>,
    ) -> WebviewWindowBuilder<'a, R, M> {
        let this = self.clone();
        builder.visible(false).on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                this.show(&window, "page loaded");
            }
        })
    }

    /// Start the [`SHOW_FALLBACK`] timer for the built `window`.
    pub fn arm_fallback<R: Runtime>(&self, window: &WebviewWindow<R>) {
        let (this, window) = (self.clone(), window.clone());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SHOW_FALLBACK).await;
            if !this.0.load(Ordering::SeqCst) {
                tracing::warn!(
                    window = window.label(),
                    timeout_secs = SHOW_FALLBACK.as_secs(),
                    "Window page did not finish loading in time; showing the window anyway"
                );
            }
            this.show(&window, "fallback timeout");
        });
    }

    /// Show and focus `window` unless it was already shown (by a page load or the fallback).
    /// Later page loads (reloads, navigations) don't touch the window again.
    fn show<R: Runtime>(&self, window: &WebviewWindow<R>, reason: &str) {
        if self.0.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::debug!(window = window.label(), reason, "Showing window");
        if let Err(e) = window.show() {
            tracing::warn!(window = window.label(), error = %e, "Failed to show window");
        }
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_desired_size_when_it_fits() {
        assert_eq!(
            fit_inner_size(
                (1800.0, 1340.0),
                (640.0, 416.0),
                (2560.0, 1415.0),
                (0.0, 28.0)
            ),
            (1800.0, 1340.0)
        );
    }

    #[test]
    fn shrinks_to_the_work_area_minus_window_chrome() {
        // 1440x900 MacBook Air: 1440x875 below the menu bar, 28 pt title bar.
        assert_eq!(
            fit_inner_size(
                (1800.0, 1340.0),
                (640.0, 416.0),
                (1440.0, 875.0),
                (0.0, 28.0)
            ),
            (1440.0, 847.0)
        );
    }

    #[test]
    fn never_goes_below_the_minimum_size() {
        assert_eq!(
            fit_inner_size(
                (1800.0, 1340.0),
                (640.0, 416.0),
                (600.0, 400.0),
                (2.0, 30.0)
            ),
            (640.0, 416.0)
        );
    }

    #[test]
    fn centers_in_the_work_area() {
        assert_eq!(
            centered_position((0, 50), (2000, 1000), (1000, 600)),
            (500, 250)
        );
        // Second monitor to the left of the primary one.
        assert_eq!(
            centered_position((-1920, 0), (1920, 1080), (1920, 1080)),
            (-1920, 0)
        );
        // Larger than the work area: pinned to its top-left corner.
        assert_eq!(centered_position((0, 25), (800, 600), (1000, 700)), (0, 25));
    }
}
