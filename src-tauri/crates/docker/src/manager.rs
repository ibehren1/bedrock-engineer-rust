//! Per-chat sandbox lifecycle. Port of `src/main/api/docker/sandboxManager.ts`.
//!
//! The TS read `projectPath`, `userDataPath` and `dockerSandboxTool` from the electron store
//! and published through `pubSubManager`; here both are injected ([`Settings`],
//! [`EventSink`]) so the app crate wires them to the `store` crate and Tauri events.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::activity::ActivityLog;
use crate::availability::{assert_sandbox_usable, check_docker_availability};
use crate::compose_writer::{
    assert_ports_available, build_compose, service_environment, write_sandbox_files,
    DEFAULT_SERVICE_ENV,
};
use crate::engine::{cpu_percent_between, rates_between, ContainerSample, DockerEngine};
use crate::events::{EventSink, SandboxEvent, StateEvent};
use crate::exec::{read_sandbox_logs, ExecRegistry, ExecTarget, LogsResult};
use crate::naming::{container_name_for, is_default_chat_title, to_folder_name, to_project_name};
use crate::runner::{
    compose, list_project_containers, CommandRunner, ComposeContext, ComposeKind, RunOptions,
    TokioRunner,
};
use crate::terminal::{EngineTransport, TerminalManager, TerminalTarget};
use crate::types::{
    ContainerRow, CreateSandboxOptions, DockerAvailability, DockerSandboxConfig,
    SandboxExecOptions, SandboxExecResult, SandboxMetadata, SandboxRemoveOptions, SandboxRunState,
    SandboxServiceSummary, SandboxStatus, DEFAULT_SANDBOX_IMAGE, SANDBOX_ROOT_DIRNAME,
    WORKSPACE_MOUNT,
};
use crate::util::{now_iso, resolve_one};
use crate::{Error, Result};

const METADATA_FILENAME: &str = "sandbox.json";
/// Docker state changes rarely; re-probe at most this often.
const AVAILABILITY_TTL: Duration = Duration::from_millis(15_000);

/// Read access to the app settings store (`store.get(key)`).
pub trait Settings: Send + Sync {
    fn get(&self, key: &str) -> Option<Value>;
}

impl<F> Settings for F
where
    F: Fn(&str) -> Option<Value> + Send + Sync,
{
    fn get(&self, key: &str) -> Option<Value> {
        self(key)
    }
}

/// Channel the chat page listens on so state changes do not wait for the next poll.
pub fn sandbox_state_channel(session_id: &str) -> String {
    format!("docker-sandbox:state:{session_id}")
}

fn project_path(settings: &dyn Settings) -> Result<String> {
    settings
        .get("projectPath")
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            Error::msg(
                "No project directory is configured. Set one in Settings before using the Docker sandbox.",
            )
        })
}

/// `readChatTitle` (`src/main/lib/chatSessionTitle.ts`): the chat's title straight from
/// the session file chat history writes.
pub fn read_chat_title(settings: &dyn Settings, session_id: &str) -> Option<String> {
    let user_data = settings.get("userDataPath")?.as_str()?.to_string();
    let file = Path::new(&user_data)
        .join("chat-sessions")
        .join(format!("{session_id}.json"));
    let text = std::fs::read_to_string(file).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    parsed.get("title")?.as_str().map(str::to_string)
}

fn sandbox_root(project_path: &str) -> PathBuf {
    Path::new(project_path).join(SANDBOX_ROOT_DIRNAME)
}

/// Locate an existing sandbox folder by scanning for the metadata file that claims this
/// session. Folders follow the chat title, so the name cannot be derived from the id.
fn find_sandbox_dir(settings: &dyn Settings, session_id: &str) -> Option<PathBuf> {
    let root = sandbox_root(&project_path(settings).ok()?);
    let entries = std::fs::read_dir(&root).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let candidate = entry.path().join(METADATA_FILENAME);
        let Ok(text) = std::fs::read_to_string(&candidate) else {
            continue;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(parsed) => {
                if parsed.get("sessionId").and_then(Value::as_str) == Some(session_id) {
                    return Some(entry.path());
                }
            }
            Err(error) => {
                // The TS aborted the scan on a parse error; keep that.
                log::warn!(
                    target: "docker:sandbox",
                    "Failed to locate sandbox directory session={session_id} error={error}"
                );
                return None;
            }
        }
    }
    None
}

