//! Port of `src/main/api/bedrock/services/subAgent/SubAgentRunner.test.ts`, test for test.

use super::*;
use crate::engine::ToolExecution;
use crate::session::AgentMessage;
use serde_json::json;
use tokio::sync::Semaphore;

type ChatFn = dyn Fn() -> Option<Result<ChatResult>> + Send + Sync;

/// `makeService()`: records every call; `chat` waits on `gate` (if any) then returns `chat_fn()`.
struct MockService {
    agents: Vec<Value>,
    calls: Mutex<Vec<String>>,
    chat_args: Mutex<Vec<(String, AgentRunConfig, String, AgentRunOptions)>>,
    deleted: Mutex<Vec<String>>,
    gate: Option<Arc<Semaphore>>,
    chat_fn: Box<ChatFn>,
}

fn chat_result(text: &str, tool_count: usize) -> ChatResult {
    ChatResult {
        response: AgentMessage {
            id: "m1".into(),
            role: "assistant".into(),
            content: vec![json!({ "text": text })],
            timestamp: 0,
            metadata: None,
        },
        tool_executions: Some(
            (0..tool_count)
                .map(|i| ToolExecution {
                    tool_name: if i % 2 == 0 { "readFiles" } else { "listFiles" }.into(),
                    input: json!({}),
                    output: json!({}),
                    success: true,
                    error: None,
                })
                .collect(),
        ),
    }
}

impl MockService {
    fn new() -> Self {
        MockService {
            agents: vec![
                json!({ "id": "reviewerAgent", "name": "Reviewer", "icon": "robot" }),
                json!({ "id": "writerAgent", "name": "Writer" }),
            ],
            calls: Mutex::new(Vec::new()),
            chat_args: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
            gate: None,
            chat_fn: Box::new(|| Some(Ok(chat_result("done", 0)))),
        }
    }

    fn with_chat(
        mut self,
        f: impl Fn() -> Option<Result<ChatResult>> + Send + Sync + 'static,
    ) -> Self {
        self.chat_fn = Box::new(f);
        self
    }

    fn with_gate(mut self, gate: Arc<Semaphore>) -> Self {
        self.gate = Some(gate);
        self
    }

    fn calls(&self, name: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == name)
            .count()
    }
}

#[async_trait]
impl AgentService for MockService {
    async fn get_all_agents(&self) -> Vec<Value> {
        self.agents.clone()
    }
    async fn create_session(&self, _session_id: &str, _meta: SessionMeta) -> Result<()> {
        self.calls.lock().unwrap().push("createSession".into());
        Ok(())
    }
    fn delete_session(&self, session_id: &str) -> bool {
        self.calls.lock().unwrap().push("deleteSession".into());
        self.deleted.lock().unwrap().push(session_id.to_string());
        true
    }
    async fn chat(
        &self,
        session_id: &str,
        config: AgentRunConfig,
        user_message: String,
        options: AgentRunOptions,
    ) -> Result<ChatResult> {
        self.calls.lock().unwrap().push("chat".into());
        self.chat_args.lock().unwrap().push((
            session_id.to_string(),
            config,
            user_message,
            options,
        ));
        if let Some(gate) = &self.gate {
            let _permit = gate.acquire().await.unwrap();
        }
        (self.chat_fn)().expect("chat result")
    }
}

fn params() -> SubAgentInvokeParams {
    SubAgentInvokeParams {
        agent_id: "reviewerAgent".into(),
        task: "Review src/main".into(),
        caller_agent_id: Some("callerAgent".into()),
        depth: 0,
        lineage: vec!["callerAgent".into()],
        allowed_agent_ids: vec!["reviewerAgent".into()],
        model_id: Some("test-model".into()),
        ..Default::default()
    }
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn make_runner(service: Arc<MockService>) -> SubAgentRunner {
    // store.get('llm') -> { modelId: 'test-model' }
    SubAgentRunner::new(
        service,
        Arc::new(|| json!({ "llm": { "modelId": "test-model" } })),
    )
}

fn err_text(r: Result<SubAgentInvokeResult>) -> String {
    r.unwrap_err().to_string()
}

// describe('SubAgentRunner policy checks')

#[tokio::test]
async fn rejects_an_agent_that_was_not_mentioned_without_starting_a_run() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.agent_id = "writerAgent".into();

    let e = err_text(make_runner(service.clone()).invoke(p).await);
    assert!(e.contains("not permitted"), "{e}");

    assert_eq!(service.calls("chat"), 0);
    assert_eq!(service.calls("createSession"), 0);
}

