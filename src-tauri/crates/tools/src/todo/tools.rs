//! Ports of `TodoInitTool.ts` / `TodoUpdateTool.ts` and the `todo-*` IPC handlers.

use super::session::{TodoItemUpdate, TodoSessionManager, TodoUpdateResult};
use crate::base::Tool;
use crate::context::{ToolContext, ToolSettings};
use crate::error::{Result, ToolError};
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use crate::util::js;
use crate::util::validate::{zod_type, Issues};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Lazily-created [`TodoSessionManager`] shared by the todo tools and the app's
/// `todo-*` commands (`getTodoSessionManager()` in TS).
#[derive(Default)]
pub struct TodoService {
    manager: Mutex<Option<Arc<TodoSessionManager>>>,
}

impl TodoService {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getTodoSessionManager()`.
    pub fn manager(
        &self,
        user_data_path: Option<&str>,
    ) -> std::result::Result<Arc<TodoSessionManager>, String> {
        let mut guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = guard.as_ref() {
            return Ok(m.clone());
        }
        let m = Arc::new(TodoSessionManager::new(user_data_path)?);
        *guard = Some(m.clone());
        Ok(m)
    }

    /// `todo-init` handler: `{ success, result, message }` / `{ success: false, error }`.
    pub async fn init_todo_list(
        &self,
        settings: &ToolSettings,
        session_id: &str,
        items: &[String],
    ) -> Value {
        let manager = match self.manager(settings.user_data_path.as_deref()) {
            Ok(m) => m,
            Err(e) => return json!({ "success": false, "error": e }),
        };
        let project_path = settings.project_path.clone().unwrap_or_else(home_dir);
        let list = manager
            .create_todo_list(session_id, &project_path, items)
            .await;
        let n = list.items.len();
        json!({
            "success": true,
            "result": list,
            "message": format!("Todo list initialized with {n} tasks")
        })
    }

    /// `todo-update` handler: a `TodoUpdateResult`.
    pub async fn update_todo_list(
        &self,
        settings: &ToolSettings,
        session_id: &str,
        updates: &[TodoItemUpdate],
    ) -> Value {
        let manager = match self.manager(settings.user_data_path.as_deref()) {
            Ok(m) => m,
            Err(e) => return json!({ "success": false, "error": e }),
        };
        let r: TodoUpdateResult = manager.update_todo_list(session_id, updates).await;
        serde_json::to_value(r).unwrap_or(Value::Null)
    }
}

/// `os.homedir()`.
fn home_dir() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default()
}

/// `createTodoTools()` sharing one [`TodoService`].
pub fn create_todo_tools(service: Arc<TodoService>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(TodoInitTool {
            service: service.clone(),
        }),
        Arc::new(TodoUpdateTool { service }),
    ]
}

fn literal(issues: &mut Issues, input: &Value, expected: &str) {
    if input.get("type").and_then(Value::as_str) != Some(expected) {
        issues.push(
            &["type"],
            format!("Invalid literal value, expected \"{expected}\""),
        );
    }
}

fn error_text(result: &Value, fallback: &str) -> String {
    match result.get("error") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => fallback.to_string(),
    }
}

const INIT_NAME: &str = "todoInit";
const INIT_DESCRIPTION: &str =
    "Initialize or replace a todo list for systematic workflow management";
const INIT_SPEC_DESCRIPTION: &str = r#"Establish a fresh todo list or overwrite the current one.

This utility enables systematic workflow management during development sessions, facilitating progress monitoring and task coordination while providing transparency to users regarding work completion status.

## Optimal Usage Scenarios

Deploy this functionality strategically under these conditions:

1. Intricate workflows requiring multiple phases - Apply when operations demand 3+ sequential actions or procedures
2. Sophisticated assignments needing orchestration - Utilize for endeavors requiring methodical coordination or compound operations
3. Direct user specification for task tracking - Activate when users explicitly request task list functionality
4. Multiple assignment batches - Engage when users present enumerated or delimited work items
5. Upon instruction receipt - Promptly document user specifications as actionable items. Modify todo list as new details emerge.
6. Following task completion - Update status and incorporate subsequent follow-up activities
7. During task initiation - Transition items to active status. Maintain singular active task focus. Finalize current work before advancing to new items.

## Inappropriate Usage Contexts

Avoid this utility when:
1. Only one straightforward operation exists
2. Work is elementary and tracking offers no structural advantage
3. Completion requires fewer than 3 basic steps
4. Interaction is purely discussion-based or informational

IMPORTANT: Refrain from using this tool for single elementary tasks. Direct execution is more efficient in such cases.

## Task Status Management and Workflow

1. **Status Categories**: Utilize these states for progress tracking:
   - pending: Task awaiting initiation
   - in_progress: Currently active (maintain ONE active task maximum)
   - completed: Task successfully finished
   - cancelled: Task no longer required

2. **Workflow Management**:
   - Update task status continuously during work
   - Mark tasks complete IMMEDIATELY upon finishing (avoid batching completions)
   - Maintain only ONE task in_progress simultaneously
   - Complete current tasks before initiating new ones
   - Cancel tasks that become obsolete

3. **Task Organization**:
   - Generate specific, actionable items
   - Decompose complex tasks into smaller, manageable components
   - Employ clear, descriptive task naming

When uncertain, deploy this tool. Proactive task management demonstrates diligence and ensures comprehensive requirement fulfillment."#;

/// `TodoInitTool`.
pub struct TodoInitTool {
    service: Arc<TodoService>,
}

#[async_trait]
impl Tool for TodoInitTool {
    fn name(&self) -> &str {
        INIT_NAME
    }
    fn description(&self) -> &str {
        INIT_DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Thinking
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            INIT_NAME,
            INIT_SPEC_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "todoInit" },
                    "items": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1,
                        "description": "Array of task descriptions to initialize the list with. All tasks are initially marked as pending."
                    }
                },
                "required": ["type", "items"],
                "additionalProperties": false
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        literal(&mut issues, input, INIT_NAME);
        match input.get("items") {
            Some(Value::Array(a)) => {
                for (i, v) in a.iter().enumerate() {
                    if !v.is_string() {
                        let idx = i.to_string();
                        issues.invalid_type(&["items", &idx], "string", Some(v));
                    }
                }
                if a.is_empty() {
                    issues.push(&["items"], "Array must contain at least 1 element(s)");
                }
            }
            other => issues.invalid_type(&["items"], "array", other),
        }
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let items: Vec<String> = input["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("session_{}", js::now_millis()));
        let result = self
            .service
            .init_todo_list(&ctx.settings, &session_id, &items)
            .await;
        if result["success"] == json!(true) {
            let mut out = serde_json::Map::new();
            out.insert("name".into(), json!(INIT_NAME));
            out.insert("success".into(), json!(true));
            out.insert("result".into(), result["result"].clone());
            if let Some(m) = result.get("message") {
                out.insert("message".into(), m.clone());
            }
            Ok(ToolOutput::Json(Value::Object(out)))
        } else {
            Err(ToolError::plain(format!(
                "Failed to initialize todo list: {}",
                error_text(&result, "Failed to initialize todo list")
            )))
        }
    }
}

const UPDATE_NAME: &str = "todoUpdate";
const UPDATE_DESCRIPTION: &str = "Update tasks in the todo list (status, description)";
const UPDATE_SPEC_DESCRIPTION: &str = r#"Update tasks in the todo list created by todoInit.

Use this to mark tasks as completed, in progress, or to modify task descriptions.
Provide an array of updates to process multiple tasks at once.

## Usage Examples

Update single task status:
{
  "type": "todoUpdate",
  "updates": [
    {
      "id": "task-1703123456789-abc123def",
      "status": "completed"
    }
  ]
}

Update multiple tasks:
{
  "type": "todoUpdate",
  "updates": [
    {
      "id": "task-1703123456789-abc123def",
      "status": "completed"
    },
    {
      "id": "task-1703123456790-def456ghi",
      "status": "in_progress"
    }
  ]
}

Update task description:
{
  "type": "todoUpdate",
  "updates": [
    {
      "id": "task-1703123456789-abc123def",
      "description": "Updated task description"
    }
  ]
}

## Status Values

- pending: Task awaiting initiation
- in_progress: Currently active (maintain ONE active task maximum)
- completed: Task successfully finished
- cancelled: Task no longer required

## Best Practices

1. Mark tasks complete IMMEDIATELY upon finishing
2. Maintain only ONE task in_progress simultaneously
3. Complete current tasks before starting new ones
4. Cancel tasks that become obsolete

If your update request is invalid, an error will be returned with the current todo list state."#;

/// `TodoUpdateTool`.
pub struct TodoUpdateTool {
    service: Arc<TodoService>,
}

#[async_trait]
impl Tool for TodoUpdateTool {
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
                    "type": { "type": "string", "const": "todoUpdate" },
                    "updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "The ID of the task to update" },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "The new status for the task"
                                },
                                "description": { "type": "string", "description": "Optional new description for the task" }
                            },
                            "required": ["id"],
                            "additionalProperties": false
                        },
                        "minItems": 1,
                        "description": "Array of task updates to process in batch"
                    }
                },
                "required": ["type", "updates"],
                "additionalProperties": false
            }),
        ))
    }

    fn validate_input(&self, input: &Value) -> Vec<String> {
        let mut issues = Issues::default();
        literal(&mut issues, input, UPDATE_NAME);
        let expected_enum = "'pending' | 'in_progress' | 'completed' | 'cancelled'";
        match input.get("updates") {
            Some(Value::Array(a)) => {
                for (i, u) in a.iter().enumerate() {
                    let idx = i.to_string();
                    let Some(o) = u.as_object() else {
                        issues.invalid_type(&["updates", &idx], "object", Some(u));
                        continue;
                    };
                    issues.string(&["updates", &idx, "id"], o.get("id"), false);
                    match o.get("status") {
                        None => {}
                        Some(Value::String(s)) => {
                            if !super::session::TodoItemStatus::ALL.contains(&s.as_str()) {
                                issues.push(
                                    &["updates", &idx, "status"],
                                    format!("Invalid enum value. Expected {expected_enum}, received '{s}'"),
                                );
                            }
                        }
                        Some(other) => issues.push(
                            &["updates", &idx, "status"],
                            format!("Expected {expected_enum}, received {}", zod_type(other)),
                        ),
                    }
                    issues.string(
                        &["updates", &idx, "description"],
                        o.get("description"),
                        true,
                    );
                }
                if a.is_empty() {
                    issues.push(&["updates"], "Array must contain at least 1 element(s)");
                }
            }
            other => issues.invalid_type(&["updates"], "array", other),
        }
        issues.into_vec()
    }

    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let updates: Vec<TodoItemUpdate> = serde_json::from_value(input["updates"].clone())
            .map_err(|e| ToolError::plain(format!("Failed to update todo tasks: {e}")))?;
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "most_recent".to_string());
        let result = self
            .service
            .update_todo_list(&ctx.settings, &session_id, &updates)
            .await;
        if result["success"] == json!(true) {
            let n = updates.len();
            let mut out = serde_json::Map::new();
            out.insert("name".into(), json!(UPDATE_NAME));
            out.insert("success".into(), json!(true));
            out.insert(
                "result".into(),
                result.get("updatedList").cloned().unwrap_or(Value::Null),
            );
            out.insert(
                "message".into(),
                json!(format!(
                    "Updated {n} task{} successfully",
                    if n > 1 { "s" } else { "" }
                )),
            );
            Ok(ToolOutput::Json(Value::Object(out)))
        } else {
            Err(ToolError::plain(format!(
                "Failed to update todo tasks: {}",
                error_text(&result, "Failed to update todo tasks")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::run_tool;

    fn ctx(dir: &tempfile::TempDir, session: Option<&str>) -> ToolContext {
        let store = json!({"userDataPath": dir.path().to_string_lossy(), "projectPath": "/proj"});
        ToolContext::from_store(&store, session.map(str::to_string))
    }

    #[tokio::test]
    async fn init_then_update() {
        let dir = tempfile::tempdir().unwrap();
        let service = Arc::new(TodoService::new());
        let tools = create_todo_tools(service.clone());
        let c = ctx(&dir, Some("session_42"));

        let out = run_tool(
            tools[0].as_ref(),
            json!({"type": "todoInit", "items": ["a", "b"]}),
            &c,
        )
        .await
        .unwrap();
        let v = out.into_value();
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, vec!["name", "success", "result", "message"]);
        assert_eq!(v["message"], "Todo list initialized with 2 tasks");
        assert_eq!(v["result"]["sessionId"], "session_42");
        assert_eq!(v["result"]["projectPath"], "/proj");
        let id = v["result"]["items"][1]["id"].as_str().unwrap().to_string();

        let out = run_tool(
            tools[1].as_ref(),
            json!({"type": "todoUpdate", "updates": [{"id": id, "status": "in_progress"}]}),
            &c,
        )
        .await
        .unwrap();
        let v = out.into_value();
        assert_eq!(v["message"], "Updated 1 task successfully");
        assert_eq!(v["result"]["items"][1]["status"], "in_progress");

        let err = run_tool(
            tools[1].as_ref(),
            json!({"type": "todoUpdate", "updates": [{"id": "zzz"}]}),
            &c,
        )
        .await
        .unwrap_err();
        let e: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(
            e["error"],
            "Failed to update todo tasks: Task with ID \"zzz\" not found"
        );
        assert_eq!(e["type"], "EXECUTION");

        // The app's get-todo-list command sees the same manager.
        let m = service.manager(None).unwrap();
        assert_eq!(m.get_todo_list("session_42").unwrap().items.len(), 2);
    }

    #[tokio::test]
    async fn missing_user_data_path() {
        let tools = create_todo_tools(Arc::new(TodoService::new()));
        let err = run_tool(
            tools[0].as_ref(),
            json!({"type": "todoInit", "items": ["a"]}),
            &ToolContext::default(),
        )
        .await
        .unwrap_err();
        let e: Value = serde_json::from_str(&err.message).unwrap();
        assert_eq!(
            e["error"],
            "Failed to initialize todo list: userDataPath is not set in store"
        );
    }

    #[test]
    fn zod_validation() {
        let tools = create_todo_tools(Arc::new(TodoService::new()));
        assert_eq!(
            tools[0].validate_input(&json!({"type": "todoInit", "items": []})),
            vec!["items: Array must contain at least 1 element(s)"]
        );
        assert_eq!(
            tools[0].validate_input(&json!({"items": [1]})),
            vec![
                "type: Invalid literal value, expected \"todoInit\"",
                "items.0: Expected string, received number"
            ]
        );
        assert_eq!(
            tools[1].validate_input(&json!({"type": "todoUpdate", "updates": [{"status": "done"}]})),
            vec![
                "updates.0.id: Required",
                "updates.0.status: Invalid enum value. Expected 'pending' | 'in_progress' | 'completed' | 'cancelled', received 'done'"
            ]
        );
        assert_eq!(
            tools[1].validate_input(&json!({"type": "todoUpdate"})),
            vec!["updates: Required"]
        );
    }
}
