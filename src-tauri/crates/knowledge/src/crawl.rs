//! Website crawler for URL sources: breadth-first from the start URL, staying on the same
//! site (and by default under the start URL's directory), honoring robots.txt and
//! `<meta name="robots">`, seeded from sitemap.xml, with a polite delay between requests.

use crate::chunk::TextKind;
use crate::extract::{extract_bytes, Format};
use crate::html::{extract_links, robots_meta};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Sent with every request; robots.txt rules are matched against [`ROBOTS_AGENT`].
pub const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; BedrockEngineerKnowledgeBase/1.0; +https://github.com/aws-samples/bedrock-engineer)";
pub const ROBOTS_AGENT: &str = "BedrockEngineerKnowledgeBase";

/// Biggest page body read.
pub const MAX_PAGE_BYTES: usize = 5 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest `Crawl-delay` honored.
const MAX_ROBOTS_DELAY: Duration = Duration::from_secs(10);
const MAX_SITEMAPS: usize = 10;

#[derive(Debug, Clone)]
pub struct CrawlOptions {
    pub max_depth: u32,
    pub max_pages: u32,
    pub path_prefix_only: bool,
    pub delay: Duration,
    pub respect_robots: bool,
    pub use_sitemap: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrawledPage {
    pub url: String,
    pub title: String,
    pub text: String,
    pub kind: TextKind,
}

#[derive(Debug, Default)]
pub struct CrawlOutcome {
    pub pages: Vec<CrawledPage>,
    /// `(url, reason)` for pages that were found but not indexed.
    pub skipped: Vec<(String, String)>,
}

/// Progress callback: `(pages fetched, pages planned, current url)`.
pub type Progress<'a> = &'a (dyn Fn(usize, usize, &str) + Send + Sync);

const SKIP_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "tiff", "avif", "mp3", "mp4", "wav",
    "ogg", "webm", "mov", "avi", "mkv", "zip", "gz", "tgz", "bz2", "xz", "7z", "rar", "tar", "dmg",
    "exe", "msi", "pkg", "deb", "rpm", "appimage", "iso", "css", "js", "mjs", "map", "woff",
    "woff2", "ttf", "otf", "eot", "json", "xml", "rss", "atom", "csv", "xlsx", "xls", "docx",
    "doc", "pptx", "ppt", "wasm", "bin",
];

/// Dedup key: no fragment, no trailing slash (except the root).
pub fn normalize(u: &Url) -> String {
    let mut u = u.clone();
    u.set_fragment(None);
    let path = u.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        u.set_path(path.trim_end_matches('/'));
    }
    u.to_string()
}

/// Where links may lead: same scheme, host and port; under `prefix` when set.
#[derive(Debug, Clone)]
pub struct Scope {
    origin: url::Origin,
    prefix: Option<String>,
}

impl Scope {
    pub fn new(start: &Url, path_prefix_only: bool) -> Self {
        let prefix = path_prefix_only.then(|| {
            let p = start.path();
            match p.rfind('/') {
                Some(i) => p[..=i].to_string(),
                None => "/".to_string(),
            }
        });
        Self {
            origin: start.origin(),
            prefix,
        }
    }

    pub fn contains(&self, u: &Url) -> bool {
        if u.origin() != self.origin {
            return false;
        }
        if let Some(prefix) = &self.prefix {
            let path = u.path();
            // `/docs/` also admits `/docs` itself.
            if !(path.starts_with(prefix.as_str()) || format!("{path}/") == *prefix) {
                return false;
            }
        }
        let ext = u
            .path()
            .rsplit('/')
            .next()
            .and_then(|seg| seg.rsplit_once('.'))
            .map(|(_, e)| e.to_lowercase());
        !ext.is_some_and(|e| SKIP_EXTENSIONS.contains(&e.as_str()))
    }
}

struct Fetched {
    url: Url,
    content_type: String,
    body: Vec<u8>,
}

