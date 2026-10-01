//! Structured output — port of `src/main/api/bedrock/services/structuredOutputService.ts`,
//! `utils/toolGenerator.ts` and `services/factories/websiteRecommendationFactory.ts`, i.e. the
//! `get_structured_output` (`POST /structured-output`) and `get_website_recommendations`
//! (`POST /website-recommendations`) commands.
//!
//! The schema becomes a single forced tool; the model's `toolUse.input` is the result.
//!
//! Errors are `StructuredOutputError`s: `{ name: "StructuredOutputError", message, code,
//! details }` with `code` `MISSING_OUTPUT` (no matching tool use) or `VALIDATION_ERROR` (the
//! Converse call failed; `message` is then the underlying error message, which is what the
//! Express route reported via `details.originalError`).

use crate::converse::ConverseService;
use crate::error::{Error, Result};
use crate::request::ConverseRequest;
use crate::settings::{ConverseSettings, InferenceConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

/// Default tool name.
pub const DEFAULT_TOOL_NAME: &str = "structured_output";
/// Default tool description.
pub const DEFAULT_TOOL_DESCRIPTION: &str =
    "Return structured data according to the specified schema";

/// `toolOptions`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `StructuredOutputRequest` (the `get_structured_output` command's `request`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredOutputRequest {
    pub model_id: String,
    pub system_prompt: String,
    pub user_message: String,
    pub output_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_options: Option<ToolOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
}

/// The `get_website_recommendations` command's `request`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteRecommendationsRequest {
    pub website_code: String,
    pub language: String,
    pub model_id: String,
}

fn or_default(v: Option<&str>, default: &str) -> String {
    v.filter(|s| !s.is_empty()).unwrap_or(default).to_string()
}

/// `ToolGenerator.generateFromSchema(schema, options)`.
pub fn generate_tool_from_schema(schema: &Value, options: Option<&ToolOptions>) -> Value {
    json!({
        "toolSpec": {
            "name": or_default(options.and_then(|o| o.name.as_deref()), DEFAULT_TOOL_NAME),
            "description": or_default(
                options.and_then(|o| o.description.as_deref()),
                DEFAULT_TOOL_DESCRIPTION
            ),
            "inputSchema": { "json": schema }
        }
    })
}

/// `ToolGenerator.wrapSchema(schema, wrapperKey = 'output')`.
pub fn wrap_schema(schema: &Value, wrapper_key: Option<&str>) -> Value {
    let key = wrapper_key.unwrap_or("output");
    let mut properties = Map::new();
    properties.insert(key.to_string(), schema.clone());
    json!({ "type": "object", "properties": properties, "required": [key] })
}

/// The Converse request `getStructuredOutput` sends, and the forced tool's name.
pub fn structured_output_converse_request(
    req: &StructuredOutputRequest,
) -> (ConverseRequest, String) {
    let tool = generate_tool_from_schema(&req.output_schema, req.tool_options.as_ref());
    let tool_name = tool["toolSpec"]["name"]
        .as_str()
        .unwrap_or(DEFAULT_TOOL_NAME)
        .to_string();
    let request = ConverseRequest {
        model_id: req.model_id.clone(),
        system: Some(vec![json!({ "text": req.system_prompt })]),
        messages: vec![json!({ "role": "user", "content": [{ "text": req.user_message }] })],
        tool_config: Some(json!({
            "tools": [tool],
            "toolChoice": { "tool": { "name": tool_name } }
        })),
        inference_config: req.inference_config.clone(),
        ..Default::default()
    };
    (request, tool_name)
}

/// The `input` of the first `toolUse` named `tool_name` in a Converse response, if it is set.
pub fn extract_structured_output(response: &Value, tool_name: &str) -> Option<Value> {
    response
        .pointer("/output/message/content")?
        .as_array()?
        .iter()
        .find(|c| c.pointer("/toolUse/name").and_then(Value::as_str) == Some(tool_name))?
        .pointer("/toolUse/input")
        .filter(|v| !v.is_null())
        .cloned()
}

fn structured_error(message: String, code: &str, details: Value) -> Error {
    let mut fields = Map::new();
    fields.insert("code".into(), json!(code));
    fields.insert("details".into(), details);
    Error::Custom {
        name: "StructuredOutputError".into(),
        message,
        fields,
    }
}

/// `getStructuredOutput(params)`.
pub async fn get_structured_output(
    converse: &ConverseService,
    settings: &ConverseSettings,
    req: &StructuredOutputRequest,
) -> Result<Value> {
    let (request, tool_name) = structured_output_converse_request(req);
    tracing::debug!(model_id = %req.model_id, tool_name = %tool_name, "Getting structured output");
    let response = converse
        .converse(settings, &request, &CancellationToken::new())
        .await
        .map_err(|e| {
            let original = e.message();
            tracing::error!(model_id = %req.model_id, tool_name = %tool_name, error = %original, "Error getting structured output");
            structured_error(
                original.clone(),
                "VALIDATION_ERROR",
                json!({ "modelId": req.model_id, "toolName": tool_name, "originalError": original }),
            )
        })?;
    extract_structured_output(&response, &tool_name).ok_or_else(|| {
        tracing::error!(model_id = %req.model_id, tool_name = %tool_name, "No structured output found in response");
        structured_error(
            "No structured output found in response".into(),
            "MISSING_OUTPUT",
            json!({ "modelId": req.model_id, "toolName": tool_name }),
        )
    })
}

