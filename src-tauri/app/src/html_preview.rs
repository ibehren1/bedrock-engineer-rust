//! `htmlpreview://` — the page the chat's ```html previews run in (`HtmlBlock.tsx`).
//!
//! A `srcdoc` iframe inherits the app's Content-Security-Policy, which (rightly) blocks the inline
//! and CDN scripts model-written pages use. So the preview iframe loads this scheme instead: a
//! separate origin with its own permissive policy, sandboxed without `allow-same-origin` (opaque
//! origin: no access to the app's DOM, storage or `__TAURI_INTERNALS__`, and IPC requests carry
//! `Origin: null`, which Tauri rejects). The page is a fixed shell; the renderer posts it the
//! HTML, and the shell replaces itself with it.

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Runtime, UriSchemeContext};

pub const PROTOCOL: &str = "htmlpreview";

/// The shell: announces `html-preview:ready` to the parent, then renders the first
/// `html-preview:render { html }` message the parent sends back (see `HtmlBlock.tsx`).
const SHELL: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8" /></head>
<body>
<script>
  (function () {
    function onMessage(event) {
      var data = event.data
      if (event.source !== window.parent || !data || data.type !== 'html-preview:render') return
      window.removeEventListener('message', onMessage)
      document.open()
      document.write(String(data.html))
      document.close()
    }
    window.addEventListener('message', onMessage)
    window.parent.postMessage({ type: 'html-preview:ready' }, '*')
  })()
</script>
</body>
</html>
"#;

/// The preview's own policy: model pages may load anything (they did under Electron), but the
/// document is always a sandboxed, opaque-origin one even if the iframe attribute were missing.
const PREVIEW_CSP: &str = "sandbox allow-scripts; default-src * data: blob: 'unsafe-inline' \
                           'unsafe-eval'";

pub fn protocol<R: Runtime>(
    _ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let (status, body) = if request.method() == tauri::http::Method::GET {
        (StatusCode::OK, SHELL.as_bytes().to_vec())
    } else {
        (StatusCode::METHOD_NOT_ALLOWED, Vec::new())
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, PREVIEW_CSP)
        .header(header::CACHE_CONTROL, "no-store")
        .body(body)
        .expect("valid response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_is_sandboxed_and_speaks_the_renderer_protocol() {
        assert!(PREVIEW_CSP.starts_with("sandbox allow-scripts;"));
        assert!(!PREVIEW_CSP.contains("allow-same-origin"));
        // HtmlBlock.tsx waits for `ready` and answers with `render { html }`.
        assert!(SHELL.contains("'html-preview:ready'"));
        assert!(SHELL.contains("'html-preview:render'"));
        assert!(SHELL.contains("event.source !== window.parent"));
    }
}
