//! Port of `RetrieveTool.ts`.

use super::{is_string, thrown_json, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const NAME: &str = "retrieve";
const DESCRIPTION: &str = "Retrieve information from a knowledge base using Amazon Bedrock Knowledge Base. Use this when you need to get information from a knowledge base.\n\nQuery knowledge bases for information. Use for domain-specific data retrieval. Only use Bedrock Knowledgebase from allowed list: {{knowledgeBases}}";

pub struct RetrieveTool {
    backend: Arc<dyn BedrockBackend>,
}

impl RetrieveTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl Tool for RetrieveTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Bedrock
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "knowledgeBaseId": {
                        "type": "string",
                        "description": "The ID of the knowledge base to retrieve from"
                    },
                    "query": {
                        "type": "string",
                        "description": "The query to search for in the knowledge base"
                    }
                },
                "required": ["knowledgeBaseId", "query"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let query = input.get("query");
        if !truthy(query) {
            errors.push("Query is required".to_string());
        }
        if !is_string(query) {
            errors.push("Query must be a string".to_string());
        }
        if let Some(Value::String(q)) = query {
            if !q.is_empty() && q.trim().is_empty() {
                errors.push("Query cannot be empty".to_string());
            }
        }
        let kb = input.get("knowledgeBaseId");
        if !truthy(kb) {
            errors.push("Knowledge base ID is required".to_string());
        }
        if !is_string(kb) {
            errors.push("Knowledge base ID must be a string".to_string());
        }
        if let Some(cfg) = input
            .get("retrievalConfiguration")
            .filter(|v| truthy(Some(v)))
            .and_then(|c| c.get("vectorSearchConfiguration"))
        {
            if let Some(n) = cfg.get("numberOfResults") {
                if !n.as_f64().is_some_and(|n| n >= 1.0) {
                    errors.push("Number of results must be a positive number".to_string());
                }
            }
            if let Some(t) = cfg.get("overrideSearchType") {
                if !matches!(t.as_str(), Some("HYBRID" | "SEMANTIC")) {
                    errors
                        .push("Override search type must be either HYBRID or SEMANTIC".to_string());
                }
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let kb = input
            .get("knowledgeBaseId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut params = Map::new();
        params.insert(
            "query".into(),
            input.get("query").cloned().unwrap_or(Value::Null),
        );
        params.insert("knowledgeBaseId".into(), json!(kb));
        if let Some(cfg) = input.get("retrievalConfiguration") {
            params.insert("retrievalConfiguration".into(), cfg.clone());
        }
        tracing::info!(knowledge_base_id = %kb, "Calling Bedrock Knowledge Base");
        match self
            .backend
            .retrieve(&ctx.settings.converse, &Value::Object(params))
            .await
        {
            Ok(result) => {
                let count = result
                    .get("retrievalResults")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                if count == 0 {
                    tracing::warn!(knowledge_base_id = %kb, "Knowledge Base returned no results");
                }
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": format!("Retrieved information from knowledge base {kb}"),
                    "result": result
                })))
            }
            Err(e) => {
                tracing::error!(knowledge_base_id = %kb, error = %e, "Error retrieving from Knowledge Base");
                Err(thrown_json(
                    "Error retrieve",
                    NAME,
                    "Failed to retrieve information from knowledge base",
                    &e,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake::{response, FakeBedrock};
    use super::*;
    use crate::base::run_tool;

    #[test]
    fn validation_messages() {
        let t = RetrieveTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({})),
            vec![
                "Query is required",
                "Query must be a string",
                "Knowledge base ID is required",
                "Knowledge base ID must be a string"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"query": " ", "knowledgeBaseId": "KB", "retrievalConfiguration": {"vectorSearchConfiguration": {"numberOfResults": 0, "overrideSearchType": "X"}}})),
            vec![
                "Query cannot be empty",
                "Number of results must be a positive number",
                "Override search type must be either HYBRID or SEMANTIC"
            ]
        );
    }

    #[tokio::test]
    async fn retrieves_and_wraps_result() {
        let fake = Arc::new(FakeBedrock::default());
        let out = json!({"$metadata": {"httpStatusCode": 200}, "retrievalResults": [{"content": {"text": "t"}, "score": 0.9}]});
        *fake.retrieve.lock().unwrap() = Some(Ok(out.clone()));
        let t = RetrieveTool::new(fake.clone());
        let v = run_tool(
            &t,
            json!({"type": NAME, "query": "what", "knowledgeBaseId": "KB1"}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v,
            json!({"success": true, "name": "retrieve", "message": "Retrieved information from knowledge base KB1", "result": out})
        );
        assert_eq!(
            fake.calls()[0].1,
            json!({"query": "what", "knowledgeBaseId": "KB1"})
        );
        // And the IPC translation used by the SDK backend.
        assert_eq!(
            bedrock::agent::retrieve_input_from_ipc(&fake.calls()[0].1),
            json!({"knowledgeBaseId": "KB1", "retrievalQuery": {"text": "what"}})
        );
    }

    #[tokio::test]
    async fn failure_is_thrown_string() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.retrieve.lock().unwrap() = Some(Err("KB not found".into()));
        let err = run_tool(
            &RetrieveTool::new(fake),
            json!({"type": NAME, "query": "q", "knowledgeBaseId": "KB1"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let thrown = r#"Error retrieve: {"success":false,"name":"retrieve","error":"Failed to retrieve information from knowledge base","message":"KB not found"}"#;
        assert_eq!(
            response(&err),
            json!({"success": false, "error": thrown, "type": "EXECUTION", "toolName": "retrieve", "originalError": thrown})
        );
    }
}
