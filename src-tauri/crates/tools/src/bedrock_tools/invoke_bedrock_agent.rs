//! Port of `InvokeBedrockAgentTool.ts`.

use super::{is_string, present, thrown_json, BedrockBackend};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js::truthy;
use crate::util::node_io::NodeIoError;
use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Arc;

const NAME: &str = "invokeBedrockAgent";
const DESCRIPTION: &str = "Invoke an Amazon Bedrock Agent using the specified agent ID and alias ID. Use this when you need to interact with an agent.\n\nInteract with AWS Bedrock agents. Only use Bedrock Agents from allowed list: {{bedrockAgents}}";

pub struct InvokeBedrockAgentTool {
    backend: Arc<dyn BedrockBackend>,
}

impl InvokeBedrockAgentTool {
    pub fn new(backend: Arc<dyn BedrockBackend>) -> Self {
        Self { backend }
    }
}

/// `getMimeType(filePath)`.
pub fn mime_type_for(file_path: &str) -> &'static str {
    let ext = Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .and_then(|n| match n.rfind('.') {
            Some(0) | None => None,
            Some(i) => Some(n[i..].to_lowercase()),
        })
        .unwrap_or_default();
    match ext.as_str() {
        ".html" => "text/html",
        ".js" => "text/javascript",
        ".css" => "text/css",
        ".json" => "application/json",
        ".png" => "image/png",
        ".jpg" => "image/jpeg",
        ".gif" => "image/gif",
        ".svg" => "image/svg+xml",
        ".wav" => "audio/wav",
        ".mp4" => "video/mp4",
        ".woff" => "application/font-woff",
        ".ttf" => "application/font-ttf",
        ".eot" => "application/vnd.ms-fontobject",
        ".otf" => "application/font-otf",
        ".wasm" => "application/wasm",
        ".csv" => "text/csv",
        _ => "text/plain",
    }
}

