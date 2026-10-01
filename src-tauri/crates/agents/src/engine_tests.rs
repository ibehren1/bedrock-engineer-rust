//! Agent loop tests with scripted model responses (no TS counterpart: the TS loop only ran
//! against real Bedrock and the renderer's preload).

use crate::*;
use async_trait::async_trait;
use bedrock::ConverseRequest;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tools::{Tool, ToolCategory, ToolContext, ToolOutput, ToolRegistry, ToolSpec};

/// Pops one scripted response per call and records every request.
#[derive(Default)]
struct Scripted {
    responses: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<ConverseRequest>>,
    delay: Option<Duration>,
}

impl Scripted {
    fn new(responses: Vec<Value>) -> Arc<Self> {
        Arc::new(Scripted {
            responses: Mutex::new(responses.into()),
            ..Default::default()
        })
    }
    fn requests(&self) -> Vec<ConverseRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl ConverseBackend for Scripted {
    async fn converse(&self, request: ConverseRequest) -> std::result::Result<Value, String> {
        self.requests.lock().unwrap().push(request);
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "no scripted response left".to_string())
    }
}

fn text_response(text: &str) -> Value {
    json!({
        "output": { "message": { "role": "assistant", "content": [{ "text": text }] } },
        "stopReason": "end_turn",
        "usage": { "inputTokens": 10, "outputTokens": 2, "totalTokens": 12 }
    })
}

fn tool_response(uses: &[(&str, &str, Value)]) -> Value {
    let content: Vec<Value> = uses
        .iter()
        .map(|(id, name, input)| json!({ "toolUse": { "toolUseId": id, "name": name, "input": input } }))
        .collect();
    json!({
        "output": { "message": { "role": "assistant", "content": content } },
        "stopReason": "tool_use"
    })
}

/// A tool that records its input and returns a `ToolResult` (or fails on `fail: true`).
struct Recording {
    name: &'static str,
    seen: Mutex<Vec<Value>>,
    contexts: Mutex<Vec<(Option<String>, tools::CallerMetadata)>>,
}

#[async_trait]
impl Tool for Recording {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "records"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Filesystem
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            self.name,
            "Reads under {{projectPath}}",
            json!({"type": "object"}),
        ))
    }
    fn validate_input(&self, _input: &Value) -> Vec<String> {
        vec![]
    }
    async fn execute_internal(&self, input: Value, ctx: &ToolContext) -> tools::Result<ToolOutput> {
        self.seen.lock().unwrap().push(input.clone());
        self.contexts
            .lock()
            .unwrap()
            .push((ctx.session_id.clone(), ctx.caller.clone()));
        if input.get("fail") == Some(&Value::Bool(true)) {
            return Err(tools::ToolError::plain("disk on fire"));
        }
        if input.get("plain") == Some(&Value::Bool(true)) {
            return Ok(ToolOutput::Text("plain text".into()));
        }
        Ok(ToolOutput::Json(json!({
            "success": true, "name": self.name, "message": "ok", "result": { "path": input["path"] }
        })))
    }
}

#[derive(Default)]
struct Listener {
    published: Mutex<Vec<(String, String)>>,
    history: Mutex<Vec<usize>>,
}

impl SessionListener for Listener {
    fn message_published(&self, session_id: &str, message: &AgentMessage) {
        self.published
            .lock()
            .unwrap()
            .push((session_id.to_string(), message.role.clone()));
    }
    fn history_updated(&self, _session_id: &str, message_count: usize) {
        self.history.lock().unwrap().push(message_count);
    }
}

struct Fixture {
    engine: Arc<AgentEngine>,
    converse: Arc<Scripted>,
    tool: Arc<Recording>,
    sessions: Arc<InMemorySessionStore>,
    listener: Arc<Listener>,
}

fn store_value(agents: Value, extra: Value) -> Value {
    let mut v =
        json!({ "projectPath": "/proj", "customAgents": agents, "llm": { "modelId": "m" } });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    v
}