async fn fetch(client: &reqwest::Client, url: &Url) -> Result<Fetched, String> {
    let mut resp = client
        .get(url.clone())
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/pdf;q=0.9,text/plain;q=0.8,*/*;q=0.5",
        )
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let final_url = resp.url().clone();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_PAGE_BYTES {
            return Err(format!(
                "Page is larger than {} MB",
                MAX_PAGE_BYTES / (1024 * 1024)
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Fetched {
        url: final_url,
        content_type,
        body,
    })
}

/// The format of a response, from its content type (or a sniff of the body).
fn response_format(f: &Fetched) -> Option<Format> {
    let ct = f.content_type.split(';').next().unwrap_or_default().trim();
    match ct {
        "text/html" | "application/xhtml+xml" => Some(Format::Html),
        "application/pdf" => Some(Format::Pdf),
        "text/plain" => Some(Format::Text),
        "text/markdown" | "text/x-markdown" => Some(Format::Markdown),
        "" | "application/octet-stream" => {
            if f.body.starts_with(b"%PDF") {
                Some(Format::Pdf)
            } else {
                let head =
                    String::from_utf8_lossy(&f.body[..f.body.len().min(1024)]).to_lowercase();
                (head.contains("<html") || head.contains("<!doctype html")).then_some(Format::Html)
            }
        }
        _ => None,
    }
}

/// `<loc>` values of a sitemap, and whether it is a sitemap index.
fn parse_sitemap(xml: &str) -> (Vec<String>, bool) {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut locs = Vec::new();
    let mut in_loc = false;
    let mut is_index = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "loc" => in_loc = true,
                "sitemapindex" => is_index = true,
                _ => {}
            },
            Ok(Event::End(e)) if e.local_name().as_ref() == "loc" => in_loc = false,
            Ok(Event::Text(t)) if in_loc => {
                let s = t.xml10_content().trim().to_string();
                if !s.is_empty() {
                    locs.push(s);
                }
            }
            Ok(Event::CData(t)) if in_loc => locs.push(t.xml10_content().trim().to_string()),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (locs, is_index)
}

struct Robots(Option<texting_robots::Robot>);

impl Robots {
    fn allowed(&self, u: &Url) -> bool {
        self.0.as_ref().is_none_or(|r| r.allowed(u.as_str()))
    }
}

async fn load_robots(client: &reqwest::Client, start: &Url) -> (Robots, Vec<String>) {
    let Ok(robots_url) = start.join("/robots.txt") else {
        return (Robots(None), Vec::new());
    };
    match fetch(client, &robots_url).await {
        Ok(f) => match texting_robots::Robot::new(ROBOTS_AGENT, &f.body) {
            Ok(r) => {
                let sitemaps = r.sitemaps.clone();
                (Robots(Some(r)), sitemaps)
            }
            Err(_) => (Robots(None), Vec::new()),
        },
        // No robots.txt (or unreachable): everything is allowed.
        Err(_) => (Robots(None), Vec::new()),
    }
}

async fn sitemap_urls(
    client: &reqwest::Client,
    start: &Url,
    declared: Vec<String>,
    scope: &Scope,
    limit: usize,
    cancel: &CancellationToken,
) -> Vec<Url> {
    let mut queue: VecDeque<Url> = declared
        .iter()
        .filter_map(|s| Url::parse(s).ok())
        .filter(|u| u.origin() == start.origin())
        .collect();
    if queue.is_empty() {
        if let Ok(u) = start.join("/sitemap.xml") {
            queue.push_back(u);
        }
    }
    let mut out = Vec::new();
    let mut fetched = 0;
    while let Some(sm) = queue.pop_front() {
        if fetched >= MAX_SITEMAPS || out.len() >= limit || cancel.is_cancelled() {
            break;
        }
        fetched += 1;
        let Ok(f) = fetch(client, &sm).await else {
            continue;
        };
        let (locs, is_index) = parse_sitemap(&String::from_utf8_lossy(&f.body));
        for loc in locs {
            let Ok(u) = Url::parse(&loc) else { continue };
            if is_index {
                if u.origin() == start.origin() {
                    queue.push_back(u);
                }
            } else if scope.contains(&u) {
                out.push(u);
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// Sleep for `d`, or return `false` early if cancelled.
async fn pause(d: Duration, cancel: &CancellationToken) -> bool {
    if d.is_zero() {
        return !cancel.is_cancelled();
    }
    tokio::select! {
        _ = tokio::time::sleep(d) => true,
        _ = cancel.cancelled() => false,
    }
}

/// Crawl from `start`. Fails only if the start page itself can't be fetched or read.
pub async fn crawl(
    client: &reqwest::Client,
    start: &str,
    opts: &CrawlOptions,
    cancel: &CancellationToken,
    progress: Progress<'_>,
) -> Result<CrawlOutcome, String> {
    let start = Url::parse(start.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
    if !matches!(start.scheme(), "http" | "https") {
        return Err("Only http and https URLs can be crawled".into());
    }
    let scope = Scope::new(&start, opts.path_prefix_only);
    let max_pages = opts.max_pages.max(1) as usize;

    let (robots, declared_sitemaps) = if opts.respect_robots || opts.use_sitemap {
        load_robots(client, &start).await
    } else {
        (Robots(None), Vec::new())
    };
    let robots = if opts.respect_robots {
        robots
    } else {
        Robots(None)
    };
    let delay = robots
        .0
        .as_ref()
        .and_then(|r| r.delay)
        .map(|d| Duration::from_secs_f32(d.max(0.0)).min(MAX_ROBOTS_DELAY))
        .unwrap_or_default()
        .max(opts.delay);

    if !robots.allowed(&start) {
        return Err("robots.txt does not allow crawling this URL".into());
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(Url, u32)> = VecDeque::new();
    seen.insert(normalize(&start));
    queue.push_back((start.clone(), 0));

    if opts.use_sitemap && opts.max_depth > 0 && max_pages > 1 {
        for u in sitemap_urls(
            client,
            &start,
            declared_sitemaps,
            &scope,
            max_pages * 2,
            cancel,
        )
        .await
        {
            if seen.insert(normalize(&u)) {
                queue.push_back((u, 1));
            }
        }
    }

    let mut out = CrawlOutcome::default();
    let mut requests = 0usize;
    while let Some((url, depth)) = queue.pop_front() {
        if out.pages.len() >= max_pages {
            break;
        }
        if cancel.is_cancelled() {
            return Err("Cancelled".into());
        }
        if !robots.allowed(&url) {
            out.skipped
                .push((url.to_string(), "Disallowed by robots.txt".into()));
            continue;
        }
        if requests > 0 && !pause(delay, cancel).await {
            return Err("Cancelled".into());
        }
        requests += 1;
        progress(
            out.pages.len(),
            max_pages.min(out.pages.len() + queue.len() + 1),
            url.as_str(),
        );

        let is_start = requests == 1;
        let fetched = match fetch(client, &url).await {
            Ok(f) => f,
            Err(e) if is_start => return Err(format!("Cannot fetch {url}: {e}")),
            Err(e) => {
                out.skipped.push((url.to_string(), e));
                continue;
            }
        };
        if !scope.contains(&fetched.url) && !is_start {
            out.skipped.push((
                url.to_string(),
                format!("Redirected off-site to {}", fetched.url),
            ));
            continue;
        }
        // A redirect may land on a page already crawled.
        if normalize(&fetched.url) != normalize(&url)
            && !seen.insert(normalize(&fetched.url))
            && !is_start
        {
            continue;
        }
        let Some(format) = response_format(&fetched) else {
            let reason = format!("Unsupported content type: {}", fetched.content_type);
            if is_start {
                return Err(reason);
            }
            out.skipped.push((url.to_string(), reason));
            continue;
        };

        let (noindex, nofollow) = if format == Format::Html {
            robots_meta(&String::from_utf8_lossy(&fetched.body))
        } else {
            (false, false)
        };

        if format == Format::Html && depth < opts.max_depth && !nofollow {
            let html = String::from_utf8_lossy(&fetched.body);
            for link in extract_links(&html, &fetched.url) {
                if scope.contains(&link) && seen.insert(normalize(&link)) {
                    queue.push_back((link, depth + 1));
                }
            }
        }

        if noindex {
            out.skipped.push((
                fetched.url.to_string(),
                "Page asks not to be indexed".into(),
            ));
            continue;
        }
        let page_url = fetched.url.to_string();
        match extract_bytes(&fetched.body, format, Some(&page_url)) {
            Ok(e) if !e.text.trim().is_empty() => out.pages.push(CrawledPage {
                title: if e.title.is_empty() {
                    page_url.clone()
                } else {
                    e.title
                },
                url: normalize(&fetched.url),
                text: e.text,
                kind: e.kind,
            }),
            Ok(_) => out.skipped.push((page_url, "No text content".into())),
            Err(e) if is_start => return Err(e),
            Err(e) => out.skipped.push((page_url, e)),
        }
    }
    progress(out.pages.len(), out.pages.len(), "");
    if out.pages.is_empty() {
        return Err(match out.skipped.first() {
            Some((u, why)) => format!("No pages could be indexed ({u}: {why})"),
            None => "No pages could be indexed".into(),
        });
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub type Route = (String, u16, String, String);

    /// A tiny HTTP/1.1 server. Records request paths.
    pub struct TestServer {
        pub base: String,
        pub hits: Arc<Mutex<Vec<String>>>,
    }

    pub fn route(path: &str, status: u16, ct: &str, body: impl Into<String>) -> Route {
        (path.to_string(), status, ct.to_string(), body.into())
    }

    /// Serve the routes built from the server's base URL (`http://127.0.0.1:<port>`).
    pub async fn serve(routes: impl FnOnce(&str) -> Vec<Route>) -> TestServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Arc<HashMap<String, (u16, String, String)>> = Arc::new(
            routes(&base)
                .into_iter()
                .map(|(p, s, ct, b)| (p, (s, ct, b)))
                .collect(),
        );
        let hits = Arc::new(Mutex::new(Vec::new()));
        let h = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                let hits = h.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                    hits.lock().unwrap().push(path.clone());
                    let (status, ct, body) = routes.get(&path).cloned().unwrap_or((
                        404,
                        "text/plain".into(),
                        "not found".into(),
                    ));
                    let mut head = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                        body.len()
                    );
                    if status == 301 {
                        head.push_str(&format!("Location: {body}\r\n"));
                    } else {
                        head.push_str(&format!("Content-Type: {ct}\r\n"));
                    }
                    head.push_str("\r\n");
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(body.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        TestServer { base, hits }
    }

    pub fn page(title: &str, body: &str, links: &[&str]) -> String {
        let links: String = links
            .iter()
            .map(|l| format!(r#"<a href="{l}">{l}</a> "#))
            .collect();
        format!("<html><head><title>{title}</title></head><body><nav>{links}</nav><main><p>{body}</p></main></body></html>")
    }

    pub fn opts(max_depth: u32, max_pages: u32) -> CrawlOptions {
        CrawlOptions {
            max_depth,
            max_pages,
            path_prefix_only: true,
            delay: Duration::ZERO,
            respect_robots: true,
            use_sitemap: true,
        }
    }

    fn noop(_: usize, _: usize, _: &str) {}

    async fn run(srv: &TestServer, path: &str, o: CrawlOptions) -> Result<CrawlOutcome, String> {
        crawl(
            &reqwest::Client::new(),
            &format!("{}{path}", srv.base),
            &o,
            &CancellationToken::new(),
            &noop,
        )
        .await
    }

    #[test]
    fn scope_rules() {
        let start = Url::parse("https://d.example/docs/guide/intro.html").unwrap();
        let s = Scope::new(&start, true);
        let ok = |u: &str| s.contains(&Url::parse(u).unwrap());
        assert!(ok("https://d.example/docs/guide/other"));
        assert!(ok("https://d.example/docs/guide"));
        assert!(!ok("https://d.example/docs/api"));
        assert!(!ok("http://d.example/docs/guide/x"));
        assert!(!ok("https://other.example/docs/guide/x"));
        assert!(!ok("https://d.example/docs/guide/logo.png"));
        assert!(ok("https://d.example/docs/guide/manual.pdf"));
        let s = Scope::new(&start, false);
        assert!(s.contains(&Url::parse("https://d.example/blog").unwrap()));
    }

    #[test]
    fn normalize_dedups() {
        let n = |u: &str| normalize(&Url::parse(u).unwrap());
        assert_eq!(n("https://x.example/a/#top"), "https://x.example/a");
        assert_eq!(n("https://x.example/"), "https://x.example/");
        assert_eq!(n("https://X.example/a?b=1"), "https://x.example/a?b=1");
    }

    #[test]
    fn sitemap_parsing() {
        let (locs, idx) = parse_sitemap(
            "<urlset><url><loc> https://x/a </loc></url><url><loc>https://x/b</loc></url></urlset>",
        );
        assert_eq!(locs, vec!["https://x/a", "https://x/b"]);
        assert!(!idx);
        let (_, idx) = parse_sitemap(
            "<sitemapindex><sitemap><loc>https://x/s.xml</loc></sitemap></sitemapindex>",
        );
        assert!(idx);
    }

    #[tokio::test]
    async fn crawls_within_depth_scope_and_robots() {
        let srv = serve(|_| {
            vec![
                route(
                    "/robots.txt",
                    200,
                    "text/plain",
                    "User-agent: *\nDisallow: /docs/private\n",
                ),
                route(
                    "/docs/",
                    200,
                    "text/html",
                    page(
                        "Home",
                        "welcome home",
                        &[
                            "a",
                            "b#frag",
                            "private",
                            "/other",
                            "https://elsewhere.example/x",
                        ],
                    ),
                ),
                route(
                    "/docs/a",
                    200,
                    "text/html",
                    page("A", "page a text", &["c", "/docs/"]),
                ),
                route("/docs/b", 200, "text/html", page("B", "page b text", &[])),
                route(
                    "/docs/c",
                    200,
                    "text/html",
                    page("C", "page c text", &["d"]),
                ),
                route("/docs/d", 200, "text/html", page("D", "too deep", &[])),
                route("/docs/private", 200, "text/html", page("P", "secret", &[])),
            ]
        })
        .await;
        let out = run(&srv, "/docs/", opts(2, 50)).await.unwrap();
        let mut titles: Vec<&str> = out.pages.iter().map(|p| p.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, vec!["A", "B", "C", "Home"], "{:?}", out.skipped);
        assert!(out.pages.iter().any(|p| p.text.contains("page a text")));
        let hits = srv.hits.lock().unwrap().clone();
        assert!(!hits.contains(&"/docs/private".to_string()), "{hits:?}");
        assert!(!hits.contains(&"/other".to_string()), "{hits:?}");
        assert!(!hits.contains(&"/docs/d".to_string()), "{hits:?}");
        assert!(out
            .skipped
            .iter()
            .any(|(u, why)| u.ends_with("/docs/private") && why.contains("robots")));
    }

    #[tokio::test]
    async fn page_cap_and_depth_zero() {
        let srv = serve(|_| {
            vec![
                route(
                    "/docs/start",
                    200,
                    "text/html",
                    page("Start", "start page", &["l1", "l2"]),
                ),
                route("/docs/l1", 200, "text/html", page("L1", "link one", &[])),
                route("/docs/l2", 200, "text/html", page("L2", "link two", &[])),
            ]
        })
        .await;
        let out = run(&srv, "/docs/start", opts(1, 2)).await.unwrap();
        assert_eq!(out.pages.len(), 2, "page cap");
        assert_eq!(out.pages[0].title, "Start");
        let out = run(&srv, "/docs/start", opts(0, 10)).await.unwrap();
        assert_eq!(out.pages.len(), 1, "depth 0 is the start page only");
    }

    #[tokio::test]
    async fn sitemap_seeds_pages() {
        let srv = serve(|base| {
            vec![
                route(
                    "/sitemap.xml",
                    200,
                    "application/xml",
                    format!("<urlset><url><loc>{base}/docs/s1</loc></url><url><loc>{base}/outside</loc></url></urlset>"),
                ),
                route("/docs/start", 200, "text/html", page("Start", "start page", &[])),
                route("/docs/s1", 200, "text/plain", "sitemap page one"),
            ]
        })
        .await;
        let out = run(&srv, "/docs/start", opts(1, 10)).await.unwrap();
        let urls: Vec<String> = out.pages.iter().map(|p| p.url.clone()).collect();
        assert_eq!(
            urls,
            vec![
                format!("{}/docs/start", srv.base),
                format!("{}/docs/s1", srv.base)
            ]
        );
        assert!(!srv.hits.lock().unwrap().contains(&"/outside".to_string()));
    }

    #[tokio::test]
    async fn noindex_pages_are_followed_not_indexed() {
        let srv = serve(|_| {
            vec![
                route(
                    "/start",
                    200,
                    "text/html",
                    r#"<html><head><meta name="robots" content="noindex"></head><body><a href="/next">n</a></body></html>"#,
                ),
                route("/next", 200, "text/html", page("Next", "next page", &[])),
            ]
        })
        .await;
        let out = run(&srv, "/start", opts(1, 10)).await.unwrap();
        assert_eq!(out.pages.len(), 1);
        assert_eq!(out.pages[0].title, "Next");
    }

    #[tokio::test]
    async fn start_failure_and_cancel() {
        let srv = serve(|_| vec![route("/img", 200, "image/png", "x")]).await;
        let e = run(&srv, "/missing", opts(1, 5)).await.unwrap_err();
        assert!(e.contains("404"), "{e}");
        let e = run(&srv, "/img", opts(1, 5)).await.unwrap_err();
        assert!(e.contains("Unsupported content type"), "{e}");
        let token = CancellationToken::new();
        token.cancel();
        let client = reqwest::Client::new();
        let e = crawl(
            &client,
            &format!("{}/img", srv.base),
            &opts(1, 5),
            &token,
            &noop,
        )
        .await
        .unwrap_err();
        assert_eq!(e, "Cancelled");
        assert!(crawl(&client, "ftp://x/y", &opts(1, 5), &token, &noop)
            .await
            .is_err());
    }
}
