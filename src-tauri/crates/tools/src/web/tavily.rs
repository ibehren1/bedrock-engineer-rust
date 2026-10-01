//! Port of `src/preload/tools/handlers/web/TavilySearchTool.ts`.

use super::http::{fetch_website, FetchOptions};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js;
use crate::util::validate::Issues;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::LazyLock;

const NAME: &str = "tavilySearch";
const DESCRIPTION: &str = "Perform a web search using Tavily API to get up-to-date information or additional context. Use this when you need current information or feel a search could provide a better answer.\n\nSearch the web for current information. Always cite sources and provide URLs.";

/// Tavily search endpoint.
pub const TAVILY_ENDPOINT: &str = "https://api.tavily.com/search";

static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("regex"));

/// `TavilySearchTool`.
pub struct TavilySearchTool {
    endpoint: String,
}

impl Default for TavilySearchTool {
    fn default() -> Self {
        TavilySearchTool {
            endpoint: TAVILY_ENDPOINT.to_string(),
        }
    }
}

impl TavilySearchTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point at a different endpoint (tests).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        TavilySearchTool {
            endpoint: endpoint.into(),
        }
    }
}

/// The Zod-generated JSON schema of `tavilySearchInputSchema`.
fn input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "minLength": 1,
                "maxLength": 400,
                "description": "The search query (max 400 characters)"
            },
            "option": {
                "type": "object",
                "properties": {
                    "include_raw_content": {
                        "type": "boolean",
                        "description": "Whether to include raw content in the search results. DEFAULT is false"
                    },
                    "max_results": {
                        "type": "number",
                        "minimum": 1,
                        "maximum": 20,
                        "description": "Maximum number of search results to return (1-20). DEFAULT is 10"
                    },
                    "start_date": {
                        "type": "string",
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}$",
                        "description": "Return results after this date based on publish date or last updated date (YYYY-MM-DD format, e.g., \"2025-02-09\")"
                    },
                    "end_date": {
                        "type": "string",
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}$",
                        "description": "Return results before this date based on publish date or last updated date (YYYY-MM-DD format, e.g., \"2025-12-29\")"
                    }
                },
                "additionalProperties": false
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

