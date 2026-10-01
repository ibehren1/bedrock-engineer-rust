//! Port of `src/preload/tools/handlers/web/FetchWebsiteTool.ts`.

use super::http::{fetch_website, FetchOptions};
use super::save::{save_website_content, SaveFormat, SaveResult};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js;
use crate::util::line_range::{
    filter_by_line_range, get_line_range_info, validate_line_range, LineRange,
};
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::LazyLock;

const NAME: &str = "fetchWebsite";
const DESCRIPTION: &str = "Fetch content from a specified URL with line range filtering support. If the cleaning option is true, extracts plain text content from HTML by removing markup and unnecessary elements. Default is false.";
const DEFAULT_MAX_TOKENS_LIMIT: f64 = 50000.0;

/// `FetchWebsiteTool`.
pub struct FetchWebsiteTool;

static SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script\b.*?</script>").expect("regex"));
static STYLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style\b.*?</style>").expect("regex"));
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("regex"));
static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("regex"));

/// `extractMainContent`: strip `<script>`/`<style>` blocks and tags, decode the basic
/// entities, collapse whitespace.
///
/// The TS script/style regexes (`<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>`) use a
/// lookahead; the lazy `<script\b.*?</script>` removes the same spans.
pub fn extract_main_content(html: &str) -> String {
    let content = SCRIPT.replace_all(html, "");
    let content = STYLE.replace_all(&content, "");
    let content = TAG.replace_all(&content, " ");
    let content = content
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    WS.replace_all(&content, " ").trim().to_string()
}

/// `smartTruncate`.
fn smart_truncate(content: &str, start: usize, end: usize, from_end: bool) -> String {
    let raw = js::slice(content, start, end);
    let len = js::len(raw) as f64;
    if from_end {
        let last_nl = raw.rfind('\n').map(|b| js::utf16_index(raw, b));
        let last_sp = raw.rfind(' ').map(|b| js::utf16_index(raw, b));
        if let Some(n) = last_nl.filter(|n| *n as f64 > len * 0.8) {
            return js::slice(raw, 0, n).to_string();
        }
        if let Some(n) = last_sp.filter(|n| *n as f64 > len * 0.8) {
            return js::slice(raw, 0, n).to_string();
        }
    } else {
        let first_nl = raw.find('\n').map(|b| js::utf16_index(raw, b));
        let first_sp = raw.find(' ').map(|b| js::utf16_index(raw, b));
        let total = js::len(raw);
        if let Some(n) = first_nl.filter(|n| (*n as f64) < len * 0.2) {
            return js::slice(raw, n + 1, total).to_string();
        }
        if let Some(n) = first_sp.filter(|n| (*n as f64) < len * 0.2) {
            return js::slice(raw, n + 1, total).to_string();
        }
    }
    raw.to_string()
}

/// `truncateContentMiddleOut`: keep the first and last 40% of the character budget
/// (`maxTokensLimit / 0.8`), with a notice in between.
pub fn truncate_content_middle_out(content: &str, max_tokens_limit: f64) -> String {
    let max_length = (max_tokens_limit / 0.8).floor() as usize;
    let len = js::len(content);
    if len <= max_length {
        return content.to_string();
    }
    let front_len = (max_length as f64 * 0.4).floor() as usize;
    let back_len = (max_length as f64 * 0.4).floor() as usize;
    let front = smart_truncate(content, 0, front_len, true);
    let back = smart_truncate(content, len - back_len, len, false);
    let omitted = len - front_len - back_len;
    format!(
        "{front}\n\n[Content truncated... ({} characters omitted)]\n\n{back}",
        js::to_locale_string(omitted)
    )
}