fn fixture(converse: Arc<Scripted>, agents: Value, extra: Value) -> Fixture {
    let store_json = store_value(agents, extra);
    let store: Arc<dyn StoreReader> = Arc::new(move || store_json.clone());
    let tool = Arc::new(Recording {
        name: "readFiles",
        seen: Mutex::new(Vec::new()),
        contexts: Mutex::new(Vec::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(tool.clone());
    registry.register_many(tools::agent::create_agent_tools());
    let sessions = Arc::new(InMemorySessionStore::new());
    let listener = Arc::new(Listener::default());
    let engine = Arc::new(
        AgentEngine::new(converse.clone(), Arc::new(registry), store)
            .with_sessions(sessions.clone())
            .with_listener(listener.clone()),
    );
    Fixture {
        engine,
        converse,
        tool,
        sessions,
        listener,
    }
}

fn agent(id: &str, tools: &[&str]) -> Value {
    json!({
        "id": id, "name": format!("Agent {id}"), "description": format!("{id} does things"),
        "system": format!("You are {id} in {{{{projectPath}}}}."), "scenarios": [],
        "tools": tools, "mcpServers": [{"name": "srv", "description": ""}]
    })
}

fn config(agent_id: &str) -> AgentRunConfig {
    AgentRunConfig {
        model_id: "m".into(),
        agent_id: agent_id.into(),
        ..Default::default()
    }
}

fn text_of(msg: &Value) -> String {
    msg["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn run_without_tools_sends_system_prompt_and_saves_both_messages() {
    let f = fixture(
        Scripted::new(vec![text_response("hello")]),
        json!([agent("a", &[])]),
        json!({}),
    );
    let result = f
        .engine
        .chat("s1", &config("a"), "Hi", &AgentRunOptions::default())
        .await
        .unwrap();

    assert_eq!(result.response.role, "assistant");
    assert_eq!(result.response.content, vec![json!({"text": "hello"})]);
    assert!(result.tool_executions.is_none());
    assert_eq!(
        result.response.metadata.as_ref().unwrap()["converseMetadata"]["stopReason"],
        "end_turn"
    );

    let req = &f.converse.requests()[0];
    assert_eq!(req.model_id, "m");
    assert!(req.tool_config.is_none());
    let system = req.system.as_ref().unwrap()[0]["text"].as_str().unwrap();
    assert!(system.starts_with("You are a in /proj.\n\n**<context>**"));
    assert_eq!(req.messages.len(), 1);
    assert!(text_of(&req.messages[0]).starts_with("Hi\n<context>\nDate: "));
    assert!(req.messages[0].get("id").is_none());

    assert_eq!(f.sessions.history("s1").len(), 2);
    assert_eq!(
        f.sessions.meta("s1").unwrap().agent_id.as_deref(),
        Some("a")
    );
    let published = f.listener.published.lock().unwrap().clone();
    assert_eq!(
        published,
        vec![
            ("s1".into(), "user".into()),
            ("s1".into(), "assistant".into())
        ]
    );
    assert_eq!(*f.listener.history.lock().unwrap(), vec![1, 2]);
}

#[tokio::test]
async fn tool_loop_runs_tools_with_metadata_and_returns_the_final_answer() {
    let f = fixture(
        Scripted::new(vec![
            tool_response(&[
                (
                    "t1",
                    "readFiles",
                    json!({"path": "a.rs", "_agentId": "spoofed"}),
                ),
                ("t1", "readFiles", json!({"plain": true})),
            ]),
            tool_response(&[("t2", "readFiles", json!({"fail": true}))]),
            text_response("done"),
        ]),
        json!([agent("a", &["readFiles"])]),
        json!({}),
    );
    let result = f
        .engine
        .chat("s1", &config("a"), "Go", &AgentRunOptions::default())
        .await
        .unwrap();

    assert_eq!(result.response.content, vec![json!({"text": "done"})]);
    let execs = result.tool_executions.unwrap();
    assert_eq!(execs.len(), 3);
    assert!(execs[0].success);
    assert_eq!(execs[0].output, json!({"path": "a.rs"}));
    assert_eq!(execs[1].output, json!("plain text"));
    assert!(!execs[2].success);
    assert!(execs[2].error.as_ref().unwrap().contains("disk on fire"));

    // Metadata travels in the context; the model's `_agentId` is stripped from the input.
    let seen = f.tool.seen.lock().unwrap()[0].clone();
    assert_eq!(seen, json!({"type": "readFiles", "path": "a.rs"}));
    let (session, caller) = f.tool.contexts.lock().unwrap()[0].clone();
    assert_eq!(session.as_deref(), Some("s1"));
    assert_eq!(caller.agent_id.as_deref(), Some("a"));
    let servers = caller.mcp_servers.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "srv");
    assert_eq!(caller.delegation_depth, Some(0));
    assert_eq!(caller.delegation_lineage, Some(vec!["a".to_string()]));
    assert_eq!(caller.allowed_agent_ids, Some(vec![]));
    assert_eq!(caller.model_id.as_deref(), Some("m"));

    let reqs = f.converse.requests();
    assert_eq!(reqs.len(), 3);
    // Tool spec description has placeholders replaced.
    let tools_cfg = reqs[0].tool_config.as_ref().unwrap();
    assert_eq!(tools_cfg["tools"][0]["toolSpec"]["name"], "readFiles");
    assert_eq!(
        tools_cfg["tools"][0]["toolSpec"]["description"],
        "Reads under /proj"
    );
    // Second request: user, assistant (deduplicated ids), tool results.
    let second = &reqs[1].messages;
    assert_eq!(second.len(), 3);
    assert_eq!(second[1]["content"][0]["toolUse"]["toolUseId"], "t1");
    assert_eq!(second[1]["content"][1]["toolUse"]["toolUseId"], "t1_1");
    let results = &second[2]["content"];
    assert_eq!(second[2]["role"], "user");
    assert_eq!(results[0]["toolResult"]["toolUseId"], "t1");
    assert_eq!(results[0]["toolResult"]["status"], "success");
    assert_eq!(
        results[0]["toolResult"]["content"][0]["text"],
        r#"{"path":"a.rs"}"#
    );
    assert_eq!(results[1]["toolResult"]["toolUseId"], "t1_1");
    assert_eq!(
        results[1]["toolResult"]["content"][0]["text"],
        r#""plain text""#
    );
    // The system prompt is kept for follow-up calls.
    assert!(reqs[2].system.is_some());
    let failed = &reqs[2].messages[4]["content"][0]["toolResult"];
    assert_eq!(failed["status"], "error");
    assert!(failed["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("disk on fire"));

    // user, tool-use, results, tool-use, results, final
    assert_eq!(f.sessions.history("s1").len(), 6);
    // The first tool-use response is saved but not published.
    let roles: Vec<String> = f
        .listener
        .published
        .lock()
        .unwrap()
        .iter()
        .map(|(_, r)| r.clone())
        .collect();
    assert_eq!(
        roles,
        vec!["user", "user", "assistant", "user", "assistant"]
    );
}

#[tokio::test]
async fn stops_after_the_round_budget_and_returns_the_last_message() {
    let f = fixture(
        Scripted::new(vec![
            tool_response(&[("t1", "readFiles", json!({}))]),
            tool_response(&[("t2", "readFiles", json!({}))]),
            tool_response(&[("t3", "readFiles", json!({}))]),
        ]),
        json!([agent("a", &["readFiles"])]),
        json!({}),
    );
    let options = AgentRunOptions {
        max_tool_executions: 2,
        ..Default::default()
    };
    let result = f
        .engine
        .chat("s", &config("a"), "Go", &options)
        .await
        .unwrap();
    assert_eq!(result.tool_executions.unwrap().len(), 2);
    assert_eq!(result.response.content[0]["toolUse"]["toolUseId"], "t3");
    assert_eq!(f.converse.requests().len(), 3);
}

#[tokio::test]
async fn tool_execution_can_be_disabled() {
    let f = fixture(
        Scripted::new(vec![tool_response(&[("t1", "readFiles", json!({}))])]),
        json!([agent("a", &["readFiles"])]),
        json!({}),
    );
    let options = AgentRunOptions {
        enable_tool_execution: false,
        ..Default::default()
    };
    let result = f
        .engine
        .chat("s", &config("a"), "Go", &options)
        .await
        .unwrap();
    assert!(result.tool_executions.is_none());
    assert!(f.tool.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unknown_agent_and_converse_errors_and_timeout() {
    let f = fixture(Scripted::new(vec![]), json!([agent("a", &[])]), json!({}));
    let e = f
        .engine
        .chat("s", &config("zzz"), "Go", &AgentRunOptions::default())
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Agent not found: zzz");

    let e = f
        .engine
        .chat("s", &config("a"), "Go", &AgentRunOptions::default())
        .await
        .unwrap_err();
    assert_eq!(e, Error::Converse("no scripted response left".into()));

    let slow = Arc::new(Scripted {
        responses: Mutex::new(vec![text_response("late")].into()),
        requests: Mutex::new(Vec::new()),
        delay: Some(Duration::from_millis(200)),
    });
    let f = fixture(slow, json!([agent("a", &[])]), json!({}));
    let options = AgentRunOptions {
        timeout: Duration::from_millis(10),
        ..Default::default()
    };
    let e = f
        .engine
        .chat("s", &config("a"), "Go", &options)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Chat timeout");
}

#[tokio::test]
async fn continues_an_existing_session() {
    let f = fixture(
        Scripted::new(vec![text_response("one"), text_response("two")]),
        json!([agent("a", &[])]),
        json!({}),
    );
    let opts = AgentRunOptions::default();
    f.engine
        .chat("s", &config("a"), "first", &opts)
        .await
        .unwrap();
    f.engine
        .chat("s", &config("a"), "second", &opts)
        .await
        .unwrap();
    let second = &f.converse.requests()[1].messages;
    assert_eq!(second.len(), 3);
    assert_eq!(text_of(&second[1]), "one");
    assert!(f.engine.delete_session("s"));
    assert!(!f.sessions.has_session("s"));
}

#[tokio::test]
async fn prompt_cache_points_are_added_when_enabled() {
    const SONNET_4: &str = "us.anthropic.claude-sonnet-4-20250514-v1:0";
    let f = fixture(
        Scripted::new(vec![text_response("x")]),
        json!([agent("a", &["readFiles"])]),
        json!({ "agentChatConfig": { "enablePromptCache": true } }),
    );
    let mut cfg = config("a");
    cfg.model_id = SONNET_4.into();
    f.engine
        .chat("s", &cfg, "Go", &AgentRunOptions::default())
        .await
        .unwrap();
    let req = &f.converse.requests()[0];
    let has_cp = |blocks: &Value| {
        blocks
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.get("cachePoint").is_some())
    };
    assert!(has_cp(&req.messages[0]["content"]));
    assert!(has_cp(&Value::Array(req.system.clone().unwrap())));
    assert!(has_cp(&req.tool_config.as_ref().unwrap()["tools"]));
}

#[tokio::test]
async fn system_prompt_for_resolves_the_agent() {
    let f = fixture(Scripted::new(vec![]), json!([agent("a", &[])]), json!({}));
    let p = f.engine.system_prompt_for("a", Some("/other")).unwrap();
    assert!(p.starts_with("You are a in /other."));
    assert!(f.engine.system_prompt_for("zzz", None).is_err());
}

/// chat -> A -> B through the real engine and runner: A calls `invokeAgent`, B answers, A
/// finishes. B's tools do not include `invokeAgent` (depth limit).
#[tokio::test]
async fn nested_delegation_through_the_engine() {
    let converse = Scripted::new(vec![
        // A, first call: delegate to B
        tool_response(&[(
            "d1",
            "invokeAgent",
            json!({"agentId": "b", "task": "Summarize", "expectedOutput": "One line"}),
        )]),
        // B: answers directly
        text_response("B says hi"),
        // A, after the tool result
        text_response("A relays: B says hi"),
    ]);
    let f = fixture(
        converse,
        json!([
            agent("a", &["invokeAgent"]),
            agent("b", &["invokeAgent", "readFiles"])
        ]),
        json!({}),
    );
    let store = {
        let v = store_value(json!([]), json!({}));
        Arc::new(move || v.clone()) as Arc<dyn StoreReader>
    };
    let runner = wire_sub_agents(&f.engine, store);

    let result = runner
        .invoke(tools::SubAgentInvokeParams {
            agent_id: "a".into(),
            task: "Ask B".into(),
            caller_agent_id: Some("root".into()),
            depth: 0,
            lineage: vec!["root".into()],
            allowed_agent_ids: vec!["a".into(), "b".into()],
            model_id: Some("m".into()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.final_text, "A relays: B says hi");
    assert_eq!(result.tool_names, vec!["invokeAgent"]);
    assert_eq!(result.depth, 1);

    let reqs = f.converse.requests();
    assert_eq!(reqs.len(), 3);
    // A (depth 1) may delegate to B only.
    let a_tools = &reqs[0].tool_config.as_ref().unwrap()["tools"];
    assert_eq!(a_tools[0]["toolSpec"]["name"], "invokeAgent");
    assert_eq!(
        a_tools[0]["toolSpec"]["inputSchema"]["json"]["properties"]["agentId"]["enum"],
        json!(["b"])
    );
    // B (depth 2) has invokeAgent stripped.
    let b_tools = &reqs[1].tool_config.as_ref().unwrap()["tools"];
    let b_names: Vec<&str> = b_tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["toolSpec"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(b_names, vec!["readFiles"]);
    assert!(text_of(&reqs[1].messages[0])
        .starts_with("Summarize\n\n## Expected output\n\nOne line\n<context>"));
    // A received B's result as a tool result.
    let tool_result = &reqs[2].messages[2]["content"][0]["toolResult"];
    assert_eq!(tool_result["status"], "success");
    let payload: Value =
        serde_json::from_str(tool_result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["finalText"], "B says hi");
    assert_eq!(payload["depth"], 2);
    // Ephemeral sessions are gone.
    assert!(f.sessions.session_ids().is_empty());
    assert_eq!(runner.active_count(), 0);
}
