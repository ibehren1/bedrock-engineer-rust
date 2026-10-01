//! Agent tool system: the [`Tool`] trait, [`ToolRegistry`], and the built-in tools.
//!
//! Port of `src/preload/tools` (registry, `BaseTool`, errors, filesystem / command / web /
//! thinking / todo handlers), `src/main/api/command` (command service, output patterns),
//! `src/preload/lib/gitignore-like-matcher.ts`, and the main-process handlers those tools
//! call (`fetch-website`, `save-website-content`, `todo-*`).
//!
//! Also: the Bedrock tools ([`bedrock_tools`]: generateImage, generateVideo, checkVideoStatus,
//! downloadVideo, recognizeImage, retrieve, invokeBedrockAgent, invokeFlow), the `mcp` adapter
//! ([`mcp_adapter`]), `codeInterpreter` ([`interpreter`]), `dockerSandbox` plus
//! `executeCommand`'s sandbox routing ([`docker_sandbox`]), and PDF / DOCX extraction
//! ([`documents`], the `pdf-*` / `docx-*` handlers), and screenCapture / cameraCapture
//! ([`system`]).
//!
//! # Wiring (app crate)
//!
//! ```ignore
//! let todo = Arc::new(tools::todo::TodoService::new());
//! let mcp = Arc::new(mcp::McpManager::new(reqwest::Client::new()));
//! let settings: Arc<dyn docker::Settings> = /* store lookups: |key| store.get(key) */;
//! let sandbox = Arc::new(docker::SandboxManager::new(settings.clone(), sink));
//! let interpreter = docker::interpreter::CodeInterpreter::new(settings, Arc::new(LogFacadeLogger));
//! let registry = tools::full_registry(&tools::ToolDeps::new(todo, mcp, interpreter, sandbox.clone()));
//! let services = tools::ContextServices {
//!     agents: Some(catalog),          // agents::AgentCatalog
//!     sandbox: Some(sandbox),         // executeCommand -> the chat's sandbox
//!     sub_agents: Some(runner),       // agents::SubAgentRunner
//!     ..Default::default()            // documents: NativeDocumentReader
//! };
//!
//! #[tauri::command]
//! async fn bedrock_execute_tool(registry: State<'_, ToolRegistry>, /* services, store */
//!                               tool_input: Value, context: Option<Value>) -> Result<Value, Value> {
//!     let session_id = context.and_then(|c| c["sessionId"].as_str().map(String::from));
//!     let ctx = services.context(&store.all(), session_id);
//!     registry.execute(tool_input, &ctx).await
//!         .map(ToolOutput::into_value)
//!         .map_err(|e| e.to_js_error()) // { name, message }: rethrow as Error in the shim
//! }
//!
//! #[tauri::command]
//! fn tools_get_tool_specs(registry: State<'_, ToolRegistry>) -> Vec<Value> { registry.tool_specs() }
//! ```
//!
//! [`create_builtin_registry`] (no Bedrock / MCP / Docker tools) remains for callers that
//! only need the self-contained tools.
//!
//! # Test mapping
//!
//! The TS tool tests (`CodeInterpreterTool.test.ts`, `DockerExecutor.test.ts` and their
//! integration tests) are ported in the `docker` crate; the tools here have trait-fake tests
//! for every Bedrock tool ([`bedrock_tools::BedrockBackend`]), the MCP adapter (fake executor,
//! plus an end-to-end run against the `mcp` crate's fake stdio server), the sandbox tool
//! ([`docker_sandbox::SandboxOps`]) and the document extractors (generated PDF / DOCX files).

pub mod agent;
pub mod base;
pub mod bedrock_tools;
pub mod command;
pub mod context;
mod deps;
pub mod docker_sandbox;
pub mod documents;
pub mod error;
pub mod filesystem;
pub mod gitignore;
pub mod interpreter;
pub mod mcp_adapter;
pub mod registry;
pub mod system;
pub mod thinking;
pub mod todo;
pub mod types;
pub mod util;
pub mod web;

pub use agent::{
    InvokeAgentTool, StoppedReason, SubAgentInvokeParams, SubAgentInvokeResult, SubAgentInvoker,
    SubAgentOptions,
};
pub use base::{run_tool, Tool};
pub use bedrock_tools::{BedrockBackend, SdkBedrockBackend};
pub use context::{
    converse_settings_from_store, AgentResolver, AgentToolConfig, CallerMetadata,
    CommandPatternConfig, ToolContext, ToolSettings,
};
pub use deps::{all_tools, full_registry, ContextServices, ToolDeps};
pub use docker_sandbox::{DockerSandboxTool, SandboxOps};
pub use documents::NativeDocumentReader;
pub use error::{Error, Result, ToolError, ToolErrorType};
pub use gitignore::GitignoreLikeMatcher;
pub use interpreter::CodeInterpreterTool;
pub use mcp_adapter::{McpExecutor, McpTool};
pub use registry::ToolRegistry;
pub use system::{CameraSource, ScreenService, ScreenSource};
pub use types::{ToolCategory, ToolOutput, ToolSpec};

use std::sync::Arc;

/// `createWebTools()`.
pub fn create_web_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(web::TavilySearchTool::new()),
        Arc::new(web::FetchWebsiteTool),
    ]
}

/// `createCommandTools()`.
pub fn create_command_tools() -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(command::ExecuteCommandTool::new())]
}