#[async_trait]
impl Tool for FetchWebsiteTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Web
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to fetch content from" },
                    "options": {
                        "type": "object",
                        "description": "Optional request configurations",
                        "properties": {
                            "method": {
                                "type": "string",
                                "description": "HTTP method (GET, POST, etc.)",
                                "enum": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
                            },
                            "headers": {
                                "type": "object",
                                "description": "Request headers",
                                "additionalProperties": { "type": "string" }
                            },
                            "body": { "type": "string", "description": "Request body (for POST, PUT, etc.)" },
                            "cleaning": {
                                "type": "boolean",
                                "description": "Optional. If true, extracts plain text content from HTML by removing markup and unnecessary elements. Default is false."
                            },
                            "lines": {
                                "type": "object",
                                "description": "Line range to display from the fetched content",
                                "properties": {
                                    "from": { "type": "number", "description": "Starting line number (1-based, inclusive)" },
                                    "to": { "type": "number", "description": "Ending line number (1-based, inclusive)" }
                                }
                            },
                            "saveToFile": {
                                "type": "object",
                                "description": "Save the fetched content to a file",
                                "properties": {
                                    "filename": { "type": "string", "description": "Custom filename (optional, auto-generated if not provided)" },
                                    "directory": { "type": "string", "description": "Directory to save the file (optional, uses project downloads directory if not provided)" },
                                    "format": { "type": "string", "description": "Format to save the content", "enum": ["original", "cleaned", "both"] }
                                }
                            }
                        }
                    }
                },
                "required": ["url"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let url = input.get("url");
        if !js::truthy(url) {
            errors.push("URL is required".to_string());
        }
        if !matches!(url, Some(Value::String(_))) {
            errors.push("URL must be a string".to_string());
        }
        if js::truthy(url) {
            let valid = url
                .and_then(Value::as_str)
                .is_some_and(|u| url::Url::parse(u).is_ok());
            if !valid {
                errors.push("Invalid URL format".to_string());
            }
        }
        if let Some(options) = input.get("options").filter(|o| js::truthy(Some(o))) {
            let lines = options.get("lines");
            if js::truthy(lines) {
                errors.extend(validate_line_range(lines));
            }
            if let Some(c) = options.get("cleaning") {
                if !c.is_boolean() {
                    errors.push("cleaning must be a boolean".to_string());
                }
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input.get("url").and_then(Value::as_str).unwrap_or_default();
        let empty = Map::new();
        let options = input
            .get("options")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let cleaning = js::truthy(options.get("cleaning"));
        let lines = LineRange::from_value(options.get("lines"));
        let save = options.get("saveToFile").filter(|s| js::truthy(Some(s)));

        let request = FetchOptions {
            method: options
                .get("method")
                .and_then(Value::as_str)
                .map(str::to_string),
            headers: options
                .get("headers")
                .and_then(Value::as_object)
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| {
                            let v = match v {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            (k.clone(), v)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            body: options.get("body").and_then(|b| match b {
                Value::String(s) => Some(s.clone()),
                Value::Null => None,
                other => Some(other.to_string()),
            }),
        };

        let response = fetch_website(url, &request, ctx.settings.proxy.as_ref())
            .await
            .map_err(|msg| {
                if msg.contains("net::") {
                    return ToolError::network(
                        format!("Network error fetching website: {msg}"),
                        NAME,
                        Some(url),
                        None,
                    );
                }
                let mut extra = Map::new();
                extra.insert("url".into(), json!(url));
                ToolError::execution(
                    format!("Error fetching website: {msg}"),
                    NAME,
                    Some(json!({})),
                    Some(extra),
                )
            })?;

        let original = match &response.data {
            Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        let mut processed = original.clone();
        let mut cleaned: Option<String> = None;
        if cleaning {
            let c = extract_main_content(&original);
            processed = c.clone();
            cleaned = Some(c);
        }

        let limit = match ctx.settings.llm_max_tokens_limit {
            Some(n) if n != 0.0 && !n.is_nan() => n,
            _ => DEFAULT_MAX_TOKENS_LIMIT,
        };
        if (js::len(&processed) as f64 * 0.8).ceil() > limit {
            processed = truncate_content_middle_out(&processed, limit);
        }

        let mut save_results: Vec<String> = Vec::new();
        if let Some(save) = save {
            let format = save.get("format").and_then(Value::as_str);
            let filename = save.get("filename").and_then(Value::as_str);
            let directory = save.get("directory").and_then(Value::as_str);
            let project = ctx.settings.project_path.as_deref();
            let mut results = Vec::new();
            if matches!(format, Some("original") | Some("both") | None)
                || format.is_some_and(str::is_empty)
            {
                results.push(
                    save_website_content(
                        &original,
                        url,
                        filename,
                        directory,
                        SaveFormat::Html,
                        project,
                    )
                    .await,
                );
            }
            if matches!(format, Some("cleaned") | Some("both")) {
                let to_save = cleaned
                    .clone()
                    .unwrap_or_else(|| extract_main_content(&original));
                let cleaned_name = filename
                    .filter(|f| !f.is_empty())
                    .map(|f| format!("{f}_cleaned"));
                results.push(
                    save_website_content(
                        &to_save,
                        url,
                        cleaned_name.as_deref(),
                        directory,
                        SaveFormat::Txt,
                        project,
                    )
                    .await,
                );
            }
            for r in results {
                save_results.push(match r {
                    SaveResult::Saved(p) => format!("✓ File saved successfully: {p}"),
                    SaveResult::Failed(e) => format!("✗ Failed to save file: {e}"),
                });
            }
        }

        let filtered = filter_by_line_range(&processed, lines.as_ref());
        let total = processed.split('\n').count();
        let info = get_line_range_info(total, lines.as_ref());
        let mut header = format!(
            "Website Content: {url}{info}\n{}\n",
            "=".repeat(js::len(url) + js::len(&info) + 18)
        );
        if !save_results.is_empty() {
            header.push_str(&format!("\nSave Results:\n{}\n\n", save_results.join("\n")));
        }
        Ok(ToolOutput::Text(format!("{header}{filtered}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use crate::web::http::test_server;

    #[test]
    fn cleaning() {
        let html = "<html><head><style>p{color:red}</style><script>var a = '<b>';</script></head><body><p>Hello&nbsp;&amp; <b>world</b></p>\n\n<p>&lt;ok&gt; &quot;q&quot; &#39;s&#39;</p></body></html>";
        assert_eq!(extract_main_content(html), "Hello & world <ok> \"q\" 's'");
    }

    #[test]
    fn middle_out() {
        let content = "a".repeat(1000);
        // limit 80 tokens -> 100 chars; 40 front + 40 back.
        let out = truncate_content_middle_out(&content, 80.0);
        assert_eq!(
            out,
            format!(
                "{}\n\n[Content truncated... (920 characters omitted)]\n\n{}",
                "a".repeat(40),
                "a".repeat(40)
            )
        );
        // Word boundaries near the cut are preferred.
        let words = format!("{} tail{}", "x".repeat(37), "y".repeat(1000));
        let out = truncate_content_middle_out(&words, 80.0);
        assert!(out.starts_with(&format!("{}\n\n[Content", "x".repeat(37))));
        assert_eq!(truncate_content_middle_out("short", 80.0), "short");
    }

    #[test]
    fn validation() {
        let t = FetchWebsiteTool;
        assert_eq!(
            t.validate_input(&json!({})),
            vec!["URL is required", "URL must be a string"]
        );
        assert_eq!(
            t.validate_input(&json!({"url": "not a url"})),
            vec!["Invalid URL format"]
        );
        assert_eq!(
            t.validate_input(&json!({"url": "https://x.com", "options": {"cleaning": "yes", "lines": {"from": 0}}})),
            vec!["Line range \"from\" must be a positive integer", "cleaning must be a boolean"]
        );
    }

    #[tokio::test]
    async fn fetches_cleans_filters_and_saves() {
        let body =
            "<html><body><h1>Title</h1><script>x()</script><p>Body</p></body></html>".to_string();
        let (base, _h) = test_server::serve(vec![(200, "text/html", body.clone())]).await;
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        let url = format!("{base}/page");
        let out = run_tool(
            &FetchWebsiteTool,
            json!({"type": "fetchWebsite", "url": url, "options": {
                "cleaning": true,
                "saveToFile": {"filename": "p", "directory": d, "format": "both"}
            }}),
            &ToolContext::default(),
        )
        .await
        .unwrap();
        let text = out.as_text().unwrap();
        let rule = "=".repeat(url.len() + 18);
        assert!(
            text.starts_with(&format!(
                "Website Content: {url}\n{rule}\n\nSave Results:\n✓ File saved successfully: "
            )),
            "{text}"
        );
        assert!(text.ends_with("\n\nTitle Body"), "{text}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("p.html")).unwrap(),
            body
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("p_cleaned.txt")).unwrap(),
            "Title Body"
        );
    }

    #[tokio::test]
    async fn json_bodies_are_pretty_printed_and_line_filtered() {
        let (base, _h) = test_server::serve(vec![(
            200,
            "application/json",
            r#"{"a":1,"b":2}"#.to_string(),
        )])
        .await;
        let out = run_tool(
            &FetchWebsiteTool,
            json!({"type": "fetchWebsite", "url": base, "options": {"lines": {"from": 2, "to": 2}}}),
            &ToolContext::default(),
        )
        .await
        .unwrap();
        let text = out.as_text().unwrap();
        assert!(text.contains(" (lines 2 to 2)\n"), "{text}");
        assert!(text.ends_with("\n  \"a\": 1,"), "{text}");
    }

    #[tokio::test]
    async fn http_errors_are_execution_errors() {
        let (base, _h) = test_server::serve(vec![(500, "text/plain", "x".to_string())]).await;
        let err = run_tool(
            &FetchWebsiteTool,
            json!({"type": "fetchWebsite", "url": base}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(
            v["error"],
            "Error fetching website: Request failed with status code 500"
        );
        assert_eq!(v["type"], "EXECUTION");
        assert_eq!(v["url"], base);
    }
}
