//! `addMemory` / `updateMemory` / `removeMemory`: the model's side of the memory list.
//!
//! Results follow the todo tools: `{ name, success: true, result: { item }, message }`, where
//! `item` is the added / updated / removed [`MemoryItem`](super::MemoryItem) (the renderer uses
//! its id for the "Memory saved · Undo" chip). Failures are execution errors.

use super::service::{MemoryItem, MemoryKind, MemoryPatch, MemoryService, MemorySource};
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::validate::{zod_type, Issues};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// The three memory tools sharing one [`MemoryService`].
pub fn create_memory_tools(service: Arc<MemoryService>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(AddMemoryTool {
            service: service.clone(),
        }),
        Arc::new(UpdateMemoryTool {
            service: service.clone(),
        }),
        Arc::new(RemoveMemoryTool { service }),
    ]
}

fn output(name: &str, item: &MemoryItem, message: String) -> ToolOutput {
    ToolOutput::Json(json!({
        "name": name,
        "success": true,
        "result": { "item": item },
        "message": message,
    }))
}

fn kind_issue(issues: &mut Issues, v: Option<&Value>) {
    let expected = "'rule' | 'memory'";
    match v {
        None => {}
        Some(Value::String(s)) if MemoryKind::ALL.contains(&s.as_str()) => {}
        Some(Value::String(s)) => issues.push(
            &["kind"],
            format!("Invalid enum value. Expected {expected}, received '{s}'"),
        ),
        Some(other) => issues.push(
            &["kind"],
            format!("Expected {expected}, received {}", zod_type(other)),
        ),
    }
}

/// A string that must not be blank once trimmed.
fn content_issue(issues: &mut Issues, v: Option<&Value>, optional: bool) {
    if let Some(s) = issues.string(&["content"], v, optional) {
        if s.trim().is_empty() {
            issues.push(&["content"], "String must contain at least 1 character(s)");
        }
    }
}

fn id_issue(issues: &mut Issues, v: Option<&Value>) {
    if let Some(s) = issues.string(&["id"], v, false) {
        if s.is_empty() {
            issues.push(&["id"], "String must contain at least 1 character(s)");
        }
    }
}

fn kind(input: &Value) -> Option<MemoryKind> {
    input
        .get("kind")
        .and_then(Value::as_str)
        .and_then(|s| MemoryKind::parse(s).ok())
}