fn derive_state(
    metadata: Option<&SandboxMetadata>,
    containers: &[ContainerRow],
) -> SandboxRunState {
    if metadata.is_none() {
        return SandboxRunState::Missing;
    }
    if containers.is_empty() {
        return SandboxRunState::Stopped;
    }
    let running = containers.iter().filter(|c| c.state == "running").count();
    if running == 0 {
        SandboxRunState::Stopped
    } else if running == containers.len() {
        SandboxRunState::Running
    } else {
        SandboxRunState::Partial
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSandboxResult {
    pub metadata: SandboxMetadata,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameResult {
    pub renamed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    pub removed: bool,
    pub data_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxComposeFile {
    /// True when the stack is driven by `docker run` because compose is unavailable.
    pub composeless: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxInsights {
    pub container_name: String,
    pub service: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// Percent of one core, in `docker stats` units: 200% means two cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_used: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_rx: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_tx: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_rx_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_tx_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_write: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_read_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_write_per_second: Option<f64>,
    /// Present when the figures could not be read, for display in the panel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct Inner {
    settings: Arc<dyn Settings>,
    runner: Arc<dyn CommandRunner>,
    engine: Arc<DockerEngine>,
    activity: Arc<ActivityLog>,
    execs: Arc<ExecRegistry>,
    terminals: TerminalManager,
    sink: Arc<dyn EventSink>,
    availability: Mutex<Option<(DockerAvailability, Instant)>>,
    /// Previous CPU sample per container: a percentage needs two readings.
    stats_samples: Mutex<HashMap<String, ContainerSample>>,
}

/// Owns every per-chat sandbox. Cheap to clone; hold one in Tauri managed state.
#[derive(Clone)]
pub struct SandboxManager {
    inner: Arc<Inner>,
}

impl SandboxManager {
    /// Production wiring: the real `docker` CLI and Engine API.
    pub fn new(settings: Arc<dyn Settings>, sink: Arc<dyn EventSink>) -> Self {
        let runner: Arc<dyn CommandRunner> = Arc::new(TokioRunner);
        let engine = Arc::new(DockerEngine::new(runner.clone()));
        Self::with_parts(settings, sink, runner, engine)
    }

    /// Wiring with an injected runner and engine (tests, alternative transports).
    pub fn with_parts(
        settings: Arc<dyn Settings>,
        sink: Arc<dyn EventSink>,
        runner: Arc<dyn CommandRunner>,
        engine: Arc<DockerEngine>,
    ) -> Self {
        let activity = Arc::new(ActivityLog::new(sink.clone()));
        // The activity log lives in the sandbox folder, which moves when a chat is renamed.
        let resolver_settings = settings.clone();
        activity.set_directory_resolver(Arc::new(move |session_id| {
            find_sandbox_dir(resolver_settings.as_ref(), session_id)
        }));
        let execs = Arc::new(ExecRegistry::new(activity.clone()));
        let terminals = TerminalManager::new(
            Arc::new(EngineTransport::new(engine.clone())),
            activity.clone(),
            sink.clone(),
        );
        Self {
            inner: Arc::new(Inner {
                settings,
                runner,
                engine,
                activity,
                execs,
                terminals,
                sink,
                availability: Mutex::new(None),
                stats_samples: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn activity(&self) -> &Arc<ActivityLog> {
        &self.inner.activity
    }

    pub fn terminals(&self) -> &TerminalManager {
        &self.inner.terminals
    }

    pub fn engine(&self) -> &Arc<DockerEngine> {
        &self.inner.engine
    }

    pub fn runner(&self) -> &Arc<dyn CommandRunner> {
        &self.inner.runner
    }

    fn settings(&self) -> &dyn Settings {
        self.inner.settings.as_ref()
    }

    fn publish_status(&self, session_id: &str, status: SandboxStatus) {
        self.inner.sink.publish(
            &sandbox_state_channel(session_id),
            SandboxEvent::State(StateEvent::State { status }),
        );
    }

    pub async fn get_availability(&self, force: bool) -> DockerAvailability {
        if !force {
            if let Some((value, at)) = self.inner.availability.lock().unwrap().as_ref() {
                if at.elapsed() < AVAILABILITY_TTL {
                    return value.clone();
                }
            }
        }
        let value = check_docker_availability(self.inner.runner.as_ref()).await;
        *self.inner.availability.lock().unwrap() = Some((value.clone(), Instant::now()));
        value
    }

    /// `<projectPath>/docker-sandboxes`.
    pub fn get_sandbox_root(&self, project_path_override: Option<&str>) -> Result<PathBuf> {
        match project_path_override {
            Some(path) => Ok(sandbox_root(path)),
            None => Ok(sandbox_root(&project_path(self.settings())?)),
        }
    }

    /// The session's existing sandbox folder, or the readable name its title implies.
    pub fn get_sandbox_dir(&self, session_id: &str) -> Result<PathBuf> {
        let project = project_path(self.settings())?;
        Ok(self.sandbox_dir_in(session_id, &project))
    }

    fn sandbox_dir_in(&self, session_id: &str, project: &str) -> PathBuf {
        if let Some(existing) = find_sandbox_dir(self.settings(), session_id) {
            return existing;
        }
        let title = read_chat_title(self.settings(), session_id);
        sandbox_root(project).join(to_folder_name(session_id, title.as_deref()))
    }

    fn config(&self) -> DockerSandboxConfig {
        let mut merged =
            serde_json::to_value(DockerSandboxConfig::default()).expect("config serializes");
        if let (Some(Value::Object(stored)), Value::Object(base)) =
            (self.settings().get("dockerSandboxTool"), &mut merged)
        {
            for (key, value) in stored {
                base.insert(key, value);
            }
        }
        serde_json::from_value(merged).unwrap_or_default()
    }

    /// Keep generated sandboxes out of the user's repository.
    fn ensure_sandbox_root(project: &str) -> Result<PathBuf> {
        let root = sandbox_root(project);
        std::fs::create_dir_all(&root)?;
        let gitignore = root.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(
                &gitignore,
                "# Chat-scoped Docker sandboxes generated by Bedrock Engineer.\n*\n",
            )?;
        }
        Ok(root)
    }

    fn read_metadata(&self, session_id: &str) -> Option<SandboxMetadata> {
        let dir = find_sandbox_dir(self.settings(), session_id)?;
        let text = std::fs::read_to_string(dir.join(METADATA_FILENAME)).ok()?;
        match serde_json::from_str::<SandboxMetadata>(&text) {
            Ok(metadata) => {
                // The folder may have been renamed since it was written; trust its location.
                let compose_name = Path::new(&metadata.compose_file)
                    .file_name()
                    .map(|n| n.to_owned())
                    .unwrap_or_else(|| "docker-compose.yml".into());
                Some(SandboxMetadata {
                    directory: dir.to_string_lossy().into_owned(),
                    compose_file: dir.join(compose_name).to_string_lossy().into_owned(),
                    ..metadata
                })
            }
            Err(error) => {
                log::warn!(
                    target: "docker:sandbox",
                    "Failed to read sandbox metadata session={session_id} error={error}"
                );
                None
            }
        }
    }

    fn write_metadata(metadata: &SandboxMetadata) -> Result<()> {
        std::fs::write(
            Path::new(&metadata.directory).join(METADATA_FILENAME),
            serde_json::to_string_pretty(metadata)?,
        )?;
        Ok(())
    }

    fn to_compose_context(
        metadata: &SandboxMetadata,
        availability: &DockerAvailability,
    ) -> Option<ComposeContext> {
        if metadata.composeless {
            return None;
        }
        let kind = ComposeKind::from_flavor(availability.compose)?;
        Some(ComposeContext {
            kind,
            compose_file: metadata.compose_file.clone(),
            env_file: Path::new(&metadata.directory)
                .join(".env")
                .to_string_lossy()
                .into_owned(),
            cwd: metadata.directory.clone(),
        })
    }

    async fn to_exec_target(
        &self,
        metadata: &SandboxMetadata,
        service: Option<&str>,
    ) -> Result<ExecTarget> {
        let availability = self.get_availability(false).await;
        let resolved = service
            .map(str::to_string)
            .or_else(|| metadata.services.first().map(|s| s.name.clone()))
            .ok_or_else(|| {
                Error::msg(format!(
                    "Sandbox for session {} has no services.",
                    metadata.session_id
                ))
            })?;
        if !metadata.services.iter().any(|s| s.name == resolved) {
            let names: Vec<&str> = metadata.services.iter().map(|s| s.name.as_str()).collect();
            return Err(Error::msg(format!(
                "Service \"{resolved}\" is not part of this sandbox. Available services: {}.",
                names.join(", ")
            )));
        }
        Ok(ExecTarget {
            session_id: metadata.session_id.clone(),
            compose: Self::to_compose_context(metadata, &availability),
            container_name: Some(container_name_for(&metadata.project_name, &resolved)),
            service: resolved,
        })
    }

    /// One `docker run -d` per service, only when no compose implementation exists.
    async fn run_composeless(
        &self,
        metadata: &SandboxMetadata,
        config: &DockerSandboxConfig,
    ) -> Result<()> {
        if metadata.services.len() > 1 {
            return Err(Error::msg(
                "This sandbox defines multiple services, which requires Docker Compose. Install the compose plugin (`docker compose`) and recreate the sandbox, or define a single service.",
            ));
        }
        let service = metadata.services.first().ok_or_else(|| {
            Error::msg(format!(
                "Sandbox for session {} has no services.",
                metadata.session_id
            ))
        })?;
        let name = container_name_for(&metadata.project_name, &service.name);
        let runner = self.inner.runner.as_ref();

        // The service's `environment` (agent `env` included) lives in the compose file,
        // which is what compose would pass to the container.
        let environment = match std::fs::read_to_string(&metadata.compose_file) {
            Ok(text) => service_environment(&text, &service.name)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                log::warn!(
                    target: "docker:sandbox",
                    "Compose file missing, starting without service environment path={}",
                    metadata.compose_file
                );
                Vec::new()
            }
            Err(error) => {
                return Err(Error::msg(format!(
                    "Could not read the sandbox compose file {}: {error}",
                    metadata.compose_file
                )))
            }
        };

        // Remove a stale container with the same name so re-creation is idempotent.
        runner
            .run(
                "docker",
                &["rm".into(), "-f".into(), name.clone()],
                RunOptions::default(),
            )
            .await;

        let data_dir = Path::new(&metadata.directory)
            .join("data")
            .join(&service.name);
        let mut args: Vec<String> = vec![
            "run".into(),
            "-d".into(),
            "--name".into(),
            name,
            "--label".into(),
            format!("com.docker.compose.project={}", metadata.project_name),
            "--label".into(),
            format!("com.docker.compose.service={}", service.name),
            "--workdir".into(),
            WORKSPACE_MOUNT.into(),
            "-v".into(),
            format!("{}:{WORKSPACE_MOUNT}", metadata.project_path),
            "-v".into(),
            format!("{}:/data", data_dir.to_string_lossy()),
            "--memory".into(),
            config.memory_limit.clone(),
            "--cpus".into(),
            config.cpu_limit.to_string(),
        ];
        for (env_name, value) in &environment {
            args.push("-e".into());
            args.push(format!("{env_name}={value}"));
        }
        for (env_name, value) in DEFAULT_SERVICE_ENV {
            if !environment.iter().any(|(n, _)| n == env_name) {
                args.push("-e".into());
                args.push(format!("{env_name}={value}"));
            }
        }
        for port in &service.ports {
            args.push("-p".into());
            args.push(format!("{}:{}", port.host, port.container));
        }
        args.extend([service.image.clone(), "sleep".into(), "infinity".into()]);

        let result = runner
            .run("docker", &args, RunOptions::timeout_ms(180_000))
            .await;
        if result.exit_code != 0 {
            let detail = if result.stderr.trim().is_empty() {
                result.stdout.trim()
            } else {
                result.stderr.trim()
            };
            return Err(Error::msg(format!(
                "Failed to start sandbox container: {detail}"
            )));
        }
        Ok(())
    }

    async fn up_sandbox(&self, metadata: &SandboxMetadata) -> Result<()> {
        let availability = self.get_availability(false).await;
        assert_sandbox_usable(&availability)?;

        let Some(ctx) = Self::to_compose_context(metadata, &availability) else {
            return self.run_composeless(metadata, &self.config()).await;
        };

        // Pulling can be slow on a cold cache, so this gets a longer budget.
        let result = compose(
            self.inner.runner.as_ref(),
            &ctx,
            &["up", "-d", "--remove-orphans"],
            RunOptions::timeout_ms(600_000),
        )
        .await;
        if result.exit_code != 0 {
            let detail = [result.stderr.trim(), result.stdout.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("unknown error");
            return Err(Error::msg(format!("docker compose up failed: {detail}")));
        }
        Ok(())
    }

    /// Create (or recreate) the sandbox for a chat session and bring it up.
    pub async fn create_sandbox(
        &self,
        session_id: &str,
        options: CreateSandboxOptions,
    ) -> Result<CreateSandboxResult> {
        let availability = self.get_availability(true).await;
        assert_sandbox_usable(&availability)?;

        let project = project_path(self.settings())?;
        Self::ensure_sandbox_root(&project)?;

        let sandbox_dir = self.sandbox_dir_in(session_id, &project);
        let project_name = to_project_name(session_id);
        let config = self.config();

        let existing = self.read_metadata(session_id);
        if let Some(existing) = &existing {
            if options.recreate != Some(true)
                && options.compose_yaml.as_deref().is_none_or(str::is_empty)
                && options.services.is_none()
            {
                self.up_sandbox(existing).await?;
                return Ok(CreateSandboxResult {
                    metadata: existing.clone(),
                    warnings: Vec::new(),
                });
            }
            if options.recreate == Some(true) {
                self.remove_sandbox(
                    session_id,
                    SandboxRemoveOptions {
                        delete_data: Some(false),
                    },
                )
                .await?;
            }
        }

        let built = build_compose(&options, &sandbox_dir, Path::new(&project), &config)?;
        assert_ports_available(&built.services)?;

        let written = write_sandbox_files(&sandbox_dir, &project_name, &project, &built)?;

        let now = now_iso();
        let metadata = SandboxMetadata {
            session_id: session_id.to_string(),
            project_name: project_name.clone(),
            directory: sandbox_dir.to_string_lossy().into_owned(),
            compose_file: written.compose_file.to_string_lossy().into_owned(),
            project_path: project.clone(),
            services: built
                .services
                .iter()
                .map(|s| SandboxServiceSummary {
                    name: s.name.clone(),
                    image: if s.image.is_empty() {
                        DEFAULT_SANDBOX_IMAGE.to_string()
                    } else {
                        s.image.clone()
                    },
                    ports: s.ports.clone(),
                })
                .collect(),
            composeless: availability.compose == crate::types::ComposeFlavor::None,
            created_at: existing
                .as_ref()
                .map(|e| e.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        };

        Self::write_metadata(&metadata)?;
        self.up_sandbox(&metadata).await?;

        log::info!(
            target: "docker:sandbox",
            "Sandbox created session={session_id} project={project_name} composeless={}",
            metadata.composeless
        );

        // Usually created by the agent's first command: tell the chat page now.
        self.publish_state(session_id).await;

        Ok(CreateSandboxResult {
            metadata,
            warnings: built.warnings,
        })
    }

    /// Move a sandbox's folder to match the chat's current title. Only the folder moves;
    /// the compose project name stays keyed on the session id, so nothing is recreated.
    pub async fn rename_sandbox(&self, session_id: &str) -> RenameResult {
        let Some(metadata) = self.read_metadata(session_id) else {
            return RenameResult {
                renamed: false,
                directory: None,
            };
        };

        // The chat session file is the single source of truth for the title.
        let title = read_chat_title(self.settings(), session_id);
        let desired =
            sandbox_root(&metadata.project_path).join(to_folder_name(session_id, title.as_deref()));

        let unchanged = RenameResult {
            renamed: false,
            directory: Some(metadata.directory.clone()),
        };
        if resolve_one(&desired) == resolve_one(Path::new(&metadata.directory)) {
            return unchanged;
        }

        // A collision means a different chat already owns that folder; leave ours be.
        if desired.exists() {
            log::warn!(
                target: "docker:sandbox",
                "Skipping sandbox rename because the target folder already exists session={session_id} desired={}",
                desired.display()
            );
            return unchanged;
        }

        if let Err(error) = std::fs::rename(&metadata.directory, &desired) {
            log::warn!(
                target: "docker:sandbox",
                "Failed to rename sandbox folder session={session_id} from={} to={} error={error}",
                metadata.directory,
                desired.display()
            );
            return unchanged;
        }

        let compose_name = Path::new(&metadata.compose_file)
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_else(|| "docker-compose.yml".into());
        let updated = SandboxMetadata {
            directory: desired.to_string_lossy().into_owned(),
            compose_file: desired.join(compose_name).to_string_lossy().into_owned(),
            updated_at: now_iso(),
            ..metadata
        };
        if let Err(error) = Self::write_metadata(&updated) {
            log::warn!(target: "docker:sandbox", "Failed to rewrite sandbox metadata: {error}");
        }

        RenameResult {
            renamed: true,
            directory: Some(updated.directory),
        }
    }

    /// The sandbox for a session, creating a default one on first use.
    pub async fn ensure_sandbox(&self, session_id: &str) -> Result<SandboxMetadata> {
        let Some(mut existing) = self.read_metadata(session_id) else {
            return Ok(self
                .create_sandbox(session_id, CreateSandboxOptions::default())
                .await?
                .metadata);
        };

        // Sandboxes are usually created before the chat has a real title; catch up here.
        let title = read_chat_title(self.settings(), session_id);
        if !is_default_chat_title(title.as_deref()) && self.rename_sandbox(session_id).await.renamed
        {
            existing = self.read_metadata(session_id).unwrap_or(existing);
        }

        let status = self.get_status(session_id).await;
        if status.state != SandboxRunState::Running {
            self.up_sandbox(&existing).await?;
            self.publish_state(session_id).await;
        }
        Ok(existing)
    }

    pub async fn get_status(&self, session_id: &str) -> SandboxStatus {
        let Some(metadata) = self.read_metadata(session_id) else {
            return SandboxStatus::missing();
        };

        let availability = self.get_availability(false).await;
        if !availability.docker_installed || !availability.daemon_running {
            // The sandbox's files exist even when Docker can't answer for its containers.
            return SandboxStatus {
                exists: true,
                state: SandboxRunState::Stopped,
                metadata: Some(metadata),
                containers: Vec::new(),
            };
        }

        // The Engine API answers in about a millisecond; `docker ps` costs a spawn.
        let containers = match self
            .inner
            .engine
            .list_project_containers(&metadata.project_name)
            .await
        {
            Ok(rows) => rows,
            Err(_) => {
                list_project_containers(self.inner.runner.as_ref(), &metadata.project_name).await
            }
        };

        SandboxStatus {
            exists: true,
            state: derive_state(Some(&metadata), &containers),
            metadata: Some(metadata),
            containers,
        }
    }

    /// Push the current status to the chat page.
    pub async fn publish_state(&self, session_id: &str) {
        let status = self.get_status(session_id).await;
        self.publish_status(session_id, status);
    }

    pub async fn start_sandbox(&self, session_id: &str) -> Result<SandboxStatus> {
        let metadata = self
            .read_metadata(session_id)
            .ok_or_else(|| Error::msg(format!("No sandbox exists for session {session_id}.")))?;
        self.up_sandbox(&metadata).await?;
        let status = self.get_status(session_id).await;
        self.publish_status(session_id, status.clone());
        Ok(status)
    }

    pub async fn stop_sandbox(&self, session_id: &str) -> Result<SandboxStatus> {
        let Some(metadata) = self.read_metadata(session_id) else {
            return Ok(SandboxStatus::missing());
        };

        self.inner.execs.invalidate_session_execs(session_id);
        // The shell dies with its container; close it so the panel reports an exit.
        self.inner.terminals.close_session_terminals(session_id);

        let availability = self.get_availability(false).await;
        match Self::to_compose_context(&metadata, &availability) {
            Some(ctx) => {
                let result = compose(
                    self.inner.runner.as_ref(),
                    &ctx,
                    &["stop"],
                    RunOptions::timeout_ms(120_000),
                )
                .await;
                if result.exit_code != 0 {
                    log::warn!(
                        target: "docker:sandbox",
                        "compose stop reported a failure session={session_id} stderr={}",
                        result.stderr.trim()
                    );
                }
            }
            None => {
                for service in &metadata.services {
                    let name = container_name_for(&metadata.project_name, &service.name);
                    self.inner
                        .runner
                        .run("docker", &["stop".into(), name], RunOptions::default())
                        .await;
                }
            }
        }

        log::info!(target: "docker:sandbox", "Sandbox stopped session={session_id}");
        let status = self.get_status(session_id).await;
        self.publish_status(session_id, status.clone());
        Ok(status)
    }

    /// Tear the sandbox down. Containers and networks always go; `delete_data` also removes
    /// the sandbox folder and its mapped data volumes.
    pub async fn remove_sandbox(
        &self,
        session_id: &str,
        options: SandboxRemoveOptions,
    ) -> Result<RemoveResult> {
        let Some(metadata) = self.read_metadata(session_id) else {
            return Ok(RemoveResult {
                removed: false,
                data_deleted: false,
            });
        };
        let delete_data = options.delete_data == Some(true);

        self.inner.execs.invalidate_session_execs(session_id);
        self.inner.terminals.close_session_terminals(session_id);

        let availability = self.get_availability(false).await;
        if availability.docker_installed && availability.daemon_running {
            match Self::to_compose_context(&metadata, &availability) {
                Some(ctx) => {
                    let args: &[&str] = if delete_data {
                        &["down", "-v", "--remove-orphans"]
                    } else {
                        &["down", "--remove-orphans"]
                    };
                    let result = compose(
                        self.inner.runner.as_ref(),
                        &ctx,
                        args,
                        RunOptions::timeout_ms(180_000),
                    )
                    .await;
                    if result.exit_code != 0 {
                        log::warn!(
                            target: "docker:sandbox",
                            "compose down reported a failure session={session_id} stderr={}",
                            result.stderr.trim()
                        );
                    }
                }
                None => {
                    for service in &metadata.services {
                        let name = container_name_for(&metadata.project_name, &service.name);
                        self.inner
                            .runner
                            .run(
                                "docker",
                                &["rm".into(), "-f".into(), name],
                                RunOptions::default(),
                            )
                            .await;
                    }
                }
            }
        } else {
            log::warn!(
                target: "docker:sandbox",
                "Removing sandbox files without Docker; containers may remain session={session_id}"
            );
        }

        // Before the folder goes: the log lives inside it, and clearing resolves it.
        self.inner.activity.clear_activity(session_id).await;

        if delete_data {
            let _ = std::fs::remove_dir_all(&metadata.directory);
        } else {
            // Keep the folder (compose file and data/) but drop the metadata, so the sandbox
            // no longer counts as existing.
            let _ = std::fs::remove_file(Path::new(&metadata.directory).join(METADATA_FILENAME));
        }

        log::info!(
            target: "docker:sandbox",
            "Sandbox removed session={session_id} dataDeleted={delete_data}"
        );
        self.publish_status(session_id, SandboxStatus::missing());
        Ok(RemoveResult {
            removed: true,
            data_deleted: delete_data,
        })
    }

    /// Resolve the container a terminal should attach to. The renderer names at most a
    /// service, never a container — otherwise an XSS would become a root shell anywhere.
    pub async fn resolve_terminal_target(
        &self,
        session_id: &str,
        service: Option<&str>,
    ) -> Result<TerminalTarget> {
        let metadata = self
            .read_metadata(session_id)
            .ok_or_else(|| Error::msg(format!("No sandbox exists for session {session_id}.")))?;
        let target = self.to_exec_target(&metadata, service).await?;
        let container_name = target.container_name.ok_or_else(|| {
            Error::msg(format!(
                "Could not resolve a container for session {session_id}."
            ))
        })?;
        Ok(TerminalTarget {
            session_id: session_id.to_string(),
            service: target.service,
            container_name,
        })
    }

    /// The generated compose file, read fresh from disk.
    pub async fn get_compose_file(&self, session_id: &str) -> Result<SandboxComposeFile> {
        let metadata = self
            .read_metadata(session_id)
            .ok_or_else(|| Error::msg(format!("No sandbox exists for session {session_id}.")))?;
        if metadata.composeless {
            return Ok(SandboxComposeFile {
                composeless: true,
                path: None,
                contents: None,
                error: None,
            });
        }
        Ok(match std::fs::read_to_string(&metadata.compose_file) {
            Ok(contents) => SandboxComposeFile {
                composeless: false,
                path: Some(metadata.compose_file),
                contents: Some(contents),
                error: None,
            },
            Err(error) => SandboxComposeFile {
                composeless: false,
                path: Some(metadata.compose_file),
                contents: None,
                error: Some(error.to_string()),
            },
        })
    }

    /// True when `port` is published by this sandbox. Guards the open-in-browser action.
    pub fn is_published_port(&self, session_id: &str, port: u16) -> bool {
        self.read_metadata(session_id)
            .map(|m| {
                m.services
                    .iter()
                    .any(|s| s.ports.iter().any(|p| p.host == port))
            })
            .unwrap_or(false)
    }

    /// Identity and resource use for a sandbox's container, for the panel's Overview tab.
    pub async fn get_insights(
        &self,
        session_id: &str,
        service: Option<&str>,
    ) -> Result<SandboxInsights> {
        let target = self.resolve_terminal_target(session_id, service).await?;
        let mut insights = SandboxInsights {
            container_name: target.container_name.clone(),
            service: target.service.clone(),
            ..Default::default()
        };

        let engine = &self.inner.engine;
        let outcome: Result<()> = async {
            let inspected = engine.inspect_container(&target.container_name).await?;
            insights.image = Some(inspected.config.image);
            insights.image_id = Some(inspected.image);
            insights.status = Some(inspected.state.status);
            insights.started_at = Some(inspected.state.started_at);

            if inspected.state.running {
                let sample = engine.container_stats(&target.container_name).await?;
                insights.memory_used = Some(sample.memory_used);
                insights.memory_limit = Some(sample.memory_limit);
                insights.net_rx = Some(sample.net_rx);
                insights.net_tx = Some(sample.net_tx);
                insights.block_read = sample.block_read;
                insights.block_write = sample.block_write;

                let mut samples = self.inner.stats_samples.lock().unwrap();
                if let Some(previous) = samples.get(&target.container_name) {
                    insights.cpu_percent = cpu_percent_between(previous, &sample);
                    let rates = rates_between(previous, &sample);
                    insights.net_rx_per_second = rates.net_rx_per_second;
                    insights.net_tx_per_second = rates.net_tx_per_second;
                    insights.block_read_per_second = rates.block_read_per_second;
                    insights.block_write_per_second = rates.block_write_per_second;
                }
                samples.insert(target.container_name.clone(), sample);
            } else {
                self.inner
                    .stats_samples
                    .lock()
                    .unwrap()
                    .remove(&target.container_name);
            }
            Ok(())
        }
        .await;
        if let Err(error) = outcome {
            insights.error = Some(error.to_string());
        }
        Ok(insights)
    }

    /// Run a command inside a session's sandbox, creating it on first use.
    pub async fn exec_command(
        &self,
        session_id: &str,
        command: &str,
        options: SandboxExecOptions,
    ) -> Result<SandboxExecResult> {
        let metadata = self.ensure_sandbox(session_id).await?;
        let target = self
            .to_exec_target(&metadata, options.service.as_deref())
            .await?;
        let options = SandboxExecOptions {
            timeout: Some(options.timeout.unwrap_or_else(|| self.config().timeout)),
            ..options
        };
        self.inner
            .execs
            .exec_in_sandbox(&target, command, &options)
            .await
    }

    pub async fn send_input(&self, pid: u32, stdin: &str) -> Result<SandboxExecResult> {
        self.inner.execs.send_input(pid, stdin, None).await
    }

    pub fn is_tracked_sandbox_pid(&self, pid: u32) -> bool {
        self.inner.execs.is_sandbox_pid(pid)
    }

    pub async fn get_logs(
        &self,
        session_id: &str,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<LogsResult> {
        let metadata = self
            .read_metadata(session_id)
            .ok_or_else(|| Error::msg(format!("No sandbox exists for session {session_id}.")))?;
        let target = self.to_exec_target(&metadata, service).await?;
        read_sandbox_logs(self.inner.runner.as_ref(), &target, tail.unwrap_or(200)).await
    }

    /// Every session id that currently has a sandbox on disk (for the chat-delete sweep).
    pub fn list_sandbox_session_ids(&self) -> Vec<String> {
        let Ok(project) = project_path(self.settings()) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(sandbox_root(&project)) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|entry| {
                let text = std::fs::read_to_string(entry.path().join(METADATA_FILENAME)).ok()?;
                // A half-written metadata file should not hide every other sandbox.
                let parsed: Value = serde_json::from_str(&text).ok()?;
                parsed
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
            })
            .collect()
    }

    /// Stop every sandbox (app quit). They come back up on next use, packages intact.
    pub async fn stop_all_sandboxes(&self) {
        let session_ids = self.list_sandbox_session_ids();
        if session_ids.is_empty() {
            return;
        }
        log::info!(
            target: "docker:sandbox",
            "Stopping all sandboxes on quit count={}",
            session_ids.len()
        );
        let mut tasks = tokio::task::JoinSet::new();
        for session_id in session_ids {
            let manager = self.clone();
            tasks.spawn(async move {
                if let Err(error) = manager.stop_sandbox(&session_id).await {
                    log::error!(
                        target: "docker:sandbox",
                        "Failed to stop sandbox on quit session={session_id} error={error}"
                    );
                }
            });
        }
        while tasks.join_next().await.is_some() {}
    }
}

#[cfg(test)]
mod tests {
    //! Manager behavior that needs no Docker, driven through a fake runner. The live
    //! lifecycle is covered by `tests/sandbox_integration.rs` (`#[ignore]`).
    use super::*;
    use crate::events::RecordingSink;
    use crate::runner::{FakeRunner, RunResult};

    struct Fixture {
        _project: tempfile::TempDir,
        _user_data: tempfile::TempDir,
        project: String,
        user_data: PathBuf,
        runner: Arc<FakeRunner>,
        sink: Arc<RecordingSink>,
        manager: SandboxManager,
    }

    fn docker_ok(command: &str, args: &[String]) -> RunResult {
        match (command, args.first().map(String::as_str)) {
            ("docker", Some("--version")) => RunResult::ok("Docker version 27.3.1, build x\n"),
            ("docker", Some("info")) => RunResult::ok("27.3.1\n"),
            ("docker", Some("compose")) if args.get(1).map(String::as_str) == Some("version") => {
                RunResult::ok("2.29.7\n")
            }
            ("docker", Some("ps")) => RunResult::ok(""),
            _ => RunResult::ok(""),
        }
    }

    fn fixture() -> Fixture {
        let project_dir = tempfile::tempdir().unwrap();
        let user_data_dir = tempfile::tempdir().unwrap();
        let project = project_dir.path().to_string_lossy().into_owned();
        let user_data = user_data_dir.path().to_path_buf();
        let settings_project = project.clone();
        let settings_user_data = user_data.to_string_lossy().into_owned();
        let settings: Arc<dyn Settings> = Arc::new(move |key: &str| match key {
            "projectPath" => Some(Value::String(settings_project.clone())),
            "userDataPath" => Some(Value::String(settings_user_data.clone())),
            "dockerSandboxTool" => {
                Some(serde_json::json!({ "memoryLimit": "512m", "cpuLimit": 1.0 }))
            }
            _ => None,
        });
        let runner = FakeRunner::new();
        runner.respond_with(docker_ok);
        let sink = Arc::new(RecordingSink::default());
        // A remote endpoint makes every Engine call fail fast, so status falls back to `docker ps`.
        let engine = Arc::new(DockerEngine::with_docker_host(
            runner.clone(),
            Some("tcp://127.0.0.1:1".into()),
        ));
        let manager = SandboxManager::with_parts(settings, sink.clone(), runner.clone(), engine);
        Fixture {
            _project: project_dir,
            _user_data: user_data_dir,
            project,
            user_data,
            runner,
            sink,
            manager,
        }
    }

    fn set_chat_title(user_data: &Path, session_id: &str, title: &str) {
        let dir = user_data.join("chat-sessions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{session_id}.json")),
            serde_json::json!({ "id": session_id, "title": title, "messages": [] }).to_string(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn creates_a_sandbox_with_compose_and_matching_names() {
        let f = fixture();
        set_chat_title(&f.user_data, "session_1", "Fix the auth bug");
        let created = f
            .manager
            .create_sandbox("session_1", CreateSandboxOptions::default())
            .await
            .unwrap();

        let dir = PathBuf::from(&created.metadata.directory);
        assert_eq!(
            dir.file_name().unwrap().to_string_lossy(),
            to_folder_name("session_1", Some("Fix the auth bug"))
        );
        assert_eq!(created.metadata.project_name, "bedrock-sandbox-session-1");
        assert!(dir.join("docker-compose.yml").exists());
        assert!(dir.join(".env").exists());
        assert!(dir.join("sandbox.json").exists());
        let gitignore =
            std::fs::read_to_string(Path::new(&f.project).join("docker-sandboxes/.gitignore"))
                .unwrap();
        assert!(gitignore.contains('*'));

        // The configured limits (merged over the defaults) land in the compose file.
        let compose_text = std::fs::read_to_string(dir.join("docker-compose.yml")).unwrap();
        assert!(compose_text.contains("mem_limit: 512m"), "{compose_text}");

        let up = f
            .runner
            .calls()
            .into_iter()
            .find(|(_, args)| args.contains(&"up".to_string()))
            .unwrap();
        assert_eq!(
            up.1,
            [
                "compose".to_string(),
                "-f".into(),
                dir.join("docker-compose.yml")
                    .to_string_lossy()
                    .into_owned(),
                "--env-file".into(),
                dir.join(".env").to_string_lossy().into_owned(),
                "up".into(),
                "-d".into(),
                "--remove-orphans".into()
            ]
        );

        assert_eq!(f.manager.list_sandbox_session_ids(), ["session_1"]);
        assert!(f
            .sink
            .on_channel(&sandbox_state_channel("session_1"))
            .iter()
            .any(|e| matches!(e, SandboxEvent::State(_))));
    }

    #[tokio::test]
    async fn runs_composeless_with_the_same_labels_compose_would_use() {
        let f = fixture();
        f.runner.respond_with(
            |command, args| match (command, args.first().map(String::as_str)) {
                ("docker", Some("compose")) => RunResult::failed(1, "unknown command"),
                ("docker-compose", _) => RunResult {
                    exit_code: 127,
                    spawn_error: Some("spawn docker-compose ENOENT".into()),
                    ..Default::default()
                },
                _ => docker_ok(command, args),
            },
        );
        let created = f
            .manager
            .create_sandbox("session_2", CreateSandboxOptions::default())
            .await
            .unwrap();
        assert!(created.metadata.composeless);

        let run = f
            .runner
            .calls()
            .into_iter()
            .find(|(_, args)| args.first().map(String::as_str) == Some("run"))
            .unwrap();
        let data = Path::new(&created.metadata.directory)
            .join("data")
            .join("main");
        assert_eq!(
            run.1,
            [
                "run".to_string(),
                "-d".into(),
                "--name".into(),
                "bedrock-sandbox-session-2-main-1".into(),
                "--label".into(),
                "com.docker.compose.project=bedrock-sandbox-session-2".into(),
                "--label".into(),
                "com.docker.compose.service=main".into(),
                "--workdir".into(),
                "/workspace".into(),
                "-v".into(),
                format!("{}:/workspace", f.project),
                "-v".into(),
                format!("{}:/data", data.to_string_lossy()),
                "--memory".into(),
                "512m".into(),
                "--cpus".into(),
                "1".into(),
                "-e".into(),
                "DEBIAN_FRONTEND=noninteractive".into(),
                "ubuntu:26.04".into(),
                "sleep".into(),
                "infinity".into()
            ]
        );
    }

    // The composeless path used to drop service `environment` and the agent's `env`.
    #[tokio::test]
    async fn runs_composeless_with_the_service_environment() {
        let f = fixture();
        f.runner.respond_with(
            |command, args| match (command, args.first().map(String::as_str)) {
                ("docker", Some("compose")) => RunResult::failed(1, "unknown command"),
                ("docker-compose", _) => RunResult {
                    exit_code: 127,
                    spawn_error: Some("spawn docker-compose ENOENT".into()),
                    ..Default::default()
                },
                _ => docker_ok(command, args),
            },
        );
        let mut environment = serde_json::Map::new();
        environment.insert("OWN".into(), serde_json::json!("a $b"));
        environment.insert("DEBIAN_FRONTEND".into(), serde_json::json!("readline"));
        let mut env = serde_json::Map::new();
        env.insert("TOKEN".into(), serde_json::json!("x=y"));
        let created = f
            .manager
            .create_sandbox(
                "session_3",
                CreateSandboxOptions {
                    services: Some(vec![crate::types::SandboxServiceSpec {
                        environment: Some(environment),
                        ..crate::types::SandboxServiceSpec::named("main")
                    }]),
                    env: Some(env),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(created.metadata.composeless);

        let run = f
            .runner
            .calls()
            .into_iter()
            .find(|(_, args)| args.first().map(String::as_str) == Some("run"))
            .unwrap();
        let mut env_args: Vec<&str> = run
            .1
            .windows(2)
            .filter(|w| w[0] == "-e")
            .map(|w| w[1].as_str())
            .collect();
        env_args.sort();
        // The service's own DEBIAN_FRONTEND wins over the default; no duplicate is added.
        assert_eq!(
            env_args,
            ["DEBIAN_FRONTEND=readline", "OWN=a $b", "TOKEN=x=y"]
        );
    }

    #[tokio::test]
    async fn reads_electron_written_metadata_and_follows_renames() {
        let f = fixture();
        // A sandbox folder exactly as the Electron build leaves it.
        let old_dir = Path::new(&f.project)
            .join("docker-sandboxes")
            .join("session-abc123");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(
            old_dir.join("sandbox.json"),
            serde_json::json!({
                "sessionId": "session_9",
                "projectName": "bedrock-sandbox-session-9",
                "directory": "/somewhere/else",
                "composeFile": "/somewhere/else/docker-compose.yml",
                "projectPath": f.project,
                "services": [{ "name": "main", "image": "ubuntu:26.04", "ports": [{ "host": 3000, "container": 3000 }] }],
                "composeless": false,
                "createdAt": "2026-09-03T14:22:15.000Z",
                "updatedAt": "2026-09-03T14:22:15.000Z"
            })
            .to_string(),
        )
        .unwrap();

        let status = f.manager.get_status("session_9").await;
        assert!(status.exists);
        let metadata = status.metadata.unwrap();
        // Trust the folder's location over the paths recorded inside it.
        assert_eq!(metadata.directory, old_dir.to_string_lossy());
        assert!(f.manager.is_published_port("session_9", 3000));
        assert!(!f.manager.is_published_port("session_9", 3001));

        set_chat_title(&f.user_data, "session_9", "Rework the login flow");
        let renamed = f.manager.rename_sandbox("session_9").await;
        assert!(renamed.renamed);
        let new_dir = f.manager.get_sandbox_dir("session_9").unwrap();
        assert!(new_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rework-the-login-flow-"));
        assert!(!old_dir.exists());
        assert!(!f.manager.rename_sandbox("session_9").await.renamed);
    }

    #[tokio::test]
    async fn rejects_unknown_services_and_missing_sandboxes() {
        let f = fixture();
        f.manager
            .create_sandbox("session_3", CreateSandboxOptions::default())
            .await
            .unwrap();
        let error = f
            .manager
            .resolve_terminal_target("session_3", Some("nope"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("is not part of this sandbox"));

        let target = f
            .manager
            .resolve_terminal_target("session_3", None)
            .await
            .unwrap();
        assert_eq!(target.container_name, "bedrock-sandbox-session-3-main-1");

        let error = f.manager.start_sandbox("missing").await.unwrap_err();
        assert!(error
            .to_string()
            .contains("No sandbox exists for session missing."));
        assert_eq!(
            f.manager.stop_sandbox("missing").await.unwrap().state,
            SandboxRunState::Missing
        );
    }

    #[tokio::test]
    async fn remove_keeps_or_deletes_the_folder() {
        let f = fixture();
        let kept = f
            .manager
            .create_sandbox("session_4", CreateSandboxOptions::default())
            .await
            .unwrap();
        let dir = PathBuf::from(&kept.metadata.directory);
        let result = f
            .manager
            .remove_sandbox(
                "session_4",
                SandboxRemoveOptions {
                    delete_data: Some(false),
                },
            )
            .await
            .unwrap();
        assert!(result.removed && !result.data_deleted);
        assert!(dir.join("docker-compose.yml").exists());
        assert!(!dir.join("sandbox.json").exists());
        assert!(!f.manager.get_status("session_4").await.exists);

        let wiped = f
            .manager
            .create_sandbox("session_5", CreateSandboxOptions::default())
            .await
            .unwrap();
        let result = f
            .manager
            .remove_sandbox(
                "session_5",
                SandboxRemoveOptions {
                    delete_data: Some(true),
                },
            )
            .await
            .unwrap();
        assert!(result.data_deleted);
        assert!(!Path::new(&wiped.metadata.directory).exists());
        assert!(f.runner.calls().iter().any(|(_, args)| args.ends_with(&[
            "down".into(),
            "-v".into(),
            "--remove-orphans".into()
        ])));
    }

    #[tokio::test]
    async fn derives_state_from_docker_ps_rows() {
        let f = fixture();
        f.manager
            .create_sandbox("session_6", CreateSandboxOptions::default())
            .await
            .unwrap();
        f.runner
            .respond_with(|command, args| match args.first().map(String::as_str) {
                Some("ps") => RunResult::ok("main\tbedrock-sandbox-session-6-main-1\trunning\t\n"),
                _ => docker_ok(command, args),
            });
        assert_eq!(
            f.manager.get_status("session_6").await.state,
            SandboxRunState::Running
        );
        assert_eq!(derive_state(None, &[]), SandboxRunState::Missing);
    }

    #[test]
    fn project_path_is_required() {
        let settings = |_: &str| None;
        let error = project_path(&settings).unwrap_err();
        assert!(error
            .to_string()
            .contains("No project directory is configured"));
    }
}
