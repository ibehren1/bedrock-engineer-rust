//! HTML → PDF for `save-chat-to-pdf`, replacing Electron's `webContents.printToPDF`.
//!
//! Electron loaded the export HTML into an offscreen `BrowserWindow` and printed it with
//! Chromium's print pipeline. Tauri has no printToPDF, so this does the same with the system
//! webview's own print-to-file API, silently (no dialog):
//!
//! 1. the document is served from memory over the [`PROTOCOL`] URI scheme (an export can carry
//!    several megabytes of inlined images, too much for a `data:` URL on WebView2);
//! 2. a hidden webview window loads it; the page-load `Finished` event fires after the `load`
//!    event, i.e. once the inline images have decoded;
//! 3. the platform prints it to a temporary PDF file: `NSPrintOperation` on the `WKWebView`
//!    (macOS), `ICoreWebView2_7::PrintToPdf` (Windows, Chromium like Electron), or a WebKitGTK
//!    `PrintOperation` to the "Print to File" printer (Linux);
//! 4. the bytes are read back and the window is closed.
//!
//! Page setup matches the Electron call: US Letter, 1-inch margins, backgrounds printed, no
//! header or footer. The print stylesheet in `chatPdfExport.tsx` does the rest.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Runtime, UriSchemeContext, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

/// URI scheme serving the documents being printed.
pub const PROTOCOL: &str = "chatexport";

/// Page width and height of the PDF in inches (U.S. Letter, as the docx export).
const PAGE_SIZE_INCHES: (f64, f64) = (8.5, 11.0);
/// Page margins of the PDF, in inches (the docx export uses 1" as well).
const MARGIN_INCHES: f64 = 1.0;
/// How long the document may take to load, and to print.
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
const PRINT_TIMEOUT: Duration = Duration::from_secs(120);

/// Page setup handed to the platform printer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageSetup {
    pub width_in: f64,
    pub height_in: f64,
    pub margin_in: f64,
}

const PAGE: PageSetup = PageSetup {
    width_in: PAGE_SIZE_INCHES.0,
    height_in: PAGE_SIZE_INCHES.1,
    margin_in: MARGIN_INCHES,
};

/// Completion callback of a platform print: `Ok` once the PDF file is fully written.
pub(crate) type PrintDone = Box<dyn FnOnce(Result<(), String>) + Send>;

/// Documents currently being printed, by id.
static DOCUMENTS: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Removes the document (and the temp file) however the export ends.
struct Pending {
    id: String,
    pdf_path: PathBuf,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Ok(mut docs) = DOCUMENTS.lock() {
            docs.remove(&self.id);
        }
        let _ = std::fs::remove_file(&self.pdf_path);
    }
}

/// The [`PROTOCOL`] handler: `GET /<id>` answers with that document's HTML.
pub fn protocol<R: Runtime>(
    _ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let id = request.uri().path().trim_start_matches('/');
    let html = DOCUMENTS.lock().ok().and_then(|d| d.get(id).cloned());
    let (status, body) = match html {
        Some(html) => (StatusCode::OK, html.into_bytes()),
        None => (StatusCode::NOT_FOUND, Vec::new()),
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(body)
        .expect("valid response")
}

/// The URL of document `id`: `chatexport://localhost/<id>`, or
/// `http://chatexport.localhost/<id>` where custom schemes go through `http` (Windows, Android).
fn document_url(id: &str) -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{PROTOCOL}.localhost/{id}")
    } else {
        format!("{PROTOCOL}://localhost/{id}")
    }
}

/// Print a self-contained HTML document (inline images only, no network access needed) to PDF
/// bytes.
pub async fn render_html_to_pdf<R: Runtime>(
    app: &AppHandle<R>,
    html: String,
) -> Result<Vec<u8>, String> {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let id = format!("{}-{n}", std::process::id());
    let pending = Pending {
        pdf_path: std::env::temp_dir().join(format!("bedrock-engineer-chat-export-{id}.pdf")),
        id: id.clone(),
    };
    DOCUMENTS
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id.clone(), html);

    let url: tauri::Url = document_url(&id).parse().map_err(|e| format!("{e}"))?;
    let (loaded_tx, loaded_rx) = oneshot::channel::<()>();
    let loaded_tx = Mutex::new(Some(loaded_tx));
    // Letter at 96 DPI: the layout viewport the page would have on screen. Printing re-lays the
    // document out at the page's printable width, so this only matters until then.
    let (w, h) = (PAGE.width_in * 96.0, PAGE.height_in * 96.0);
    // The document needs no network, but on Windows every webview must share the main
    // window's proxy setting (see `webview_proxy`).
    let builder = WebviewWindowBuilder::new(
        app,
        format!("pdf-export-{n}"),
        WebviewUrl::CustomProtocol(url),
    );
    let window = crate::webview_proxy::configure(app, builder)
        .title("PDF export")
        .inner_size(w, h)
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .on_page_load(move |_window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                if let Some(tx) = loaded_tx.lock().ok().and_then(|mut t| t.take()) {
                    let _ = tx.send(());
                }
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    crate::webview_proxy::attach(&window);

    let result = async {
        tokio::time::timeout(LOAD_TIMEOUT, loaded_rx)
            .await
            .map_err(|_| "Timed out loading the document to print".to_string())?
            .map_err(|_| "The print window closed before the document loaded".to_string())?;

        let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
        let done: PrintDone = Box::new(move |r| {
            let _ = done_tx.send(r);
        });
        let out = pending.pdf_path.clone();
        window
            .with_webview(move |webview| print_to_file(webview, &out, done))
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(PRINT_TIMEOUT, done_rx)
            .await
            .map_err(|_| "Timed out printing the document".to_string())?
            .map_err(|_| "The print operation was dropped".to_string())??;

        std::fs::read(&pending.pdf_path).map_err(|e| e.to_string())
    }
    .await;

    let _ = window.destroy();
    drop(pending);
    result
}

/// Start printing `webview`'s document to the PDF file `out`; `done` is called once it is written.
/// Runs on the main thread (`with_webview`).
fn print_to_file(webview: tauri::webview::PlatformWebview, out: &Path, done: PrintDone) {
    #[cfg(target_os = "macos")]
    macos::print_to_file(webview, out, PAGE, done);
    #[cfg(windows)]
    windows::print_to_file(webview, out, PAGE, done);
    #[cfg(target_os = "linux")]
    linux::print_to_file(webview, out, PAGE, done);
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = (webview, out);
        done(Err("PDF export is not supported on this platform".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_are_served_by_id_and_removed_afterwards() {
        let pending = Pending {
            id: "t-1".into(),
            pdf_path: std::env::temp_dir().join("bedrock-engineer-pdf-test-missing.pdf"),
        };
        DOCUMENTS
            .lock()
            .unwrap()
            .insert("t-1".into(), "<p>hi</p>".into());
        assert_eq!(
            DOCUMENTS.lock().unwrap().get("t-1").map(String::as_str),
            Some("<p>hi</p>")
        );
        drop(pending);
        assert!(DOCUMENTS.lock().unwrap().get("t-1").is_none());
    }

    #[test]
    fn document_urls_use_the_platform_scheme_form() {
        let url = document_url("42-1");
        if cfg!(windows) {
            assert_eq!(url, "http://chatexport.localhost/42-1");
        } else {
            assert_eq!(url, "chatexport://localhost/42-1");
        }
    }
}
