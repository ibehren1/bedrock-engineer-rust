//! The long-lived services behind the commands, built once in `setup` (they need the
//! `AppHandle` for events and store access) and managed as [`Backend`].
//!
//! Wiring follows the library crate docs: one `ConverseService` and one `TranslateService`
//! (it owns the translation cache), one `McpManager` pool, one Docker `SandboxManager`, the full
//! tool registry, and one `AgentEngine` shared by background agents and sub-agent delegation
//! (Electron shared the `BackgroundAgentService` the same way).

use crate::settings;
use crate::state::StoreMutex;
use agents::{AgentEngine, BedrockConverseBackend, StoreReader, SubAgentRunner};
use background::{BackgroundAgents, BackgroundDeps, BackgroundStorage, PubSubManager};
use bedrock::translate::TranslateService;
use bedrock::{ConverseService, DefaultSdkConfig, SdkConfigSource};
use docker::interpreter::{CodeInterpreter, LogFacadeLogger};
use docker::SandboxManager;
use mcp::McpManager;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tools::memory::MemoryService;
use tools::todo::TodoService;
use tools::{ContextServices, ToolDeps, ToolRegistry};

/// Managed state for the Bedrock / tools / MCP / agents / background / Docker commands.
pub struct Backend {
    pub converse: Arc<ConverseService>,
    /// `converse_stream` / `converse` cancellation by request id.
    pub streams: crate::commands::bedrock::StreamRegistry,
    /// Where the non-Converse services get their `SdkConfig`.
    pub sdk: Arc<dyn SdkConfigSource>,
    pub translate: TranslateService,
    pub todo: Arc<TodoService>,
    /// `<userData>/memory.json`, shared by the memory tools and the `memory_*` commands.
    pub memory: Arc<MemoryService>,
    pub mcp: Arc<McpManager>,
    pub sandbox: Arc<SandboxManager>,
    pub registry: Arc<ToolRegistry>,
    /// Services plugged into each renderer tool call's `ToolContext`.
    pub services: ContextServices,
    pub sub_agents: Arc<SubAgentRunner>,
    pub background: Arc<BackgroundAgents>,
    /// `screen:*` handlers (shared with the screenCapture tool).
    pub screen: Arc<tools::ScreenService>,
    /// Shells on the user's machine for the chat page's Terminal tab. Deliberately not in
    /// `ToolDeps` / `ContextServices`: no tool can reach them.
    pub local_terminals: local_terminal::LocalTerminalManager,
    /// Agents' local knowledgebases (`<userData>/knowledgebases`), also served to agents as the
    /// built-in `local-knowledgebase` MCP server.
    pub knowledge: Arc<knowledge::service::KnowledgeService>,
    /// The on-device embedding model (download, status, inference).
    pub local_model: Arc<knowledge::local::LocalModel>,
}

/// `store.all()`; `{}` if the lock is poisoned.
pub fn store_snapshot(app: &AppHandle) -> Value {
    app.state::<StoreMutex>()
        .lock()
        .map(|s| s.all())
        .unwrap_or_else(|_| json!({}))
}

fn store_get(app: &AppHandle, key: &str) -> Option<Value> {
    app.state::<StoreMutex>().lock().ok()?.get(key)
}

/// The config store as the background scheduler sees it.
struct AppConfig(AppHandle);

impl background::ConfigStore for AppConfig {
    fn get(&self, key: &str) -> Option<Value> {
        store_get(&self.0, key)
    }
    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        self.0
            .state::<StoreMutex>()
            .lock()
            .map_err(|e| e.to_string())?
            .set(key, value)
            .map_err(|e| e.to_string())
    }
}

/// Main → renderer pushes as Tauri events with the Electron channel names (BRIDGE.md rule 5).
pub struct TauriEvents(pub AppHandle);

impl background::EventSink for TauriEvents {
    fn emit(&self, event: &str, payload: Value) {
        if let Err(e) = self.0.emit(event, payload) {
            tracing::warn!(event, error = %e, "Failed to emit event");
        }
    }

    fn emit_to(&self, target: &str, event: &str, payload: Value) -> Result<(), String> {
        if self.0.get_webview_window(target).is_none() {
            return Err(format!("window {target} is gone"));
        }
        self.0
            .emit_to(target, event, payload)
            .map_err(|e| e.to_string())
    }
}

/// Docker sandbox events go through pub/sub, as `pubSubManager.publish(channel, data)` did:
/// only windows subscribed to the channel receive them.
struct DockerToPubSub(Arc<PubSubManager>);

impl docker::EventSink for DockerToPubSub {
    fn publish(&self, channel: &str, event: docker::SandboxEvent) {
        match serde_json::to_value(&event) {
            Ok(data) => self.0.publish(channel, data),
            Err(e) => tracing::warn!(channel, error = %e, "Failed to serialize sandbox event"),
        }
    }
}