/// `WebsiteRecommendationFactory.buildSystemPrompt(language)`.
pub fn website_recommendation_system_prompt(language: &str) -> String {
    format!(
        "You are an AI assistant specializing in web UI/UX improvements.
Analyze the provided website code and provide actionable recommendations using the 'recommend_website_changes' tool.

Focus on:
- User experience improvements
- Accessibility enhancements
- Performance optimizations
- Visual design improvements
- Navigation simplification
- Content clarity

Guidelines:
- Provide at least 2 and up to 5 recommendations
- Each recommendation should have a concise title (max 10 characters)
- Each recommendation should include detailed improvement instructions
- Format recommendations as actionable directives (e.g., \"Add ~\", \"Change to ~\")
- Prioritize UI/UX improvements that directly benefit users

Respond in: {language}"
    )
}

/// `WebsiteRecommendationFactory.getOutputSchema()`.
pub fn website_recommendation_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "recommendations": {
                "type": "array",
                "description": "List of recommended changes for website improvement",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Short title summarizing the recommendation (max 10 characters)"
                        },
                        "value": {
                            "type": "string",
                            "description": "Detailed recommendation in instruction form (e.g., \"add ~\", \"change to ~\")"
                        }
                    },
                    "required": ["title", "value"]
                },
                "minItems": 2,
                "maxItems": 5
            }
        },
        "required": ["recommendations"]
    })
}

/// `WebsiteRecommendationFactory.createRequest(...)` plus the route's `modelId`.
pub fn website_recommendation_request(
    req: &WebsiteRecommendationsRequest,
) -> StructuredOutputRequest {
    StructuredOutputRequest {
        model_id: req.model_id.clone(),
        system_prompt: website_recommendation_system_prompt(&req.language),
        user_message: req.website_code.clone(),
        output_schema: website_recommendation_schema(),
        tool_options: Some(ToolOptions {
            name: Some("recommend_website_changes".into()),
            description: Some(
                "Generate recommendations for website improvements based on code analysis".into(),
            ),
        }),
        inference_config: Some(InferenceConfig {
            max_tokens: Some(2048),
            temperature: Some(0.5),
            ..Default::default()
        }),
    }
}

/// `POST /website-recommendations`: `{ recommendations: [{ title, value }] }`.
pub async fn get_website_recommendations(
    converse: &ConverseService,
    settings: &ConverseSettings,
    req: &WebsiteRecommendationsRequest,
) -> Result<Value> {
    get_structured_output(converse, settings, &website_recommendation_request(req)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_generation_defaults() {
        let schema = json!({ "type": "object" });
        assert_eq!(
            generate_tool_from_schema(&schema, None),
            json!({ "toolSpec": {
                "name": "structured_output",
                "description": "Return structured data according to the specified schema",
                "inputSchema": { "json": { "type": "object" } }
            } })
        );
        let t = generate_tool_from_schema(
            &schema,
            Some(&ToolOptions {
                name: Some("x".into()),
                description: Some(String::new()),
            }),
        );
        assert_eq!(t["toolSpec"]["name"], "x");
        assert_eq!(t["toolSpec"]["description"], DEFAULT_TOOL_DESCRIPTION);
        assert_eq!(
            wrap_schema(&schema, None),
            json!({ "type": "object", "properties": { "output": { "type": "object" } }, "required": ["output"] })
        );
    }

    #[test]
    fn converse_request_forces_the_tool() {
        let (r, name) = structured_output_converse_request(&StructuredOutputRequest {
            model_id: "m".into(),
            system_prompt: "sys".into(),
            user_message: "hi".into(),
            output_schema: json!({ "type": "object" }),
            ..Default::default()
        });
        assert_eq!(name, "structured_output");
        assert_eq!(r.system, Some(vec![json!({ "text": "sys" })]));
        assert_eq!(
            r.messages,
            vec![json!({ "role": "user", "content": [{ "text": "hi" }] })]
        );
        let tc = r.tool_config.unwrap();
        assert_eq!(
            tc["toolChoice"],
            json!({ "tool": { "name": "structured_output" } })
        );
        assert_eq!(
            tc["tools"][0]["toolSpec"]["inputSchema"]["json"]["type"],
            "object"
        );
        assert_eq!(r.inference_config, None);
    }

    #[test]
    fn extraction() {
        let resp = json!({ "output": { "message": { "content": [
            { "text": "thinking" },
            { "toolUse": { "name": "other", "input": { "a": 0 } } },
            { "toolUse": { "name": "t", "input": { "a": 1 } } }
        ] } } });
        assert_eq!(
            extract_structured_output(&resp, "t"),
            Some(json!({ "a": 1 }))
        );
        assert_eq!(extract_structured_output(&resp, "missing"), None);
        assert_eq!(extract_structured_output(&json!({}), "t"), None);
    }

    #[test]
    fn website_request() {
        let r = website_recommendation_request(&WebsiteRecommendationsRequest {
            website_code: "<html/>".into(),
            language: "日本語".into(),
            model_id: "m".into(),
        });
        assert!(r.system_prompt.ends_with("Respond in: 日本語"));
        assert!(r
            .system_prompt
            .starts_with("You are an AI assistant specializing"));
        assert_eq!(r.user_message, "<html/>");
        assert_eq!(
            r.output_schema["properties"]["recommendations"]["maxItems"],
            5
        );
        assert_eq!(
            r.tool_options.unwrap().name.as_deref(),
            Some("recommend_website_changes")
        );
        let ic = r.inference_config.unwrap();
        assert_eq!((ic.max_tokens, ic.temperature), (Some(2048), Some(0.5)));
    }

    #[test]
    fn error_shape() {
        let e = structured_error("m".into(), "MISSING_OUTPUT", json!({ "toolName": "t" }));
        assert_eq!(
            e.to_json(),
            json!({ "name": "StructuredOutputError", "message": "m", "code": "MISSING_OUTPUT", "details": { "toolName": "t" } })
        );
    }
}