fn basename(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl InvokeBedrockAgentTool {
    async fn run(&self, input: &Value, ctx: &ToolContext) -> std::result::Result<Value, String> {
        let s = |k: &str| input.get(k).and_then(Value::as_str);
        let agent_id = s("agentId").unwrap_or_default();
        let alias_id = s("agentAliasId").unwrap_or_default();

        let mut command = Map::new();
        command.insert("agentId".into(), json!(agent_id));
        command.insert("agentAliasId".into(), json!(alias_id));
        if let Some(sid) = s("sessionId") {
            command.insert("sessionId".into(), json!(sid));
        }
        command.insert(
            "inputText".into(),
            json!(s("inputText").unwrap_or_default()),
        );
        command.insert("enableTrace".into(), json!(true));

        let file = input.get("file").filter(|f| truthy(Some(f)));
        if let Some(file_path) = file
            .and_then(|f| f.get("filePath"))
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
        {
            let content = tokio::fs::read(file_path)
                .await
                .map_err(|e| NodeIoError::new(&e, "open", file_path, None).message)?;
            let filename = basename(file_path);
            let media_type = if filename.ends_with(".csv") {
                "text/csv"
            } else {
                mime_type_for(file_path)
            };
            let mut entry = Map::new();
            entry.insert("name".into(), json!(filename));
            entry.insert(
                "source".into(),
                json!({
                    "sourceType": "BYTE_CONTENT",
                    "byteContent": {
                        "mediaType": media_type,
                        "data": base64::engine::general_purpose::STANDARD.encode(&content)
                    }
                }),
            );
            if let Some(use_case) = file.and_then(|f| f.get("useCase")).filter(|u| !u.is_null()) {
                entry.insert("useCase".into(), use_case.clone());
            }
            command.insert("sessionState".into(), json!({ "files": [entry] }));
        }

        let result = self
            .backend
            .invoke_agent(&ctx.settings.converse, &Value::Object(command))
            .await?;

        let mut paths = Vec::new();
        let files = result
            .completion
            .as_ref()
            .map(|c| c.files.as_slice())
            .unwrap_or_default();
        if !files.is_empty() {
            // `path.join(projectPath, file.name)` throws when projectPath is unset.
            let project = ctx.settings.project_path.as_deref().ok_or_else(|| {
                "The \"path\" argument must be of type string. Received undefined".to_string()
            })?;
            for f in files {
                let path = Path::new(project).join(&f.name);
                if let Err(e) = tokio::fs::write(&path, &f.content).await {
                    tracing::error!(file_path = %path.display(), error = %e, "Failed to write file from agent result");
                }
                paths.push(path.to_string_lossy().into_owned());
            }
            tracing::info!(file_count = paths.len(), "Created files from agent result");
        }

        let mut out = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        let completion = out
            .as_object_mut()
            .map(|o| o.entry("completion").or_insert_with(|| json!({})));
        if let Some(Value::Object(c)) = completion {
            c.insert("files".into(), json!(paths));
        }
        Ok(out)
    }
}

#[async_trait]
impl Tool for InvokeBedrockAgentTool {
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
                    "agentId": { "type": "string", "description": "The ID of the agent to invoke" },
                    "agentAliasId": { "type": "string", "description": "The alias ID of the agent to invoke" },
                    "sessionId": {
                        "type": "string",
                        "description": "Optional. The session ID to use for the agent invocation. The session ID is issued when you execute invokeBedrockAgent for the first time and is included in the response. Specify it if you want to continue the conversation from the second time onwards."
                    },
                    "inputText": { "type": "string", "description": "The input text to send to the agent" },
                    "file": {
                        "type": "object",
                        "description": "Optional. The file to send to the agent. Be sure to specify if you need to analyze files.",
                        "properties": {
                            "filePath": { "type": "string", "description": "The path of the file to send to the agent" },
                            "useCase": {
                                "type": "string",
                                "description": "The use case of the file. Specify \"CODE_INTERPRETER\" if Python code analysis is required. Otherwise, specify \"CHAT\".",
                                "enum": ["CODE_INTERPRETER", "CHAT"]
                            }
                        }
                    }
                },
                "required": ["agentId", "agentAliasId", "inputText"]
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        for (key, required, must) in [
            (
                "agentId",
                "Agent ID is required",
                "Agent ID must be a string",
            ),
            (
                "agentAliasId",
                "Agent alias ID is required",
                "Agent alias ID must be a string",
            ),
            (
                "inputText",
                "Input text is required",
                "Input text must be a string",
            ),
        ] {
            if !truthy(input.get(key)) {
                errors.push(required.to_string());
            }
            if !is_string(input.get(key)) {
                errors.push(must.to_string());
            }
        }
        if let Some(Value::String(t)) = input.get("inputText") {
            if !t.is_empty() && t.trim().is_empty() {
                errors.push("Input text cannot be empty".to_string());
            }
        }
        if present(input, "sessionId") && !is_string(input.get("sessionId")) {
            errors.push("Session ID must be a string".to_string());
        }
        if let Some(file) = input.get("file").filter(|f| truthy(Some(f))) {
            let fp = file.get("filePath");
            if truthy(fp) && !is_string(fp) {
                errors.push("File path must be a string".to_string());
            }
            let uc = file.get("useCase");
            if truthy(uc)
                && !matches!(
                    uc.and_then(Value::as_str),
                    Some("CODE_INTERPRETER" | "CHAT")
                )
            {
                errors.push("File use case must be either CODE_INTERPRETER or CHAT".to_string());
            }
        }
        errors
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let agent_id = input
            .get("agentId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let alias_id = input
            .get("agentAliasId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match self.run(&input, ctx).await {
            Ok(result) => Ok(ToolOutput::Json(json!({
                "success": true,
                "name": NAME,
                "message": format!("Invoked agent {agent_id} with alias {alias_id}"),
                "result": result
            }))),
            Err(e) => {
                tracing::error!(agent_id, alias_id, error = %e, "Error invoking Bedrock Agent");
                Err(thrown_json(
                    "Error invoking agent",
                    NAME,
                    "Failed to invoke agent",
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
    use crate::context::ToolSettings;
    use bedrock::agent::{AgentFile, Completion, InvokeAgentResult};

    fn agent_result(files: Vec<AgentFile>) -> InvokeAgentResult {
        InvokeAgentResult {
            metadata: json!({"httpStatusCode": 200}),
            content_type: "application/json".into(),
            session_id: "sess-1".into(),
            completion: Some(Completion {
                message: "done".into(),
                files,
                traces: vec![],
            }),
        }
    }

    #[test]
    fn validation_and_mime_types() {
        let t = InvokeBedrockAgentTool::new(Arc::new(FakeBedrock::default()));
        assert_eq!(
            t.validate_input(&json!({"sessionId": 1, "file": {"filePath": 2, "useCase": "X"}})),
            vec![
                "Agent ID is required",
                "Agent ID must be a string",
                "Agent alias ID is required",
                "Agent alias ID must be a string",
                "Input text is required",
                "Input text must be a string",
                "Session ID must be a string",
                "File path must be a string",
                "File use case must be either CODE_INTERPRETER or CHAT"
            ]
        );
        assert_eq!(
            t.validate_input(&json!({"agentId": "A", "agentAliasId": "B", "inputText": "  "})),
            vec!["Input text cannot be empty"]
        );
        assert_eq!(mime_type_for("/a/B.PNG"), "image/png");
        assert_eq!(mime_type_for("/a/x.py"), "text/plain");
        assert_eq!(mime_type_for("/a/noext"), "text/plain");
    }

    #[tokio::test]
    async fn sends_file_and_saves_returned_files() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.agent.lock().unwrap() = Some(Ok(agent_result(vec![AgentFile {
            name: "out.csv".into(),
            content: b"a,b\n1,2".to_vec(),
        }])));
        let dir = tempfile::tempdir().unwrap();
        let input_file = dir.path().join("data.csv");
        std::fs::write(&input_file, b"hi").unwrap();
        let s = ToolSettings {
            project_path: Some(dir.path().to_string_lossy().into_owned()),
            ..Default::default()
        };
        let v = run_tool(
            &InvokeBedrockAgentTool::new(fake.clone()),
            json!({"type": NAME, "agentId": "AG", "agentAliasId": "AL", "inputText": "analyze",
                   "file": {"filePath": input_file.to_string_lossy(), "useCase": "CODE_INTERPRETER"}}),
            &ToolContext::new(s),
        )
        .await
        .unwrap()
        .into_value();
        let saved = dir.path().join("out.csv");
        assert_eq!(std::fs::read(&saved).unwrap(), b"a,b\n1,2");
        assert_eq!(
            v,
            json!({
                "success": true,
                "name": "invokeBedrockAgent",
                "message": "Invoked agent AG with alias AL",
                "result": {
                    "$metadata": {"httpStatusCode": 200},
                    "contentType": "application/json",
                    "sessionId": "sess-1",
                    "completion": {"message": "done", "files": [saved.to_string_lossy()], "traces": []}
                }
            })
        );
        assert_eq!(
            fake.calls()[0].1,
            json!({
                "agentId": "AG", "agentAliasId": "AL", "inputText": "analyze", "enableTrace": true,
                "sessionState": {"files": [{
                    "name": "data.csv",
                    "source": {"sourceType": "BYTE_CONTENT", "byteContent": {"mediaType": "text/csv", "data": "aGk="}},
                    "useCase": "CODE_INTERPRETER"
                }]}
            })
        );
        // The SDK backend's body builder accepts the base64 bytes.
        let body =
            bedrock::agent::invoke_agent_body(fake.calls()[0].1.as_object().unwrap()).unwrap();
        assert_eq!(
            body["sessionState"]["files"][0]["source"]["byteContent"]["data"],
            "aGk="
        );
    }

    #[tokio::test]
    async fn errors_are_thrown_strings() {
        let fake = Arc::new(FakeBedrock::default());
        *fake.agent.lock().unwrap() = Some(Err("Access denied".into()));
        let t = InvokeBedrockAgentTool::new(fake.clone());
        let input = json!({"type": NAME, "agentId": "AG", "agentAliasId": "AL", "inputText": "x", "sessionId": "s"});
        let err = run_tool(&t, input, &ToolContext::default())
            .await
            .unwrap_err();
        let thrown = r#"Error invoking agent: {"success":false,"name":"invokeBedrockAgent","error":"Failed to invoke agent","message":"Access denied"}"#;
        assert_eq!(response(&err)["error"], thrown);
        assert_eq!(response(&err)["originalError"], thrown);
        assert_eq!(fake.calls()[0].1["sessionId"], "s");

        // Missing input file fails before the call.
        let err = run_tool(
            &t,
            json!({"type": NAME, "agentId": "AG", "agentAliasId": "AL", "inputText": "x", "file": {"filePath": "/nope/f.txt"}}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        assert!(response(&err)["error"]
            .as_str()
            .unwrap()
            .contains(r#""message":"ENOENT: no such file or directory, open '/nope/f.txt'""#));
        assert_eq!(fake.calls().len(), 1);
    }
}