/// OS notifications for finished background tasks.
struct TauriNotifier(AppHandle);

impl background::Notifier for TauriNotifier {
    fn is_app_focused(&self) -> bool {
        self.0
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false)
    }

    fn show(&self, n: background::TaskOsNotification) {
        // Clicking opens the task history window (`window:openTaskHistory`); see `crate::notify`
        // for the per-platform support.
        let app = self.0.clone();
        let task_id = n.task_id.clone();
        let task_name = n.task_name.clone();
        let on_click = move || {
            tracing::info!(task_id = %task_id, task_name = %task_name, "Background agent notification clicked, opening task history");
            let result = crate::commands::window::open_task_history(&app, &task_id);
            if result["success"] != true {
                tracing::error!(task_id = %task_id, error = %result["error"], "Failed to open task history from notification click");
            }
        };
        if let Err(e) = crate::notify::show_with_click(&self.0, &n.title, &n.body, on_click) {
            tracing::warn!(task_id = %n.task_id, error = %e, "Failed to show notification");
        }
    }
}

/// Local terminal output, as the pub/sub event for its channel (`pubsub:local-terminal:<id>`),
/// sent to the main window only: no other window may read a local shell, whatever it
/// subscribes to.
struct LocalTerminalEvents(AppHandle);

impl local_terminal::EventSink for LocalTerminalEvents {
    fn publish(&self, channel: &str, message: local_terminal::TerminalMessage) {
        let event = background::pubsub_event_name(channel);
        if let Err(e) = self.0.emit_to(LOCAL_TERMINAL_WINDOW, &event, message) {
            tracing::warn!(channel, error = %e, "Failed to send local terminal event");
        }
    }
}

/// The only window the local terminal commands and events are for.
const LOCAL_TERMINAL_WINDOW: &str = "main";

/// `getMcpToolSpecs(agent.mcpServers)` for Rust-side agent runs.
struct McpSpecs(Arc<McpManager>);

#[async_trait::async_trait]
impl agents::McpToolSpecProvider for McpSpecs {
    async fn tool_specs(&self, mcp_servers: &Value) -> Result<Vec<Value>, String> {
        let servers = crate::commands::mcp::parse_servers(mcp_servers)?;
        self.0
            .get_tool_specs(Some(&servers))
            .await
            .map_err(|e| e.to_string())
    }
}

/// Store, proxy and events for the knowledgebase service.
struct KnowledgeHost(AppHandle);

impl knowledge::service::KnowledgeHost for KnowledgeHost {
    fn settings(&self) -> knowledge::types::KnowledgeSettings {
        knowledge::types::KnowledgeSettings::from_store(&store_snapshot(&self.0))
    }
    fn http_client(&self) -> reqwest::Client {
        settings::http_client(&store_snapshot(&self.0))
    }
    fn emit(&self, event: &str, payload: Value) {
        if let Err(e) = self.0.emit(event, payload) {
            tracing::warn!(event, error = %e, "Failed to emit event");
        }
    }
}

/// PDF / DOCX text for attachment context (the `pdf-*` / `docx-*` extractors in `tools`).
pub struct DocumentExtractor;

impl attachments::TextExtractor for DocumentExtractor {
    fn extract_pdf_text(&self, path: &Path) -> Result<String, String> {
        tools::documents::extract_pdf_text(&path.to_string_lossy(), None)
    }
    fn extract_docx_text(&self, path: &Path) -> Result<String, String> {
        tools::documents::extract_docx_text(&path.to_string_lossy(), None)
    }
}