#[tokio::test]
async fn rejects_a_caller_already_at_the_depth_limit() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.depth = MAX_DELEGATION_DEPTH;

    let e = err_text(make_runner(service.clone()).invoke(p).await);
    assert!(e.contains("depth limit"), "{e}");

    assert_eq!(service.calls("chat"), 0);
}

#[tokio::test]
async fn rejects_a_target_already_in_the_caller_chain() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.lineage = ids(&["callerAgent", "reviewerAgent"]);
    p.allowed_agent_ids = ids(&["reviewerAgent", "writerAgent"]);

    let e = err_text(make_runner(service.clone()).invoke(p).await);
    assert!(e.contains("loop blocked"), "{e}");

    assert_eq!(service.calls("chat"), 0);
}

#[tokio::test]
async fn rejects_an_allowlisted_agent_that_no_longer_exists() {
    let mut s = MockService::new();
    s.agents = Vec::new();
    let service = Arc::new(s);

    let e = err_text(make_runner(service.clone()).invoke(params()).await);
    assert!(e.contains("Agent not found"), "{e}");
    assert_eq!(service.calls("chat"), 0);
}

// describe('SubAgentRunner happy path')

#[tokio::test]
async fn creates_the_session_before_chatting_and_deletes_it_afterwards() {
    let service = Arc::new(MockService::new().with_chat(|| Some(Ok(chat_result("reviewed", 3)))));

    let result = make_runner(service.clone()).invoke(params()).await.unwrap();

    assert_eq!(
        *service.calls.lock().unwrap(),
        ids(&["createSession", "chat", "deleteSession"])
    );
    assert!(result.success);
    assert_eq!(result.agent_name, "Reviewer");
    assert_eq!(result.final_text, "reviewed");
    assert_eq!(result.tool_call_count, 3);
    assert_eq!(result.tool_names, ids(&["readFiles", "listFiles"]));
    assert_eq!(result.depth, 1);
    assert_eq!(result.stopped_reason, StoppedReason::Completed);
}

#[tokio::test]
async fn passes_depth_lineage_and_the_allowlist_down_to_the_sub_agent() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.allowed_agent_ids = ids(&["reviewerAgent", "writerAgent"]);

    make_runner(service.clone()).invoke(p).await.unwrap();

    let config = service.chat_args.lock().unwrap()[0].1.clone();
    assert_eq!(config.delegation_depth, Some(1));
    assert_eq!(
        config.delegation_lineage,
        Some(ids(&["callerAgent", "reviewerAgent"]))
    );
    assert_eq!(
        config.allowed_delegation_agent_ids,
        Some(ids(&["reviewerAgent", "writerAgent"]))
    );
}

#[tokio::test]
async fn caps_the_sub_agent_tool_budget_rather_than_inheriting_the_engine_default() {
    let service = Arc::new(MockService::new());

    make_runner(service.clone()).invoke(params()).await.unwrap();

    let options = service.chat_args.lock().unwrap()[0].3.clone();
    assert_eq!(options.max_tool_executions, 30);
    assert_eq!(options.timeout, Duration::from_millis(10 * 60 * 1000));
}

#[tokio::test]
async fn reports_a_partial_answer_when_the_tool_budget_was_exhausted() {
    let service = Arc::new(MockService::new().with_chat(|| Some(Ok(chat_result("partial", 2)))));
    let mut p = params();
    p.options = Some(tools::SubAgentOptions {
        max_tool_executions: Some(2),
        timeout_ms: None,
    });

    let result = make_runner(service).invoke(p).await.unwrap();

    assert!(result.success);
    assert_eq!(result.stopped_reason, StoppedReason::MaxToolExecutions);
}

#[tokio::test]
async fn substitutes_a_placeholder_when_the_sub_agent_produced_no_text() {
    let service = Arc::new(MockService::new().with_chat(|| {
        Some(Ok(ChatResult {
            response: AgentMessage {
                id: "m1".into(),
                role: "assistant".into(),
                content: vec![],
                timestamp: 0,
                metadata: None,
            },
            tool_executions: Some(vec![]),
        }))
    }));

    let result = make_runner(service).invoke(params()).await.unwrap();
    assert!(result.final_text.contains("no text output"));
}

#[tokio::test]
async fn deletes_the_session_even_when_the_run_fails() {
    let service = Arc::new(
        MockService::new().with_chat(|| Some(Err(Error::Converse("bedrock down".into())))),
    );

    let e = err_text(make_runner(service.clone()).invoke(params()).await);
    assert_eq!(e, "bedrock down");
    assert_eq!(service.calls("deleteSession"), 1);
}

// describe('SubAgentRunner concurrency')