/// Lowercased with runs of whitespace collapsed, for the duplicate check.
fn normalized(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

const ADD_NAME: &str = "addMemory";
const ADD_DESCRIPTION: &str = "Save a rule or memory that applies to all future chats";
const ADD_SPEC_DESCRIPTION: &str = r#"Save an item to the user's global memory list. Enabled items are included in the system prompt of every future chat.

Use this when the user asks you to remember something, to add a rule, or sets a standing preference ("always ...", "never ...", "from now on ...", "remember that ...").

- kind "rule": an instruction on how you should behave (e.g. "Answer in Japanese", "Use pnpm instead of npm").
- kind "memory": a fact to keep in mind (e.g. "The user's name is Ada", "The main repo is at ~/work/app").

Write the content as one short, self-contained statement. Do not add an item that duplicates or overlaps an existing one; use updateMemory to change an existing item instead. Do not save things the user did not ask to keep, or one-off details of the current task."#;

/// `addMemory`.
pub struct AddMemoryTool {
    service: Arc<MemoryService>,
}

#[async_trait]
impl Tool for AddMemoryTool {
    fn name(&self) -> &str {
        ADD_NAME
    }
    fn description(&self) -> &str {
        ADD_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Thinking
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            ADD_NAME,
            ADD_SPEC_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The rule or fact to remember, as one short self-contained statement."
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["rule", "memory"],
                        "description": "\"rule\" for an instruction on how to behave, \"memory\" for a fact. Defaults to \"memory\"."
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        content_issue(&mut issues, input.get("content"), false);
        kind_issue(&mut issues, input.get("kind"));
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let content = input["content"].as_str().unwrap_or_default();
        let wanted = normalized(content);
        if let Some(existing) = self
            .service
            .list()
            .into_iter()
            .find(|i| normalized(&i.content) == wanted)
        {
            return Err(ToolError::plain(format!(
                "Failed to add memory: an identical item already exists (id \"{}\"). Use updateMemory to change it.",
                existing.id
            )));
        }
        let kind = kind(&input).unwrap_or_default();
        let item = self
            .service
            .add(content, kind, MemorySource::Chat)
            .map_err(|e| ToolError::plain(format!("Failed to add memory: {e}")))?;
        let label = match item.kind {
            MemoryKind::Rule => "rule",
            MemoryKind::Memory => "memory",
        };
        let message = format!("Saved {label}: {}", item.content);
        Ok(output(ADD_NAME, &item, message))
    }
}

const UPDATE_NAME: &str = "updateMemory";
const UPDATE_DESCRIPTION: &str = "Change, enable or disable an item in the memory list";
const UPDATE_SPEC_DESCRIPTION: &str = r#"Update an existing item in the user's global memory list by its id.

Use this when the user corrects or refines something already remembered, or asks to turn an item on or off. Only the fields you pass are changed. Prefer this over adding a new item when the new information replaces an existing one."#;

/// `updateMemory`.
pub struct UpdateMemoryTool {
    service: Arc<MemoryService>,
}

#[async_trait]
impl Tool for UpdateMemoryTool {
    fn name(&self) -> &str {
        UPDATE_NAME
    }
    fn description(&self) -> &str {
        UPDATE_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Thinking
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            UPDATE_NAME,
            UPDATE_SPEC_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The ID of the memory item to update" },
                    "content": { "type": "string", "description": "New text for the item" },
                    "kind": {
                        "type": "string",
                        "enum": ["rule", "memory"],
                        "description": "\"rule\" for an instruction on how to behave, \"memory\" for a fact"
                    },
                    "enabled": { "type": "boolean", "description": "Whether the item is included in future chats" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        id_issue(&mut issues, input.get("id"));
        content_issue(&mut issues, input.get("content"), true);
        kind_issue(&mut issues, input.get("kind"));
        match input.get("enabled") {
            None | Some(Value::Bool(_)) => {}
            other => issues.invalid_type(&["enabled"], "boolean", other),
        }
        if ["content", "kind", "enabled"]
            .iter()
            .all(|k| input.get(*k).is_none())
        {
            issues
                .0
                .push("Provide at least one of content, kind or enabled".to_string());
        }
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"].as_str().unwrap_or_default();
        let patch = MemoryPatch {
            content: input
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string),
            kind: kind(&input),
            enabled: input.get("enabled").and_then(Value::as_bool),
        };
        let item = self
            .service
            .update(id, patch)
            .map_err(|e| ToolError::plain(format!("Failed to update memory: {e}")))?;
        let message = format!("Updated memory: {}", item.content);
        Ok(output(UPDATE_NAME, &item, message))
    }
}

const REMOVE_NAME: &str = "removeMemory";
const REMOVE_DESCRIPTION: &str = "Delete an item from the memory list";
const REMOVE_SPEC_DESCRIPTION: &str = r#"Delete an item from the user's global memory list by its id.

Use this when the user asks you to forget something or to drop a rule. To pause an item without deleting it, use updateMemory with enabled: false instead."#;

/// `removeMemory`.
pub struct RemoveMemoryTool {
    service: Arc<MemoryService>,
}

#[async_trait]
impl Tool for RemoveMemoryTool {
    fn name(&self) -> &str {
        REMOVE_NAME
    }
    fn description(&self) -> &str {
        REMOVE_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Thinking
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            REMOVE_NAME,
            REMOVE_SPEC_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The ID of the memory item to delete" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        id_issue(&mut issues, input.get("id"));
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let id = input["id"].as_str().unwrap_or_default();
        let item = self
            .service
            .delete(id)
            .map_err(|e| ToolError::plain(format!("Failed to remove memory: {e}")))?
            .ok_or_else(|| {
                ToolError::plain(format!(
                    "Failed to remove memory: Memory item with ID \"{id}\" not found"
                ))
            })?;
        let message = format!("Removed memory: {}", item.content);
        Ok(output(REMOVE_NAME, &item, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;

    fn setup() -> (tempfile::TempDir, Arc<MemoryService>, Vec<Arc<dyn Tool>>) {
        let dir = tempfile::tempdir().unwrap();
        let service = Arc::new(MemoryService::open(dir.path()));
        let tools = create_memory_tools(service.clone());
        (dir, service, tools)
    }

    async fn run(tool: &Arc<dyn Tool>, input: Value) -> std::result::Result<Value, Value> {
        run_tool(tool.as_ref(), input, &ToolContext::default())
            .await
            .map(ToolOutput::into_value)
            .map_err(|e| serde_json::from_str(&e.message).unwrap())
    }

    #[tokio::test]
    async fn add_update_remove() {
        let (_dir, service, tools) = setup();

        let v = run(
            &tools[0],
            json!({"type": "addMemory", "content": " Answer in Japanese ", "kind": "rule"}),
        )
        .await
        .unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, vec!["name", "success", "result", "message"]);
        assert_eq!(v["name"], "addMemory");
        assert_eq!(v["success"], true);
        assert_eq!(v["message"], "Saved rule: Answer in Japanese");
        let item = &v["result"]["item"];
        assert_eq!(item["content"], "Answer in Japanese");
        assert_eq!(item["kind"], "rule");
        assert_eq!(item["source"], "chat");
        assert_eq!(item["enabled"], true);
        let id = item["id"].as_str().unwrap().to_string();
        assert_eq!(service.list().len(), 1);

        // kind defaults to memory
        let v = run(&tools[0], json!({"content": "Name is Ada"}))
            .await
            .unwrap();
        assert_eq!(v["result"]["item"]["kind"], "memory");

        let v = run(&tools[1], json!({"id": id, "enabled": false}))
            .await
            .unwrap();
        assert_eq!(v["name"], "updateMemory");
        assert_eq!(v["result"]["item"]["id"], id.as_str());
        assert_eq!(v["result"]["item"]["enabled"], false);
        assert_eq!(v["message"], "Updated memory: Answer in Japanese");
        assert!(!service.get(&id).unwrap().enabled);

        let v = run(&tools[2], json!({"id": id})).await.unwrap();
        assert_eq!(v["name"], "removeMemory");
        assert_eq!(v["result"]["item"]["id"], id.as_str());
        assert_eq!(v["message"], "Removed memory: Answer in Japanese");
        assert!(service.get(&id).is_none());
    }

    #[tokio::test]
    async fn execution_errors() {
        let (_dir, _service, tools) = setup();
        run(&tools[0], json!({"content": "Use pnpm"}))
            .await
            .unwrap();

        let e = run(&tools[0], json!({"content": "use  PNPM"}))
            .await
            .unwrap_err();
        assert_eq!(e["type"], "EXECUTION");
        assert!(e["error"]
            .as_str()
            .unwrap()
            .starts_with("Failed to add memory: an identical item already exists"));

        let e = run(&tools[1], json!({"id": "zzz", "content": "x"}))
            .await
            .unwrap_err();
        assert_eq!(
            e["error"],
            "Failed to update memory: Memory item with ID \"zzz\" not found"
        );

        let e = run(&tools[2], json!({"id": "zzz"})).await.unwrap_err();
        assert_eq!(
            e["error"],
            "Failed to remove memory: Memory item with ID \"zzz\" not found"
        );
    }

    #[test]
    fn validation() {
        let (_dir, _service, tools) = setup();
        assert_eq!(
            tools[0].validate_input(&json!({"kind": "fact"})),
            vec![
                "content: Required",
                "kind: Invalid enum value. Expected 'rule' | 'memory', received 'fact'"
            ]
        );
        assert_eq!(
            tools[0].validate_input(&json!({"content": "  ", "kind": 1})),
            vec![
                "content: String must contain at least 1 character(s)",
                "kind: Expected 'rule' | 'memory', received number"
            ]
        );
        assert!(tools[0]
            .validate_input(&json!({"content": "x", "kind": "memory"}))
            .is_empty());

        assert_eq!(
            tools[1].validate_input(&json!({"enabled": "yes"})),
            vec!["id: Required", "enabled: Expected boolean, received string"]
        );
        assert_eq!(
            tools[1].validate_input(&json!({"id": "a"})),
            vec!["Provide at least one of content, kind or enabled"]
        );
        assert!(tools[1]
            .validate_input(&json!({"id": "a", "kind": "rule"}))
            .is_empty());

        assert_eq!(
            tools[2].validate_input(&json!({"id": ""})),
            vec!["id: String must contain at least 1 character(s)"]
        );
        assert_eq!(tools[2].validate_input(&json!({})), vec!["id: Required"]);
    }
}