#[async_trait]
impl Tool for TavilySearchTool {
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
        Some(ToolSpec::new(NAME, DESCRIPTION, input_schema()))
    }

    /// `tavilySearchInputSchema.parse(input)`.
    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        if !input.is_object() {
            issues.invalid_type(&[], "object", Some(input));
            return issues.into_vec();
        }
        if let Some(q) = issues.string(&["query"], input.get("query"), false) {
            let n = js::len(q);
            if n < 1 {
                issues.push(&["query"], "Query cannot be empty");
            }
            if n > 400 {
                issues.push(&["query"], "Query must be 400 characters or less");
            }
        }
        match input.get("option") {
            None => {}
            Some(Value::Object(o)) => {
                if let Some(v) = o.get("include_raw_content") {
                    if !v.is_boolean() {
                        issues.invalid_type(&["option", "include_raw_content"], "boolean", Some(v));
                    }
                }
                if let Some(v) = o.get("max_results") {
                    match v.as_f64() {
                        None => issues.invalid_type(&["option", "max_results"], "number", Some(v)),
                        Some(n) => {
                            if n < 1.0 {
                                issues.push(
                                    &["option", "max_results"],
                                    "Number must be greater than or equal to 1",
                                );
                            }
                            if n > 20.0 {
                                issues.push(
                                    &["option", "max_results"],
                                    "Number must be less than or equal to 20",
                                );
                            }
                        }
                    }
                }
                for key in ["start_date", "end_date"] {
                    if let Some(s) = issues.string(&["option", key], o.get(key), true) {
                        if !DATE.is_match(s) {
                            issues.push(&["option", key], "Date must be in YYYY-MM-DD format");
                        }
                    }
                }
            }
            Some(other) => issues.invalid_type(&["option"], "object", Some(other)),
        }
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let option = input.get("option");
        let query_meta = || {
            let mut m = Map::new();
            m.insert("query".into(), json!(query));
            Some(m)
        };

        let Some(api_key) = ctx.settings.tavily_api_key.as_deref() else {
            return Err(ToolError::execution(
                "Tavily API key not configured",
                NAME,
                None,
                query_meta(),
            ));
        };

        let domains = ctx.settings.tavily_domain_config();
        let mut body = Map::new();
        body.insert("query".into(), json!(query));
        body.insert("search_depth".into(), json!("advanced"));
        body.insert("include_answer".into(), json!(true));
        body.insert("include_images".into(), json!(true));
        body.insert(
            "include_raw_content".into(),
            option
                .and_then(|o| o.get("include_raw_content"))
                .cloned()
                .unwrap_or(json!(false)),
        );
        body.insert(
            "max_results".into(),
            option
                .and_then(|o| o.get("max_results"))
                .cloned()
                .unwrap_or(json!(10)),
        );
        for key in ["start_date", "end_date"] {
            if let Some(v) = option
                .and_then(|o| o.get(key))
                .filter(|v| js::truthy(Some(v)))
            {
                body.insert(key.into(), v.clone());
            }
        }
        body.insert("include_domains".into(), json!(domains.include_domains));
        body.insert("exclude_domains".into(), json!(domains.exclude_domains));

        let mut headers = std::collections::BTreeMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));
        let request = FetchOptions {
            method: Some("POST".to_string()),
            headers,
            body: Some(Value::Object(body).to_string()),
        };

        let response = fetch_website(&self.endpoint, &request, ctx.settings.proxy.as_ref())
            .await
            .map_err(|msg| {
                ToolError::execution(
                    format!("Error searching: {msg}"),
                    NAME,
                    Some(json!({})),
                    query_meta(),
                )
            })?;

        if response.status != 200 {
            return Err(ToolError::network(
                format!("Tavily API error: {}", response.status),
                NAME,
                Some(TAVILY_ENDPOINT),
                Some(response.status),
            ));
        }

        Ok(ToolOutput::Json(json!({
            "success": true,
            "name": NAME,
            "message": format!("Searched using Tavily. Query: {query}"),
            "result": response.data
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;
    use crate::web::http::test_server;

    #[test]
    fn zod_style_validation() {
        let t = TavilySearchTool::new();
        assert!(t
            .validate_input(&json!({"type": "tavilySearch", "query": "rust"}))
            .is_empty());
        assert_eq!(t.validate_input(&json!({})), vec!["query: Required"]);
        assert_eq!(
            t.validate_input(&json!({"query": 5})),
            vec!["query: Expected string, received number"]
        );
        assert_eq!(
            t.validate_input(&json!({"query": ""})),
            vec!["query: Query cannot be empty"]
        );
        assert_eq!(
            t.validate_input(&json!({"query": "x".repeat(401)})),
            vec!["query: Query must be 400 characters or less"]
        );
        assert_eq!(
            t.validate_input(
                &json!({"query": "q", "option": {"max_results": 0, "start_date": "2025/01/01"}})
            ),
            vec![
                "option.max_results: Number must be greater than or equal to 1",
                "option.start_date: Date must be in YYYY-MM-DD format"
            ]
        );
    }

    #[tokio::test]
    async fn missing_api_key() {
        let err = run_tool(
            &TavilySearchTool::new(),
            json!({"type": "tavilySearch", "query": "q"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let v: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(v["error"], "Tavily API key not configured");
        assert_eq!(v["query"], "q");
    }

    #[tokio::test]
    async fn posts_search_request() {
        let (base, h) = test_server::serve(vec![(
            200,
            "application/json",
            r#"{"query":"q","results":[]}"#.to_string(),
        )])
        .await;
        let store = json!({
            "tavilySearch": {"apikey": "tvly-key"},
            "selectedAgentId": "a",
            "customAgents": [{"id": "a", "tavilySearchConfig": {"includeDomains": ["docs.rs"], "excludeDomains": ["x.com"]}}]
        });
        let tool = TavilySearchTool::with_endpoint(format!("{base}/search"));
        let out = run_tool(
            &tool,
            json!({"type": "tavilySearch", "query": "q", "option": {"max_results": 3, "start_date": "2025-01-01"}}),
            &ToolContext::from_store(&store, None),
        )
        .await
        .unwrap();
        assert_eq!(
            out,
            ToolOutput::Json(json!({
                "success": true,
                "name": "tavilySearch",
                "message": "Searched using Tavily. Query: q",
                "result": {"query": "q", "results": []}
            }))
        );
        let req = h.await.unwrap().remove(0);
        assert!(req.starts_with("POST /search "));
        assert!(req
            .to_ascii_lowercase()
            .contains("authorization: bearer tvly-key"));
        let body: Value = serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({
                "query": "q", "search_depth": "advanced", "include_answer": true, "include_images": true,
                "include_raw_content": false, "max_results": 3, "start_date": "2025-01-01",
                "include_domains": ["docs.rs"], "exclude_domains": ["x.com"]
            })
        );
    }
}