/// All tools ported in this crate, in the TS `initializeToolSystem()` order
/// (filesystem, web, thinking, command, todo, agent).
pub fn builtin_tools(todo: Arc<todo::TodoService>) -> Vec<Arc<dyn Tool>> {
    let mut all = filesystem::create_filesystem_tools();
    all.extend(create_web_tools());
    all.extend(thinking::create_thinking_tools());
    all.extend(create_command_tools());
    all.extend(todo::create_todo_tools(todo));
    all.extend(agent::create_agent_tools());
    all
}

/// A registry with [`builtin_tools`] registered.
pub fn create_builtin_registry(todo: Arc<todo::TodoService>) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register_many(builtin_tools(todo));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_contents_and_spec_order() {
        let r = create_builtin_registry(Arc::new(todo::TodoService::new()));
        let names: Vec<String> = r
            .tool_specs()
            .iter()
            .map(|s| s["toolSpec"]["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "createFolder",
                "writeToFile",
                "readFiles",
                "listFiles",
                "applyDiffEdit",
                "moveFile",
                "copyFile",
                "tavilySearch",
                "fetchWebsite",
                "executeCommand",
                "think",
                "todoInit",
                "todoUpdate",
                "invokeAgent"
            ]
        );
        for s in r.tool_specs() {
            assert_eq!(s["toolSpec"]["inputSchema"]["json"]["type"], "object");
        }
        let stats = r.get_statistics();
        assert_eq!(stats.total_tools, 14);
        assert_eq!(stats.tools_by_category[&ToolCategory::Agent], 1);
        assert_eq!(stats.tools_by_category[&ToolCategory::Filesystem], 7);
        assert_eq!(stats.tools_by_category[&ToolCategory::Thinking], 3);
    }

    fn full() -> ToolRegistry {
        let settings: Arc<dyn docker::Settings> = Arc::new(|_: &str| None::<serde_json::Value>);
        let sandbox = Arc::new(docker::SandboxManager::new(
            settings.clone(),
            Arc::new(docker::NoopSink),
        ));
        let interpreter = docker::interpreter::CodeInterpreter::new(
            settings,
            Arc::new(docker::interpreter::LogFacadeLogger),
        );
        full_registry(&ToolDeps::new(
            Arc::new(todo::TodoService::new()),
            Arc::new(mcp::McpManager::new(reqwest::Client::new())),
            interpreter,
            sandbox,
        ))
    }

    #[test]
    fn full_registry_contents_and_spec_order() {
        let r = full();
        let registered: Vec<String> = r.get_all_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(
            registered,
            vec![
                "createFolder",
                "writeToFile",
                "readFiles",
                "applyDiffEdit",
                "listFiles",
                "moveFile",
                "copyFile",
                "tavilySearch",
                "fetchWebsite",
                "think",
                "executeCommand",
                "mcp",
                "generateImage",
                "generateVideo",
                "checkVideoStatus",
                "downloadVideo",
                "recognizeImage",
                "retrieve",
                "invokeBedrockAgent",
                "invokeFlow",
                "codeInterpreter",
                "dockerSandbox",
                "screenCapture",
                "cameraCapture",
                "todoInit",
                "todoUpdate",
                "invokeAgent"
            ]
        );
        let names: Vec<String> = r
            .tool_specs()
            .iter()
            .map(|s| s["toolSpec"]["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "createFolder",
                "writeToFile",
                "readFiles",
                "listFiles",
                "applyDiffEdit",
                "moveFile",
                "copyFile",
                "tavilySearch",
                "fetchWebsite",
                "generateImage",
                "generateVideo",
                "checkVideoStatus",
                "downloadVideo",
                "recognizeImage",
                "retrieve",
                "invokeBedrockAgent",
                "invokeFlow",
                "executeCommand",
                "think",
                "codeInterpreter",
                "dockerSandbox",
                "screenCapture",
                "cameraCapture",
                "todoInit",
                "todoUpdate",
                "invokeAgent"
            ]
        );
        for s in r.tool_specs() {
            assert_eq!(s["toolSpec"]["inputSchema"]["json"]["type"], "object");
            assert!(types::is_built_in_tool(
                s["toolSpec"]["name"].as_str().unwrap()
            ));
        }
        let stats = r.get_statistics();
        assert_eq!(stats.total_tools, 27);
        assert_eq!(stats.tools_by_category[&ToolCategory::System], 2);
        assert_eq!(stats.tools_by_category[&ToolCategory::Bedrock], 8);
        assert_eq!(stats.tools_by_category[&ToolCategory::Mcp], 1);
        assert_eq!(stats.tools_by_category[&ToolCategory::Interpreter], 1);
        assert_eq!(stats.tools_by_category[&ToolCategory::Docker], 1);
    }

    #[test]
    fn context_services_fill_the_context() {
        let ctx = ContextServices::default().context(
            &serde_json::json!({"projectPath": "/p", "aws": {"region": "eu-west-1"}}),
            Some("session_1".into()),
        );
        assert!(ctx.documents.is_some());
        assert!(ctx.sandbox.is_none());
        assert_eq!(ctx.session_id.as_deref(), Some("session_1"));
        assert_eq!(ctx.settings.project_path.as_deref(), Some("/p"));
        assert_eq!(ctx.settings.converse.aws.region, "eu-west-1");
    }
}
