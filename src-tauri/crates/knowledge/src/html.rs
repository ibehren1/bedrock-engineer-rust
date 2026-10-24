//! HTML to text (readability-style main content as Markdown) and link extraction.

use dom_query::Document;
use dom_smoothie::{Config, Readability, TextMode};
use url::Url;

/// Main content of a page as Markdown, plus its title.
pub struct PageText {
    pub title: String,
    pub text: String,
}

/// Extract the readable content. Pages readability can't make sense of (link hubs, very short
/// pages) fall back to the whole body text.
pub fn html_to_text(html: &str, url: Option<&str>) -> PageText {
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };
    let readable = Readability::new(html, url, Some(cfg))
        .ok()
        .and_then(|mut r| r.parse().ok());
    if let Some(article) = readable {
        let text = article.text_content.to_string();
        if !text.trim().is_empty() {
            return PageText {
                title: article.title.trim().to_string(),
                text: tidy(&text),
            };
        }
    }
    let doc = Document::from(html);
    let title = doc.select("title").text().trim().to_string();
    doc.select("script, style, noscript, template, svg")
        .remove();
    let text = doc.select("body").text().to_string();
    let text = if text.trim().is_empty() {
        doc.root().text().to_string()
    } else {
        text
    };
    PageText {
        title,
        text: tidy(&text),
    }
}

/// Collapse runs of blank lines and trailing whitespace.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
            out.push('\n');
            continue;
        }
        blank = 0;
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}

/// Absolute `http(s)` links in the page, resolved against `<base href>` or the page URL.
pub fn extract_links(html: &str, page_url: &Url) -> Vec<Url> {
    let doc = Document::from(html);
    let base = doc
        .select("base[href]")
        .attr("href")
        .and_then(|h| page_url.join(&h).ok())
        .unwrap_or_else(|| page_url.clone());
    let mut out = Vec::new();
    for a in doc.select("a[href]").iter() {
        let Some(href) = a.attr("href") else { continue };
        let href = href.trim();
        if href.is_empty() || href.starts_with('#') {
            continue;
        }
        let rel = a.attr("rel").map(|r| r.to_lowercase()).unwrap_or_default();
        if rel.split_whitespace().any(|r| r == "nofollow") {
            continue;
        }
        if let Ok(u) = base.join(href) {
            if matches!(u.scheme(), "http" | "https") {
                out.push(u);
            }
        }
    }
    out
}

/// `<meta name="robots" content="nofollow">` / `noindex`.
pub fn robots_meta(html: &str) -> (bool, bool) {
    let doc = Document::from(html);
    let content = doc
        .select(r#"meta[name="robots" i]"#)
        .attr("content")
        .map(|c| c.to_lowercase())
        .unwrap_or_default();
    let has = |w: &str| content.split(',').any(|p| p.trim() == w);
    (
        has("noindex") || has("none"),
        has("nofollow") || has("none"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r##"<html><head><title>Guide</title></head><body>
        <nav><a href="/a">A</a> <a href="b.html#x">B</a> <a href="mailto:x@y">m</a>
        <a href="https://other.example/c" rel="nofollow">C</a> <a href="#top">top</a></nav>
        <article><h1>Install</h1><p>First you download the installer from the website and then
        you run it. This paragraph is long enough to look like real content to readability, which
        wants a few hundred characters before it believes a block is the main article text.</p>
        <h2>Linux</h2><p>On Linux use the AppImage. It runs on most distributions without any
        installation step; just mark it executable and start it from your file manager or shell.</p>
        </article><script>var x = 1;</script></body></html>"##;

    #[test]
    fn readable_text_as_markdown() {
        let p = html_to_text(PAGE, Some("https://docs.example/guide/intro.html"));
        assert_eq!(p.title, "Guide");
        assert!(p.text.contains("download the installer"), "{}", p.text);
        assert!(p.text.contains("Linux"), "{}", p.text);
        assert!(!p.text.contains("var x"), "{}", p.text);
    }

    #[test]
    fn fallback_to_body_text() {
        let p = html_to_text(
            "<html><body><p>tiny</p><script>no()</script></body></html>",
            None,
        );
        assert_eq!(p.text, "tiny");
    }

    #[test]
    fn links_resolved_and_filtered() {
        let base = Url::parse("https://docs.example/guide/intro.html").unwrap();
        let links: Vec<String> = extract_links(PAGE, &base)
            .into_iter()
            .map(|u| u.to_string())
            .collect();
        assert_eq!(
            links,
            vec![
                "https://docs.example/a",
                "https://docs.example/guide/b.html#x"
            ]
        );
    }

    #[test]
    fn meta_robots() {
        assert_eq!(
            robots_meta(r#"<meta name="ROBOTS" content="noindex, nofollow">"#),
            (true, true)
        );
        assert_eq!(robots_meta("<p>x</p>"), (false, false));
    }
}
