//! `workspacefile://` — raw bytes of files under the project folder for the Files panel's PDF,
//! DOCX and image viewers: `workspacefile://localhost/<encodeURIComponent(path)>` (macOS /
//! Linux) or `http://workspacefile.localhost/<...>` (Windows), what `convertFileSrc(path,
//! 'workspacefile')` builds. The query (a `?v=` cache-buster) is ignored.
//!
//! File serving (path resolution, MIME types, ranges) is `workspace_files::serve`. This module
//! adds CORS: requests carrying an `Origin` must come from the app's pages, which get it echoed in
//! `Access-Control-Allow-Origin`; any other origin (e.g. the opaque `null` of the HTML preview
//! iframe) gets `403`. Requests without an `Origin` (`<img>` loads) are served without CORS
//! headers, so other origins can at most display, never read, a file. Responses are never HTML
//! (see `workspace_files::mime_for`) and carry a locked-down CSP.

use crate::state::{project_path, StoreMutex};
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder, Url};

pub const PROTOCOL: &str = "workspacefile";

pub fn protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let dev_url = crate::security::dev_url(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let root = project_path(&app.state::<StoreMutex>());
        responder.respond(respond(&request, root.as_deref(), dev_url.as_ref()));
    });
}

fn header_str(request: &Request<Vec<u8>>, name: header::HeaderName) -> Option<&str> {
    request.headers().get(name).and_then(|v| v.to_str().ok())
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Vec::new())
        .expect("valid response")
}

/// The response to `request` for the project folder `root` (`None`: none set).
pub(crate) fn respond(
    request: &Request<Vec<u8>>,
    root: Option<&std::path::Path>,
    dev_url: Option<&Url>,
) -> Response<Vec<u8>> {
    let origin = header_str(request, header::ORIGIN);
    if let Some(o) = origin {
        if !crate::security::is_app_origin(o, dev_url) {
            tracing::warn!(
                category = "workspace-files",
                origin = o,
                "Rejected workspace file request from a non-app origin"
            );
            return status(StatusCode::FORBIDDEN);
        }
    }
    let with_cors = |builder: tauri::http::response::Builder| match origin {
        Some(o) => builder
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, o)
            .header(header::VARY, "Origin")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Range, Content-Length, Accept-Ranges",
            ),
        None => builder,
    };
    if request.method() == Method::OPTIONS {
        // CORS preflight (pdf.js range requests).
        return with_cors(Response::builder().status(StatusCode::NO_CONTENT))
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
            .header(header::ACCESS_CONTROL_MAX_AGE, "600")
            .body(Vec::new())
            .expect("valid response");
    }
    let served = workspace_files::serve::serve(
        root,
        request.method().as_str(),
        request.uri().path(),
        header_str(request, header::RANGE),
    );
    let mut builder =
        with_cors(Response::builder().status(
            StatusCode::from_u16(served.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        ));
    for (name, value) in &served.headers {
        builder = builder.header(*name, value);
    }
    builder.body(served.body).expect("valid response")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, uri: &str, origin: Option<&str>) -> Request<Vec<u8>> {
        let mut b = Request::builder().method(method).uri(uri);
        if let Some(o) = origin {
            b = b.header(header::ORIGIN, o);
        }
        b.body(Vec::new()).unwrap()
    }

    fn get(r: &Response<Vec<u8>>, name: header::HeaderName) -> Option<&str> {
        r.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn serves_project_files_to_app_pages() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("docs")).unwrap();
        std::fs::write(d.path().join("docs/a b.png"), b"PNG").unwrap();
        let root = Some(d.path());

        // macOS / Linux form, with a cache-buster query.
        let r = respond(
            &request(
                "GET",
                "workspacefile://localhost/docs%2Fa%20b.png?v=123",
                Some("tauri://localhost"),
            ),
            root,
            None,
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.body(), b"PNG");
        assert_eq!(get(&r, header::CONTENT_TYPE), Some("image/png"));
        assert_eq!(get(&r, header::CACHE_CONTROL), Some("no-store"));
        assert_eq!(
            get(&r, header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some("tauri://localhost")
        );

        // Windows form; an <img> load without Origin: served, no CORS.
        let r = respond(
            &request(
                "GET",
                "http://workspacefile.localhost/docs%2Fa%20b.png",
                None,
            ),
            root,
            None,
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert!(get(&r, header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

        let missing = request("GET", "workspacefile://localhost/nope.pdf", None);
        assert_eq!(
            respond(&missing, root, None).status(),
            StatusCode::NOT_FOUND
        );
        let outside = request(
            "GET",
            "workspacefile://localhost/..%2F..%2Fetc%2Fhosts",
            None,
        );
        assert_eq!(
            respond(&outside, root, None).status(),
            StatusCode::FORBIDDEN
        );
        let no_root = request("GET", "workspacefile://localhost/docs%2Fa%20b.png", None);
        assert_eq!(
            respond(&no_root, None, None).status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn other_origins_are_refused() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.pdf"), b"%PDF").unwrap();
        for bad in ["null", "htmlpreview://localhost", "https://evil.example"] {
            let r = respond(
                &request("GET", "workspacefile://localhost/a.pdf", Some(bad)),
                Some(d.path()),
                None,
            );
            assert_eq!(r.status(), StatusCode::FORBIDDEN, "{bad}");
            assert!(r.body().is_empty());
            assert!(get(&r, header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        }
        let dev: Url = "http://localhost:5173".parse().unwrap();
        let r = respond(
            &request(
                "GET",
                "workspacefile://localhost/a.pdf",
                Some("http://localhost:5173"),
            ),
            Some(d.path()),
            Some(&dev),
        );
        assert_eq!(r.status(), StatusCode::OK);
    }

    #[test]
    fn preflight_and_ranges() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.pdf"), b"0123456789").unwrap();
        let pre = respond(
            &request(
                "OPTIONS",
                "workspacefile://localhost/a.pdf",
                Some("tauri://localhost"),
            ),
            Some(d.path()),
            None,
        );
        assert_eq!(pre.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            get(&pre, header::ACCESS_CONTROL_ALLOW_HEADERS),
            Some("Range")
        );
        let mut req = request(
            "GET",
            "workspacefile://localhost/a.pdf",
            Some("tauri://localhost"),
        );
        req.headers_mut()
            .insert(header::RANGE, "bytes=0-3".parse().unwrap());
        let r = respond(&req, Some(d.path()), None);
        assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(r.body(), b"0123");
        assert_eq!(get(&r, header::CONTENT_RANGE), Some("bytes 0-3/10"));
        assert!(get(&r, header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .unwrap()
            .contains("Content-Range"));
    }
}
