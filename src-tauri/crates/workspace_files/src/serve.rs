//! The `workspacefile://` scheme's file serving, without the HTTP plumbing (the app turns a
//! [`Served`] into a Tauri response and adds the CORS headers).
//!
//! The renderer builds URLs with `convertFileSrc(relativePath, 'workspacefile')`:
//! `workspacefile://localhost/<encodeURIComponent(path)>` (macOS / Linux) or
//! `http://workspacefile.localhost/<...>` (Windows), maybe with a `?v=` cache-buster (the URI
//! path the app passes here has no query). Single `Range: bytes=` requests get `206` (pdf.js
//! loads large PDFs in chunks).

use crate::{canonical_root, mime_for, resolve_in_root, Error};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A response to send: status, headers (besides CORS) and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// Locks down whatever the response is rendered as: no script, no subresources, and a sandbox
/// should it ever be loaded as a document.
pub const RESPONSE_CSP: &str =
    "default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; sandbox";

fn plain(status: u16, message: &str) -> Served {
    Served {
        status,
        headers: vec![
            ("Content-Type", "text/plain; charset=utf-8".to_string()),
            ("Cache-Control", "no-store".to_string()),
        ],
        body: message.as_bytes().to_vec(),
    }
}

/// The root-relative (or absolute) file path from a request's URI path (`/<percent-encoded>`).
pub fn path_from_uri(uri_path: &str) -> Option<String> {
    let raw = uri_path.strip_prefix('/').unwrap_or(uri_path);
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()
        .map(|s| s.into_owned())
}

/// A parsed single `bytes=` range, clamped to `len`: `(start, end_inclusive)`. `Err(())` for a
/// range that can't be satisfied; `Ok(None)` for no / unsupported (multi-range) headers, which
/// get the whole file.
fn parse_range(header: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if spec.contains(',') {
        return Ok(None);
    }
    let (start, end) = spec.split_once('-').ok_or(())?;
    let (start, end) = (start.trim(), end.trim());
    let range = if start.is_empty() {
        let n: u64 = end.parse().map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        (len.saturating_sub(n), len.checked_sub(1).ok_or(())?)
    } else {
        let s: u64 = start.parse().map_err(|_| ())?;
        let e: u64 = if end.is_empty() {
            len.saturating_sub(1)
        } else {
            end.parse::<u64>()
                .map_err(|_| ())?
                .min(len.saturating_sub(1))
        };
        if s >= len || e < s {
            return Err(());
        }
        (s, e)
    };
    Ok(Some(range))
}

/// Serve `GET` / `HEAD` of `uri_path` under the project folder `root` (`None`: no project set).
/// `403` outside the root, `404` missing (or a folder), `416` for an unsatisfiable range.
pub fn serve(root: Option<&Path>, method: &str, uri_path: &str, range: Option<&str>) -> Served {
    let head = method.eq_ignore_ascii_case("HEAD");
    if !head && !method.eq_ignore_ascii_case("GET") {
        return plain(405, "Method not allowed");
    }
    let Some(root) = root else {
        return plain(404, "No project folder is set");
    };
    let Ok(root) = canonical_root(root) else {
        return plain(404, "Project folder not found");
    };
    let Some(rel) = path_from_uri(uri_path) else {
        return plain(400, "Bad path");
    };
    let abs = match resolve_in_root(&root, &rel) {
        Ok(p) => p,
        Err(Error::Outside) => return plain(403, "Path is outside the project folder"),
        Err(_) => return plain(404, "Not found"),
    };
    let Ok(mut file) = fs::File::open(&abs) else {
        return plain(404, "Not found");
    };
    let len = match file.metadata() {
        Ok(m) if m.is_file() => m.len(),
        _ => return plain(404, "Not found"),
    };
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut headers = vec![
        ("Content-Type", mime_for(&name).to_string()),
        ("Cache-Control", "no-store".to_string()),
        ("Accept-Ranges", "bytes".to_string()),
        ("X-Content-Type-Options", "nosniff".to_string()),
        ("Content-Security-Policy", RESPONSE_CSP.to_string()),
    ];
    let (status, start, count) = match parse_range(range, len) {
        Ok(None) => (200, 0, len),
        Ok(Some((s, e))) => {
            headers.push(("Content-Range", format!("bytes {s}-{e}/{len}")));
            (206, s, e - s + 1)
        }
        Err(()) => {
            let mut r = plain(416, "Range not satisfiable");
            r.headers.push(("Content-Range", format!("bytes */{len}")));
            return r;
        }
    };
    headers.push(("Content-Length", count.to_string()));
    let mut body = Vec::new();
    if !head {
        let read = file
            .seek(SeekFrom::Start(start))
            .and_then(|_| file.take(count).read_to_end(&mut body));
        if read.is_err() {
            return plain(500, "Read failed");
        }
    }
    Served {
        status,
        headers,
        body,
    }
}
