//! Windows: WebView2's `ICoreWebView2_7::PrintToPdf` — Chromium's print-to-PDF, the same
//! pipeline Electron's `printToPDF` used. Needs a WebView2 runtime ≥ 1.0.1020 (any Evergreen
//! runtime today).

use super::{PageSetup, PrintDone};
use std::path::Path;
use std::sync::Mutex;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Environment6, ICoreWebView2_7, COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
};
use webview2_com::PrintToPdfCompletedHandler;
use windows::core::{Interface, HSTRING};

pub(super) fn print_to_file(
    webview: tauri::webview::PlatformWebview,
    out: &Path,
    page: PageSetup,
    done: PrintDone,
) {
    // `done` must run exactly once: from the completion handler, or here if starting fails.
    let done = std::sync::Arc::new(Mutex::new(Some(done)));
    let finish = {
        let done = done.clone();
        move |result: Result<(), String>| {
            if let Some(done) = done.lock().ok().and_then(|mut d| d.take()) {
                done(result);
            }
        }
    };
    let on_complete = finish.clone();
    let started = unsafe {
        (|| -> windows::core::Result<()> {
            let core = webview
                .controller()
                .CoreWebView2()?
                .cast::<ICoreWebView2_7>()?;
            let env = webview.environment().cast::<ICoreWebView2Environment6>()?;
            let settings = env.CreatePrintSettings()?;
            settings.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)?;
            settings.SetPageWidth(page.width_in)?;
            settings.SetPageHeight(page.height_in)?;
            settings.SetMarginTop(page.margin_in)?;
            settings.SetMarginBottom(page.margin_in)?;
            settings.SetMarginLeft(page.margin_in)?;
            settings.SetMarginRight(page.margin_in)?;
            settings.SetShouldPrintBackgrounds(true)?;
            settings.SetShouldPrintHeaderAndFooter(false)?;
            let handler = PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
                on_complete(match hr {
                    Ok(()) if ok => Ok(()),
                    Ok(()) => Err("WebView2 could not print the document to PDF".into()),
                    Err(e) => Err(e.message()),
                });
                Ok(())
            }));
            core.PrintToPdf(&HSTRING::from(out.as_os_str()), &settings, &handler)
        })()
    };
    if let Err(e) = started {
        finish(Err(e.message()));
    }
}