#[tokio::test]
async fn rejects_once_the_concurrency_cap_is_reached() {
    let gate = Arc::new(Semaphore::new(0));
    let service = Arc::new(MockService::new().with_gate(gate.clone()));
    let runner = Arc::new(make_runner(service.clone()));

    let in_flight: Vec<_> = (0..SUB_AGENT_MAX_CONCURRENT)
        .map(|_| {
            let r = runner.clone();
            tokio::spawn(async move { r.invoke(params()).await })
        })
        .collect();
    // Let each in-flight call get past its own concurrency check
    while service.calls("chat") < SUB_AGENT_MAX_CONCURRENT {
        tokio::task::yield_now().await;
    }

    let e = err_text(runner.invoke(params()).await);
    assert!(e.contains("Too many concurrent delegations"), "{e}");

    gate.add_permits(SUB_AGENT_MAX_CONCURRENT);
    for h in in_flight {
        h.await.unwrap().unwrap();
    }
    assert_eq!(runner.active_count(), 0);
}

// describe('SubAgentRunner timeout')

#[tokio::test]
async fn fails_with_a_timeout_error_and_cleans_up_when_the_orphaned_run_settles() {
    let gate = Arc::new(Semaphore::new(0));
    let service = Arc::new(MockService::new().with_gate(gate.clone()));
    let runner = make_runner(service.clone());
    let mut p = params();
    p.options = Some(tools::SubAgentOptions {
        max_tool_executions: None,
        timeout_ms: Some(20),
    });

    let e = err_text(runner.invoke(p).await);
    assert!(e.contains("timed out"), "{e}");

    // The abandoned run is skipped in `finally`, then cleaned up once it settles
    assert_eq!(service.calls("deleteSession"), 0);

    gate.add_permits(1);
    for _ in 0..200 {
        if service.calls("deleteSession") > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let deleted = service.deleted.lock().unwrap().clone();
    assert_eq!(deleted.len(), 1);
    assert!(deleted[0].contains("subagent-"));
}

// Rust-side additions

#[test]
fn prompt_contains_only_non_empty_sections() {
    let mut p = params();
    p.task = "  Do it  ".into();
    assert_eq!(build_prompt(&p), "Do it");
    p.context = Some(" files: a.rs ".into());
    p.expected_output = Some("   ".into());
    assert_eq!(build_prompt(&p), "Do it\n\n## Context\n\nfiles: a.rs");
    p.expected_output = Some("A list".into());
    assert!(build_prompt(&p).ends_with("\n\n## Expected output\n\nA list"));
}

#[tokio::test]
async fn session_id_names_caller_and_target_and_prompt_is_sent() {
    let service = Arc::new(MockService::new());
    let result = make_runner(service.clone()).invoke(params()).await.unwrap();
    assert!(result
        .session_id
        .starts_with("subagent-callerAgent-reviewerAgent-"));
    assert!(result.session_id.ends_with("-0"));
    assert_eq!(result.agent_icon.as_deref(), Some("robot"));
    let (sid, _, prompt, _) = service.chat_args.lock().unwrap()[0].clone();
    assert_eq!(sid, result.session_id);
    assert_eq!(prompt, "Review src/main");
}

#[tokio::test]
async fn falls_back_to_the_configured_model_and_fails_without_one() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.model_id = None;
    make_runner(service.clone())
        .invoke(p.clone())
        .await
        .unwrap();
    assert_eq!(
        service.chat_args.lock().unwrap()[0].1.model_id,
        "test-model"
    );

    let runner = SubAgentRunner::new(service.clone(), Arc::new(|| json!({})));
    let e = err_text(runner.invoke(p).await);
    assert!(e.contains("No model is configured"), "{e}");
    assert_eq!(runner.active_count(), 0);
}

#[tokio::test]
async fn no_permitted_agents_is_reported_below_the_depth_limit() {
    let service = Arc::new(MockService::new());
    let mut p = params();
    p.allowed_agent_ids = vec![];
    let e = err_text(make_runner(service).invoke(p).await);
    assert!(e.contains("The user must @mention an agent first"), "{e}");
}

#[tokio::test]
async fn truncates_very_long_final_text() {
    let long = "x".repeat(MAX_DELEGATION_RESULT_CHARS + 10);
    let service = Arc::new(MockService::new().with_chat(move || Some(Ok(chat_result(&long, 0)))));
    let result = make_runner(service).invoke(params()).await.unwrap();
    assert_eq!(result.final_text.len(), MAX_DELEGATION_RESULT_CHARS);
    assert_eq!(result.truncated, Some(true));
}