impl Backend {
    /// Build every service. `user_data` is Electron's `userData` (also the config.json dir).
    pub fn build(app: &AppHandle, user_data: &Path) -> anyhow::Result<Backend> {
        let snapshot = {
            let app = app.clone();
            move || store_snapshot(&app)
        };
        let store_reader: Arc<dyn StoreReader> = Arc::new(snapshot.clone());

        let events: Arc<dyn background::EventSink> = Arc::new(TauriEvents(app.clone()));
        let storage = BackgroundStorage::open(user_data, user_data, events.clone())?;

        let docker_settings: Arc<dyn docker::Settings> = Arc::new({
            let app = app.clone();
            move |key: &str| store_get(&app, key)
        });
        let sandbox = Arc::new(SandboxManager::new(
            docker_settings.clone(),
            Arc::new(DockerToPubSub(storage.pubsub.clone())),
        ));
        let interpreter = CodeInterpreter::new(docker_settings, Arc::new(LogFacadeLogger));

        let todo = Arc::new(TodoService::new());
        let memory = Arc::new(MemoryService::open(user_data));
        memory.set_listener({
            let app = app.clone();
            move |items| {
                let event = crate::commands::memory::MEMORY_CHANGED_EVENT;
                if let Err(e) = app.emit(event, items) {
                    tracing::warn!(event, error = %e, "Failed to emit event");
                }
            }
        });
        // URL servers use the proxy configured at startup.
        let mcp = Arc::new(McpManager::new(settings::http_client(&snapshot())));

        let runtime = tauri::async_runtime::handle().inner().clone();
        let sdk: Arc<dyn SdkConfigSource> = Arc::new(DefaultSdkConfig);
        let kb_root = user_data.join("knowledgebases");
        let local_model = Arc::new(knowledge::local::LocalModel::new(
            kb_root.join(knowledge::service::MODELS_DIR),
        ));
        let knowledge = knowledge::service::KnowledgeService::new(
            kb_root,
            Arc::new(KnowledgeHost(app.clone())),
            Arc::new(knowledge::embed::DefaultEmbedders {
                sdk: sdk.clone(),
                aws: Arc::new({
                    let snapshot = snapshot.clone();
                    move || settings::aws_settings(&snapshot())
                }),
                local: local_model.clone(),
            }),
            runtime.clone(),
        );
        mcp.set_builtin(Arc::new(knowledge::mcp_server::KnowledgeMcp {
            svc: knowledge.clone(),
            runtime: runtime.clone(),
        }));
        let mut tool_deps = ToolDeps::new(
            todo.clone(),
            memory.clone(),
            mcp.clone(),
            interpreter,
            sandbox.clone(),
        );
        // cameraCapture grabs frames in the main webview.
        tool_deps.camera = Arc::new(crate::commands::camera::WebviewCamera::new(
            app.clone(),
            app.state::<crate::commands::camera::CameraState>()
                .frames
                .clone(),
        ));
        let screen = tool_deps.screen.clone();
        let registry = Arc::new(tools::full_registry(&tool_deps));

        let base_services = ContextServices {
            sandbox: Some(sandbox.clone()),
            ..Default::default()
        };

        let converse = Arc::new(ConverseService::default());
        let converse_backend = Arc::new(BedrockConverseBackend::new(
            converse.clone(),
            Arc::new({
                let snapshot = snapshot.clone();
                move || settings::converse_settings(&snapshot())
            }),
        ));
        let engine_services = base_services.clone();
        let engine = Arc::new(
            AgentEngine::new(converse_backend, registry.clone(), store_reader.clone())
                .with_sessions(storage.sessions.clone())
                .with_listener(storage.listener.clone())
                .with_mcp_specs(Arc::new(McpSpecs(mcp.clone())))
                .with_tool_context(Arc::new(move |snap: &Value| {
                    engine_services.context(snap, None)
                })),
        );
        let sub_agents = agents::wire_sub_agents(&engine, store_reader);

        let services = ContextServices {
            agents: Some(engine.catalog().clone()),
            sub_agents: Some(sub_agents.clone()),
            ..base_services
        };

        let background = Arc::new(BackgroundAgents::new(BackgroundDeps {
            engine: engine.clone(),
            storage,
            config: Arc::new(AppConfig(app.clone())),
            events,
            notifier: Arc::new(TauriNotifier(app.clone())),
            runtime,
        }));

        Ok(Backend {
            converse,
            streams: Default::default(),
            sdk,
            translate: TranslateService::new(),
            todo,
            memory,
            mcp,
            sandbox,
            registry,
            services,
            sub_agents,
            background,
            screen,
            local_terminals: local_terminal::LocalTerminalManager::new(Arc::new(
                LocalTerminalEvents(app.clone()),
            )),
            knowledge,
            local_model,
        })
    }

    /// App quit (Electron `before-quit`): end the local shells, close sandbox terminals before
    /// stopping their containers, stop sandboxes (not removed), drop MCP clients, stop the
    /// scheduler.
    pub fn shutdown(&self) {
        // Local shells: SIGHUP, then SIGKILL whatever is left after a short grace.
        self.local_terminals.close_all();
        self.knowledge.cancel_all();
        self.sandbox.terminals().close_all_terminals();
        let sandbox = self.sandbox.clone();
        let mcp = self.mcp.clone();
        tauri::async_runtime::block_on(async move {
            let stop = async {
                sandbox.stop_all_sandboxes().await;
                mcp.cleanup().await;
            };
            if tokio::time::timeout(std::time::Duration::from_secs(10), stop)
                .await
                .is_err()
            {
                tracing::warn!("Shutdown cleanup timed out");
            }
        });
        self.background.shutdown_scheduler();
        tracing::info!("Background services shut down");
    }
}
