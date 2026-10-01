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
    pub mcp: Arc<McpManager>,
    pub sandbox: Arc<SandboxManager>,
    pub registry: Arc<ToolRegistry>,
    /// Services plugged into each renderer tool call's `ToolContext`.
    pub services: ContextServices,
    pub sub_agents: Arc<SubAgentRunner>,
    pub background: Arc<BackgroundAgents>,
    /// `screen:*` handlers (shared with the screenCapture tool).
    pub screen: Arc<tools::ScreenService>,
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
        // URL servers use the proxy configured at startup.
        let mcp = Arc::new(McpManager::new(settings::http_client(&snapshot())));
        let mut tool_deps = ToolDeps::new(todo.clone(), mcp.clone(), interpreter, sandbox.clone());
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
            runtime: tauri::async_runtime::handle().inner().clone(),
        }));

        Ok(Backend {
            converse,
            streams: Default::default(),
            sdk: Arc::new(DefaultSdkConfig),
            translate: TranslateService::new(),
            todo,
            mcp,
            sandbox,
            registry,
            services,
            sub_agents,
            background,
            screen,
        })
    }

    /// App quit (Electron `before-quit`): close terminals before stopping their containers,
    /// stop sandboxes (not removed), drop MCP clients, stop the scheduler.
    pub fn shutdown(&self) {
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
