//! One-stop wiring: [`full_registry`] registers every ported tool in the TS
//! `initializeToolSystem()` order, and [`ContextServices`] builds the per-call
//! [`ToolContext`] with the services the tools reach through it.

use crate::agent::SubAgentInvoker;
use crate::bedrock_tools::{create_bedrock_tools, BedrockBackend, SdkBedrockBackend};
use crate::command::CommandSandbox;
use crate::context::{AgentResolver, ToolContext};
use crate::docker_sandbox::{DockerSandboxTool, SandboxOps};
use crate::documents::NativeDocumentReader;
use crate::filesystem::DocumentReader;
use crate::interpreter::CodeInterpreterTool;
use crate::mcp_adapter::{McpExecutor, McpTool};
use crate::registry::ToolRegistry;
use crate::system::{create_system_tools, CameraSource, NoCameraSource, ScreenService};
use crate::{agent, filesystem, thinking, todo, Tool};
use docker::interpreter::CodeInterpreter;
use serde_json::Value;
use std::sync::Arc;

/// Long-lived services the tools are constructed with.
pub struct ToolDeps {
    pub todo: Arc<todo::TodoService>,
    /// `bedrock:*` handlers; [`SdkBedrockBackend`] in production.
    pub bedrock: Arc<dyn BedrockBackend>,
    /// `mcp:executeTool`; an `Arc<mcp::McpManager>` in production.
    pub mcp: Arc<dyn McpExecutor>,
    /// The `codeInterpreter` operations.
    pub interpreter: Arc<CodeInterpreter>,
    /// `docker-sandbox-*` handlers; an `Arc<docker::SandboxManager>` in production.
    pub sandbox: Arc<dyn SandboxOps>,
    /// `screen:*` handlers; [`ScreenService::native`] by default.
    pub screen: Arc<ScreenService>,
    /// Camera frames for cameraCapture; the app replaces the default [`NoCameraSource`] with
    /// its webview capture.
    pub camera: Arc<dyn CameraSource>,
}

impl ToolDeps {
    /// Production wiring from the shared managers. The same `SandboxManager` should also be
    /// passed to [`ContextServices::sandbox`] so `executeCommand` routes into it.
    pub fn new(
        todo: Arc<todo::TodoService>,
        mcp: Arc<mcp::McpManager>,
        interpreter: Arc<CodeInterpreter>,
        sandbox: Arc<docker::SandboxManager>,
    ) -> Self {
        ToolDeps {
            todo,
            bedrock: Arc::new(SdkBedrockBackend::default()),
            mcp,
            interpreter,
            sandbox,
            screen: Arc::new(ScreenService::native()),
            camera: Arc::new(NoCameraSource),
        }
    }
}

/// Every ported tool, in the TS registration order: filesystem, web, thinking, command, mcp,
/// bedrock, interpreter, docker, system, todo, agent.
pub fn all_tools(deps: &ToolDeps) -> Vec<Arc<dyn Tool>> {
    let mut all = filesystem::create_filesystem_tools();
    all.extend(crate::create_web_tools());
    all.extend(thinking::create_thinking_tools());
    all.extend(crate::create_command_tools());
    all.push(Arc::new(McpTool::new(deps.mcp.clone())));
    all.extend(create_bedrock_tools(deps.bedrock.clone()));
    all.push(Arc::new(CodeInterpreterTool::new(deps.interpreter.clone())));
    all.push(Arc::new(DockerSandboxTool::new(deps.sandbox.clone())));
    all.extend(create_system_tools(
        deps.screen.clone(),
        deps.camera.clone(),
        deps.bedrock.clone(),
    ));
    all.extend(todo::create_todo_tools(deps.todo.clone()));
    all.extend(agent::create_agent_tools());
    all
}

/// A registry with [`all_tools`] registered.
pub fn full_registry(deps: &ToolDeps) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register_many(all_tools(deps));
    r
}

/// Per-call services plugged into each [`ToolContext`].
#[derive(Clone)]
pub struct ContextServices {
    /// `findAgentById` (`agents::AgentCatalog`).
    pub agents: Option<Arc<dyn AgentResolver>>,
    /// Docker sandbox command routing (`docker::SandboxManager`).
    pub sandbox: Option<Arc<dyn CommandSandbox>>,
    /// PDF / DOCX extraction; [`NativeDocumentReader`] by default.
    pub documents: Option<Arc<dyn DocumentReader>>,
    /// `invokeAgent` (`agents::SubAgentRunner`).
    pub sub_agents: Option<Arc<dyn SubAgentInvoker>>,
}

impl Default for ContextServices {
    fn default() -> Self {
        ContextServices {
            agents: None,
            sandbox: None,
            documents: Some(Arc::new(NativeDocumentReader)),
            sub_agents: None,
        }
    }
}

impl ContextServices {
    /// The context for one tool call from a store snapshot (`store.all()`) and the chat's
    /// session id.
    pub fn context(&self, store: &Value, session_id: Option<String>) -> ToolContext {
        ToolContext {
            agents: self.agents.clone(),
            sandbox: self.sandbox.clone(),
            documents: self.documents.clone(),
            sub_agents: self.sub_agents.clone(),
            ..ToolContext::from_store(store, session_id)
        }
    }
}
