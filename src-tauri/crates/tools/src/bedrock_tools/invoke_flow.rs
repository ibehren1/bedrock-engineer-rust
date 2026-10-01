//! Port of `InvokeFlowTool.ts`.

use super::{thrown_json, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

const NAME: &str = "invokeFlow";
const DESCRIPTION: &str = "Invoke AWS Bedrock Flow to execute the specified flow. Flows can be used to automate workflows consisting of multiple steps.\n\nExecute AWS Bedrock Flows. Only use Bedrock Flows from allowed list: {{bedrockFlows}}";

pub struct InvokeFlowTool {
    backend: Arc<dyn BedrockBackend>,
}

impl InvokeFlowTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

/// `parseInput`: objects (and arrays) as-is, JSON strings parsed, anything else wrapped as
/// `{ content: { document } }`.
pub fn parse_flow_input(input: &Value) -> Value {
    match input {
        Value::Object(_) | Value::Array(_) => input.clone(),
        Value::String(s) => {
            serde_json::from_str(s).unwrap_or_else(|_| json!({ "content": { "document": s } }))
        }
        other => json!({ "content": { "document": other } }),
    }
}

fn flow_id_for_log(id: &str) -> String {
    if crate::util::js::len(id) > 8 {
        format!("{}...", crate::util::js::slice(id, 0, 8))
    } else if id.is_empty() {
        "unknown".to_string()
    } else {
        id.to_string()
    }
}

#[async_trait]
impl Tool for InvokeFlowTool {
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
                    "flowIdentifier": { "type": "string", "description": "The identifier of the Flow to execute" },
                    "flowAliasIdentifier": { "type": "string", "description": "The alias identifier of the Flow" },
                    "input": {
                        "type": "object",
                        "description": "Input data for the Flow",
                        "properties": {
                            "content": {
                                "type": "object",
                                "properties": {
                                    "document": {
                                        "description": "Data to send to the Flow. Accepts strings, numbers, booleans, objects, and arrays.",
                                        "anyOf": [
                                            { "type": "string" },
                                            { "type": "number" },
                                            { "type": "boolean" },
                                            { "type": "object" },
                                            { "type": "array" }
                                        ]
                                    }
                                },
                                "required": ["document"]
                            }
                        },
                        "required": ["content"]
                    }
                },
                "required": ["flowIdentifier", "flowAliasIdentifier", "input"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        let id = input.get("flowIdentifier");
        if !truthy(id) {
            errors.push(
                "Flow identifier is required. Use \"flowIdentifier\" parameter, not \"flowId\"."
                    .to_string(),
            );
        }
        if truthy(id) && !id.is_some_and(Value::is_string) {
            errors.push("Flow identifier must be a string".to_string());
        }
        let alias = input.get("flowAliasIdentifier");
        if !truthy(alias) {
            errors.push(
                "Flow alias identifier is required. Use \"flowAliasIdentifier\" parameter, not \"flowAliasId\"."
                    .to_string(),
            );
        }
        if truthy(alias) && !alias.is_some_and(Value::is_string) {
            errors.push("Flow alias identifier must be a string".to_string());
        }
        let Some(raw) = input.get("input").filter(|v| !v.is_null()) else {
            errors.push(
                "Input is required. Use \"input.content.document\" structure, not \"inputData\"."
                    .to_string(),
            );
            return errors;
        };
        let parsed = parse_flow_input(raw);
        let content = parsed.get("content");
        if !truthy(content) || !content.is_some_and(|c| c.is_object() || c.is_array()) {
            errors.push(
                "Input content must be an object. Use \"input.content.document\" structure."
                    .to_string(),
            );
        }
        for (key, message) in [
            ("nodeName", "Input nodeName must be a string"),
            ("nodeOutputName", "Input nodeOutputName must be a string"),
        ] {
            let v = parsed.get(key);
            if truthy(v) && !v.is_some_and(Value::is_string) {
                errors.push(message.to_string());
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let flow_id = input
            .get("flowIdentifier")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let alias_id = input
            .get("flowAliasIdentifier")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if flow_id.is_empty() || alias_id.is_empty() {
            return Err(crate::ToolError::plain(
                "Missing required parameters: flowIdentifier and flowAliasIdentifier are required",
            ));
        }

        let mut processed = parse_flow_input(input.get("input").unwrap_or(&Value::Null));
        if let Some(o) = processed.as_object_mut() {
            for (key, default) in [
                ("nodeName", "FlowInputNode"),
                ("nodeOutputName", "document"),
            ] {
                if !truthy(o.get(key)) {
                    o.insert(key.into(), json!(default));
                }
            }
        }

        tracing::info!(flow_identifier = %flow_id_for_log(&flow_id), flow_alias_identifier = %alias_id, "Calling Bedrock Flow API");
        let params = json!({
            "flowIdentifier": flow_id,
            "flowAliasIdentifier": alias_id,
            "inputs": [processed]
        });
        match self
            .backend
            .invoke_flow(&ctx.settings.converse, &params)
            .await
        {
            Ok(response) => {
                let count = response
                    .get("outputs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                Ok(ToolOutput::Json(json!({
                    "success": true,
                    "name": NAME,
                    "message": format!("Flow invoked successfully with {count} output(s)"),
                    "result": response
                })))
            }
            Err(e) => {
                tracing::error!(flow_identifier = %flow_id_for_log(&flow_id), error = %e, "Error invoking Bedrock Flow");
                Err(thrown_json(
                    "Error invokeFlow",
                    NAME,
                    "Failed to invoke Bedrock Flow",
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
    fn parse_input_forms() {
        assert_eq!(
            parse_flow_input(&json!({"content": {"document": 1}})),
            json!({"content": {"document": 1}})
        );
        assert_eq!(
            parse_flow_input(&json!(
                "{\"content\":{\"document\":\"x\"},\"nodeName\":\"N\"}"
            )),
            json!({"content": {"document": "x"}, "nodeName": "N"})
        );
        assert_eq!(
            parse_flow_input(&json!("plain text")),
            json!({"content": {"document": "plain text"}})
        );
        assert_eq!(
            parse_flow_input(&json!(5)),
            json!({"content": {"document": 5}})
        );
        assert_eq!(flow_id_for_log("ABCDEFGHIJ"), "ABCDEFGH...");
    }

    #[test]
    fn validation_messages() {
        let t = InvokeFlowTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({"flowId": "x"})),
            vec![
                "Flow identifier is required. Use \"flowIdentifier\" parameter, not \"flowId\".",
                "Flow alias identifier is required. Use \"flowAliasIdentifier\" parameter, not \"flowAliasId\".",
                "Input is required. Use \"input.content.document\" structure, not \"inputData\"."
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"flowIdentifier": 1, "flowAliasIdentifier": true, "input": {"content": "s", "nodeName": 3, "nodeOutputName": {}}})),
            vec![
                "Flow identifier must be a string",
                "Flow alias identifier must be a string",
                "Input content must be an object. Use \"input.content.document\" structure.",
                "Input nodeName must be a string",
                "Input nodeOutputName must be a string"
            ]
        );
        assert!(t
            .validate_input(
                &json!({"flowIdentifier": "F", "flowAliasIdentifier": "A", "input": "hello"})
            )
            .is_empty());
    }

    #[tokio::test]
    async fn invokes_with_defaults() {
        let fake = Arc::new(FakeBedrock::default());
        let out = json!({"executionId": "e", "outputs": [{"content": {"document": "r"}, "nodeName": "FlowOutputNode", "nodeOutputName": "document"}]});
        *fake.flow.lock().unwrap() = Some(Ok(out.clone()));
        let v = run_tool(
            &InvokeFlowTool::new(fake.clone()),
            json!({"type": NAME, "flowIdentifier": "FLOW", "flowAliasIdentifier": "ALIAS", "input": {"content": {"document": {"q": 1}}, "nodeName": ""}}),
            &ToolContext::default(),
        )
        .await
        .unwrap()
        .into_value();
        assert_eq!(
            v,
            json!({"success": true, "name": "invokeFlow", "message": "Flow invoked successfully with 1 output(s)", "result": out})
        );
        let sent = &fake.calls()[0].1;
        assert_eq!(
            sent,
            &json!({"flowIdentifier": "FLOW", "flowAliasIdentifier": "ALIAS", "inputs": [
                {"content": {"document": {"q": 1}}, "nodeName": "FlowInputNode", "nodeOutputName": "document"}
            ]})
        );
        assert_eq!(
            serde_json::to_string(&sent["inputs"][0]).unwrap(),
            r#"{"content":{"document":{"q":1}},"nodeName":"FlowInputNode","nodeOutputName":"document"}"#
        );
    }

    #[tokio::test]
    async fn failure_is_thrown_string() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.flow.lock().unwrap() = Some(Err("Flow not found".into()));
        let err = run_tool(
            &InvokeFlowTool::new(fake),
            json!({"type": NAME, "flowIdentifier": "F", "flowAliasIdentifier": "A", "input": "x"}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            response(&err)["error"],
            r#"Error invokeFlow: {"success":false,"name":"invokeFlow","error":"Failed to invoke Bedrock Flow","message":"Flow not found"}"#
        );
    }
}
