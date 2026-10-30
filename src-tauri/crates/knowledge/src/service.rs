//! The knowledgebase service behind the `knowledge_*` commands and the built-in MCP server.
//!
//! Layout: `<root>/<agentId>/<kbId>.db`, one SQLite file per knowledgebase (see [`crate::store`]);
//! the on-device model lives in `<root>/.models` (a name no agent id can have).
//!
//! Ingestion runs as background jobs, one at a time per knowledgebase (a per-knowledgebase
//! lock), each with its own cancellation token. Progress goes out as `knowledge:progress`
//! events; `knowledge:changed` follows any change to a knowledgebase's sources or contents.

use crate::chunk::{chunk_text, TextKind};
use crate::crawl::{crawl, CrawlOptions};
use crate::embed::{batch_size, embed_checked, EmbedInput, Embedder, EmbedderProvider};
use crate::extract::{extract_file, format_for_path, supported_files_in, LARGE_FOLDER_FILES};
use crate::search::hybrid_search;
use crate::store::{db_files, db_size, embed_text, KbMeta, KbStore, NewChunk};
use crate::types::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub const PROGRESS_EVENT: &str = "knowledge:progress";
pub const CHANGED_EVENT: &str = "knowledge:changed";

/// Days of usage the dashboards show.
pub const USAGE_DAYS: usize = 30;
/// A website not synced for this long is flagged as stale.
const STALE_WEBSITE_DAYS: i64 = 30;
/// Free space worth flagging: this share of the file and at least this many bytes.
const RECLAIM_FLAG_SHARE: f64 = 0.2;
const RECLAIM_FLAG_BYTES: u64 = 1024 * 1024;
/// After a job, optimize automatically past this share and size of free space.
const AUTO_OPTIMIZE_SHARE: f64 = 0.25;
const AUTO_OPTIMIZE_BYTES: u64 = 4 * 1024 * 1024;
/// The first automatic re-sync of file and folder sources runs about this long after launch.
const AUTO_SYNC_FIRST_DELAY: Duration = Duration::from_secs(30);
/// How often the auto-sync loop checks whether a pass is due (and re-reads the settings).
const AUTO_SYNC_TICK: Duration = Duration::from_secs(30);

/// Modification time in ms since the epoch; `None` when the file can't be read.
fn file_mtime(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Name of the directory under the root that holds downloaded models.
pub const MODELS_DIR: &str = ".models";

/// What the service needs from the app.
pub trait KnowledgeHost: Send + Sync {
    /// The `knowledgebase` store key.
    fn settings(&self) -> KnowledgeSettings;
    /// HTTP client for crawling (with the app's proxy).
    fn http_client(&self) -> reqwest::Client;
    fn emit(&self, event: &str, payload: Value);
}

type Key = (String, String);

struct ActiveJob {
    info: JobInfo,
    token: CancellationToken,
}

enum JobKind {
    Source(String),
    Rebuild(EmbeddingSpec),
}

pub struct KnowledgeService {
    root: PathBuf,
    host: Arc<dyn KnowledgeHost>,
    embedders: Arc<dyn EmbedderProvider>,
    runtime: tokio::runtime::Handle,
    handles: Mutex<HashMap<Key, Arc<KbStore>>>,
    locks: Mutex<HashMap<Key, Arc<tokio::sync::Mutex<()>>>>,
    jobs: Mutex<HashMap<String, ActiveJob>>,
    /// Stops the auto-sync loop (app quit).
    auto_sync_stop: CancellationToken,
}

fn validate_id(kind: &str, id: &str) -> Result<(), String> {
    if common::json_file::is_safe_file_id(id) {
        Ok(())
    } else {
        Err(format!("Invalid {kind} id: {id}"))
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn new_id(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
}

fn hash(text: &str) -> String {
    let d = Sha256::digest(text.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

const CANCELLED: &str = "Cancelled";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Reports one job's progress.
#[derive(Clone)]
struct Reporter {
    svc: Arc<KnowledgeService>,
    job_id: String,
    token: CancellationToken,
}

impl Reporter {
    fn update(&self, phase: JobPhase, current: usize, total: usize, message: impl Into<String>) {
        let info = {
            let mut jobs = lock(&self.svc.jobs);
            let Some(job) = jobs.get_mut(&self.job_id) else {
                return;
            };
            job.info.phase = phase;
            job.info.current = current;
            job.info.total = total;
            job.info.message = message.into();
            job.info.clone()
        };
        self.svc.emit_progress(&info);
    }

    fn check(&self) -> Result<(), String> {
        if self.token.is_cancelled() {
            Err(CANCELLED.into())
        } else {
            Ok(())
        }
    }
}

impl KnowledgeService {
    pub fn new(
        root: PathBuf,
        host: Arc<dyn KnowledgeHost>,
        embedders: Arc<dyn EmbedderProvider>,
        runtime: tokio::runtime::Handle,
    ) -> Arc<Self> {
        Arc::new(Self {
            root,
            host,
            embedders,
            runtime,
            handles: Mutex::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            auto_sync_stop: CancellationToken::new(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn models_dir(&self) -> PathBuf {
        self.root.join(MODELS_DIR)
    }

    pub fn settings(&self) -> KnowledgeSettings {
        self.host.settings()
    }

    fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.root.join(agent_id)
    }

    fn kb_path(&self, agent_id: &str, kb_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join(format!("{kb_id}.db"))
    }

    fn emit_progress(&self, info: &JobInfo) {
        self.host.emit(
            PROGRESS_EVENT,
            serde_json::to_value(info).unwrap_or(Value::Null),
        );
    }

    fn emit_changed(&self, agent_id: &str, kb_id: &str) {
        self.host
            .emit(CHANGED_EVENT, json!({ "agentId": agent_id, "kbId": kb_id }));
    }

    fn kb_lock(&self, agent_id: &str, kb_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        lock(&self.locks)
            .entry((agent_id.into(), kb_id.into()))
            .or_default()
            .clone()
    }

    /// The open store for a knowledgebase (opened on first use).
    fn store(&self, agent_id: &str, kb_id: &str) -> Result<Arc<KbStore>, String> {
        validate_id("agent", agent_id)?;
        validate_id("knowledge base", kb_id)?;
        let key = (agent_id.to_string(), kb_id.to_string());
        if let Some(s) = lock(&self.handles).get(&key) {
            return Ok(s.clone());
        }
        let s = Arc::new(KbStore::open(&self.kb_path(agent_id, kb_id))?);
        Ok(lock(&self.handles).entry(key).or_insert(s).clone())
    }

    fn close(&self, agent_id: &str, kb_id: &str) {
        lock(&self.handles).remove(&(agent_id.to_string(), kb_id.to_string()));
    }

    fn kb_ids(&self, agent_id: &str) -> Result<Vec<String>, String> {
        validate_id("agent", agent_id)?;
        let dir = self.agent_dir(agent_id);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".db")?.to_string();
                common::json_file::is_safe_file_id(&id).then_some(id)
            })
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn info(&self, agent_id: &str, kb_id: &str) -> Result<KbInfo, String> {
        let s = self.store(agent_id, kb_id)?;
        let meta = s.meta()?;
        let counts = s.counts()?;
        Ok(KbInfo {
            id: kb_id.into(),
            agent_id: agent_id.into(),
            name: meta.name,
            description: meta.description,
            embedding: meta.embedding,
            chunk_size: meta.chunk_size,
            chunk_overlap: meta.chunk_overlap,
            created_at: meta.created_at,
            source_count: counts.sources,
            document_count: counts.documents,
            chunk_count: counts.chunks,
            bytes: db_size(s.path()),
        })
    }

    // ---- knowledgebases ----

    /// The agent's knowledgebases, oldest first. Unreadable files are skipped (logged).
    pub fn list(&self, agent_id: &str) -> Result<Vec<KbInfo>, String> {
        let mut out = Vec::new();
        for id in self.kb_ids(agent_id)? {
            match self.info(agent_id, &id) {
                Ok(i) => out.push(i),
                Err(e) => {
                    tracing::warn!(agent_id, kb_id = %id, error = %e, "Skipping unreadable knowledge base")
                }
            }
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// Create a knowledgebase with the current global embedding and chunking settings.
    pub fn create(&self, agent_id: &str, name: &str, description: &str) -> Result<KbInfo, String> {
        validate_id("agent", agent_id)?;
        let name = name.trim();
        if name.is_empty() {
            return Err("Knowledge base name is required".into());
        }
        let settings = self.host.settings();
        let spec = settings.embedding.spec();
        if spec.provider == Provider::Local && !crate::local::is_installed(&self.models_dir()) {
            return Err(
                "The local embedding model is not downloaded yet. Download it in Settings > Knowledge Base, or switch to Bedrock embeddings."
                    .into(),
            );
        }
        let kb_id = new_id("kb");
        KbStore::create(
            &self.kb_path(agent_id, &kb_id),
            &KbMeta {
                name: name.into(),
                description: description.trim().into(),
                embedding: spec,
                chunk_size: settings.chunk_size,
                chunk_overlap: settings.chunk_overlap,
                created_at: now(),
            },
        )?;
        self.emit_changed(agent_id, &kb_id);
        self.info(agent_id, &kb_id)
    }

    pub fn update(
        &self,
        agent_id: &str,
        kb_id: &str,
        name: &str,
        description: &str,
    ) -> Result<KbInfo, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Knowledge base name is required".into());
        }
        self.store(agent_id, kb_id)?
            .set_name(name, description.trim())?;
        self.emit_changed(agent_id, kb_id);
        self.info(agent_id, kb_id)
    }

    fn cancel_where(&self, f: impl Fn(&JobInfo) -> bool) {
        for job in lock(&self.jobs).values() {
            if f(&job.info) {
                job.token.cancel();
            }
        }
    }

    /// Delete a knowledgebase. Running jobs are cancelled first.
    pub async fn delete(&self, agent_id: &str, kb_id: &str) -> Result<(), String> {
        validate_id("agent", agent_id)?;
        validate_id("knowledge base", kb_id)?;
        self.cancel_where(|j| j.agent_id == agent_id && j.kb_id == kb_id);
        let kb_lock = self.kb_lock(agent_id, kb_id);
        let _guard = kb_lock.lock().await;
        self.close(agent_id, kb_id);
        for f in db_files(&self.kb_path(agent_id, kb_id)) {
            match std::fs::remove_file(&f) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Cannot delete {}: {e}", f.display())),
            }
        }
        let dir = self.agent_dir(agent_id);
        if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&dir);
        }
        self.emit_changed(agent_id, kb_id);
        Ok(())
    }

    /// Delete every knowledgebase of an agent (agent deleted, or a new agent discarded).
    pub async fn delete_agent(&self, agent_id: &str) -> Result<(), String> {
        for kb in self.kb_ids(agent_id)? {
            self.delete(agent_id, &kb).await?;
        }
        let dir = self.agent_dir(agent_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .map_err(|e| format!("Cannot delete {}: {e}", dir.display()))?;
        }
        Ok(())
    }

    /// Copy an agent's knowledgebases to another agent (agent duplicated). Ids are kept.
    pub async fn copy_agent(&self, from: &str, to: &str) -> Result<Vec<KbInfo>, String> {
        validate_id("agent", to)?;
        if from == to {
            return Err("Source and target agent are the same".into());
        }
        for kb in self.kb_ids(from)? {
            let dest = self.kb_path(to, &kb);
            if dest.exists() {
                return Err(format!(
                    "Knowledge base {kb} already exists for the target agent"
                ));
            }
            let s = self.store(from, &kb)?;
            blocking(move || s.copy_to(&dest)).await?;
        }
        for kb in self.kb_ids(to)? {
            self.emit_changed(to, &kb);
        }
        self.list(to)
    }

    // ---- sources ----

    /// Sources with their status. A source left queued or processing by an earlier run of
    /// the app (no job for it now) shows as interrupted.
    pub fn list_sources(&self, agent_id: &str, kb_id: &str) -> Result<Vec<SourceInfo>, String> {
        let mut sources = self.store(agent_id, kb_id)?.list_sources()?;
        let active: HashSet<String> = lock(&self.jobs)
            .values()
            .filter(|j| j.info.agent_id == agent_id && j.info.kb_id == kb_id)
            .filter_map(|j| j.info.source_id.clone())
            .collect();
        for s in &mut sources {
            if matches!(s.status, SourceStatus::Queued | SourceStatus::Processing)
                && !active.contains(&s.id)
            {
                s.status = SourceStatus::Error;
                s.error = Some("Interrupted. Re-sync to continue.".into());
            }
        }
        Ok(sources)
    }

    /// Add files and queue their ingestion. Every path is checked first; nothing is added if
    /// one is unusable.
    pub fn add_files(
        self: &Arc<Self>,
        agent_id: &str,
        kb_id: &str,
        paths: &[String],
    ) -> Result<Vec<SourceInfo>, String> {
        let store = self.store(agent_id, kb_id)?;
        let existing: HashSet<String> = store.list_sources()?.into_iter().map(|s| s.uri).collect();
        let mut files = Vec::new();
        for p in paths {
            let path = PathBuf::from(p);
            if !path.is_absolute() {
                return Err(format!("Not an absolute path: {p}"));
            }
            if !path.is_file() {
                return Err(format!("File not found: {p}"));
            }
            if format_for_path(&path).is_none() {
                return Err(format!("Unsupported file type: {p}"));
            }
            let uri = path.to_string_lossy().into_owned();
            if existing.contains(&uri) || files.iter().any(|(u, _)| u == &uri) {
                continue;
            }
            let title = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| uri.clone());
            files.push((uri, title));
        }
        let mut out = Vec::new();
        for (uri, title) in files {
            let id = new_id("src");
            store.add_source(&id, SourceKind::File, &uri, &title, None, &now())?;
            self.spawn(agent_id, kb_id, JobKind::Source(id.clone()));
            if let Some(s) = store.source(&id)? {
                out.push(s);
            }
        }
        self.emit_changed(agent_id, kb_id);
        Ok(out)
    }

    /// Add a folder and queue its ingestion: every supported file under it becomes a document
    /// of the one source, re-scanned on each sync. Errors when it has no supported files. With
    /// [`LARGE_FOLDER_FILES`] or more, `confirm` is asked with the count first and `None`
    /// returned when it declines. The walk runs here, so call it from a blocking thread.
    pub fn add_folder(
        self: &Arc<Self>,
        agent_id: &str,
        kb_id: &str,
        path: &str,
        confirm: impl FnOnce(usize) -> bool,
    ) -> Result<Option<SourceInfo>, String> {
        let store = self.store(agent_id, kb_id)?;
        let dir = PathBuf::from(path);
        if !dir.is_absolute() {
            return Err(format!("Not an absolute path: {path}"));
        }
        if !dir.is_dir() {
            return Err(format!("Folder not found: {path}"));
        }
        let uri = dir.to_string_lossy().into_owned();
        if store.list_sources()?.iter().any(|s| s.uri == uri) {
            return Err("This folder is already a source of the knowledge base".into());
        }
        let count = supported_files_in(&dir)?.len();
        if count == 0 {
            return Err("The folder has no supported files".into());
        }
        if count >= LARGE_FOLDER_FILES && !confirm(count) {
            return Ok(None);
        }
        let title = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| uri.clone());
        let id = new_id("src");
        store.add_source(&id, SourceKind::Folder, &uri, &title, None, &now())?;
        self.spawn(agent_id, kb_id, JobKind::Source(id.clone()));
        self.emit_changed(agent_id, kb_id);
        store
            .source(&id)?
            .map(Some)
            .ok_or_else(|| "Source not found".into())
    }

    /// Add a website and queue its crawl.
    pub fn add_url(
        self: &Arc<Self>,
        agent_id: &str,
        kb_id: &str,
        url: &str,
        options: UrlOptions,
    ) -> Result<SourceInfo, String> {
        let store = self.store(agent_id, kb_id)?;
        let parsed = url::Url::parse(url.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err("Only http and https URLs can be crawled".into());
        }
        let uri = parsed.to_string();
        if store.list_sources()?.iter().any(|s| s.uri == uri) {
            return Err("This URL is already a source of the knowledge base".into());
        }
        let id = new_id("src");
        let title = format!("{}{}", parsed.host_str().unwrap_or_default(), parsed.path());
        store.add_source(
            &id,
            SourceKind::Url,
            &uri,
            &title,
            Some(&options.clamped()),
            &now(),
        )?;
        self.spawn(agent_id, kb_id, JobKind::Source(id.clone()));
        self.emit_changed(agent_id, kb_id);
        store.source(&id)?.ok_or_else(|| "Source not found".into())
    }

    /// Re-ingest a source (unchanged documents are skipped). Returns the job id; a source
    /// that is already being processed keeps its current job.
    pub fn resync_source(
        self: &Arc<Self>,
        agent_id: &str,
        kb_id: &str,
        source_id: &str,
    ) -> Result<String, String> {
        let store = self.store(agent_id, kb_id)?;
        if store.source(source_id)?.is_none() {
            return Err("Source not found".into());
        }
        if let Some(job) = lock(&self.jobs).values().find(|j| {
            j.info.agent_id == agent_id
                && j.info.kb_id == kb_id
                && j.info.source_id.as_deref() == Some(source_id)
        }) {
            return Ok(job.info.job_id.clone());
        }
        store.set_source_status(source_id, SourceStatus::Queued, None)?;
        let id = self.spawn(agent_id, kb_id, JobKind::Source(source_id.into()));
        self.emit_changed(agent_id, kb_id);
        Ok(id)
    }

    /// Change a website source's crawl options; they apply from its next sync.
    pub fn update_source_options(
        &self,
        agent_id: &str,
        kb_id: &str,
        source_id: &str,
        options: UrlOptions,
    ) -> Result<SourceInfo, String> {
        let store = self.store(agent_id, kb_id)?;
        store.set_source_options(source_id, &options.clamped())?;
        self.emit_changed(agent_id, kb_id);
        store
            .source(source_id)?
            .ok_or_else(|| "Source not found".into())
    }

    /// Remove a source and its indexed content (cancelling its job).
    pub async fn remove_source(
        &self,
        agent_id: &str,
        kb_id: &str,
        source_id: &str,
    ) -> Result<(), String> {
        let store = self.store(agent_id, kb_id)?;
        self.cancel_where(|j| {
            j.agent_id == agent_id && j.kb_id == kb_id && j.source_id.as_deref() == Some(source_id)
        });
        let kb_lock = self.kb_lock(agent_id, kb_id);
        let _guard = kb_lock.lock().await;
        let id = source_id.to_string();
        blocking(move || store.remove_source(&id)).await?;
        self.emit_changed(agent_id, kb_id);
        Ok(())
    }

    /// Re-embed every chunk with the current global embedding model.
    pub fn rebuild(self: &Arc<Self>, agent_id: &str, kb_id: &str) -> Result<String, String> {
        self.store(agent_id, kb_id)?;
        let spec = self.host.settings().embedding.spec();
        if spec.provider == Provider::Local && !crate::local::is_installed(&self.models_dir()) {
            return Err("The local embedding model is not downloaded yet.".into());
        }
        if lock(&self.jobs).values().any(|j| {
            j.info.agent_id == agent_id && j.info.kb_id == kb_id && j.info.source_id.is_none()
        }) {
            return Err("A rebuild is already running".into());
        }
        Ok(self.spawn(agent_id, kb_id, JobKind::Rebuild(spec)))
    }

    // ---- jobs ----

    pub fn jobs(&self, agent_id: Option<&str>) -> Vec<JobInfo> {
        let mut v: Vec<JobInfo> = lock(&self.jobs)
            .values()
            .filter(|j| agent_id.is_none_or(|a| j.info.agent_id == a))
            .map(|j| j.info.clone())
            .collect();
        v.sort_by(|a, b| a.job_id.cmp(&b.job_id));
        v
    }

    pub fn cancel_job(&self, job_id: &str) -> bool {
        match lock(&self.jobs).get(job_id) {
            Some(j) => {
                j.token.cancel();
                true
            }
            None => false,
        }
    }

    /// Cancel every job and stop the auto-sync loop (app quit).
    pub fn cancel_all(&self) {
        self.auto_sync_stop.cancel();
        self.cancel_where(|_| true);
    }

    /// Re-sync file and folder sources in the background: first shortly after launch, then
    /// every `knowledgebase.autoSync.intervalMinutes`. Only sources whose files changed get a
    /// job, so an idle knowledge base costs a directory scan and nothing else.
    pub fn start_auto_sync(self: &Arc<Self>) {
        let svc = self.clone();
        self.runtime.spawn(async move {
            let stop = svc.auto_sync_stop.clone();
            let first = tokio::time::Instant::now() + AUTO_SYNC_FIRST_DELAY;
            let mut last: Option<tokio::time::Instant> = None;
            loop {
                tokio::select! {
                    _ = stop.cancelled() => return,
                    _ = tokio::time::sleep(AUTO_SYNC_TICK) => {}
                }
                // Due time from the current settings, so a new interval applies at once.
                let auto = svc.host.settings().auto_sync;
                let due = last.map_or(first, |l| {
                    l + Duration::from_secs(auto.interval_minutes * 60)
                });
                if !auto.enabled || tokio::time::Instant::now() < due {
                    continue;
                }
                let s = svc.clone();
                let queued = blocking(move || Ok(s.auto_sync_pass()))
                    .await
                    .unwrap_or_default();
                if queued > 0 {
                    tracing::info!(queued, "Knowledge base auto-sync queued changed sources");
                }
                last = Some(tokio::time::Instant::now());
            }
        });
    }

    /// Queue a re-sync of every file and folder source whose files changed; returns how many.
    pub fn auto_sync_pass(self: &Arc<Self>) -> usize {
        let mut queued = 0;
        for agent in self.agent_ids() {
            for kb in self.kb_ids(&agent).unwrap_or_default() {
                let Ok(store) = self.store(&agent, &kb) else {
                    continue;
                };
                for src in store.list_sources().unwrap_or_default() {
                    if matches!(src.status, SourceStatus::Queued | SourceStatus::Processing) {
                        continue;
                    }
                    if self.source_changed(&store, &src)
                        && self.resync_source(&agent, &kb, &src.id).is_ok()
                    {
                        queued += 1;
                    }
                }
            }
        }
        queued
    }

    /// Whether a file or folder source's files differ from what was indexed (always `false`
    /// for websites). A source that is missing on disk counts once: after its sync has
    /// recorded the error, it is left alone until it reappears.
    fn source_changed(&self, store: &KbStore, src: &SourceInfo) -> bool {
        let path = Path::new(&src.uri);
        let Ok(known) = store.document_stamps(&src.id) else {
            return false;
        };
        let known: HashMap<String, Option<i64>> =
            known.into_iter().map(|(uri, (m, _))| (uri, m)).collect();
        let current: HashMap<String, Option<i64>> = match src.kind {
            SourceKind::Url => return false,
            SourceKind::File => match file_mtime(path) {
                Some(m) => HashMap::from([(src.uri.clone(), Some(m))]),
                None => return src.status != SourceStatus::Error,
            },
            SourceKind::Folder => match supported_files_in(path) {
                Ok(files) => files
                    .into_iter()
                    .map(|f| {
                        let m = file_mtime(&f);
                        (f.to_string_lossy().into_owned(), m)
                    })
                    .collect(),
                Err(_) => return src.status != SourceStatus::Error,
            },
        };
        current != known
    }

    /// Agent folders under the root (not the user guide's index).
    fn agent_ids(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut agents: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| common::json_file::is_safe_file_id(n))
            // The user guide's index belongs to the app (restored from the bundle), not to
            // the user: never listed, never purged.
            .filter(|n| n != crate::help::HELP_OWNER)
            .collect();
        agents.sort();
        agents
    }

    fn spawn(self: &Arc<Self>, agent_id: &str, kb_id: &str, kind: JobKind) -> String {
        let job_id = new_id("job");
        let token = CancellationToken::new();
        let info = JobInfo {
            job_id: job_id.clone(),
            agent_id: agent_id.into(),
            kb_id: kb_id.into(),
            source_id: match &kind {
                JobKind::Source(s) => Some(s.clone()),
                JobKind::Rebuild(_) => None,
            },
            phase: JobPhase::Queued,
            current: 0,
            total: 0,
            message: String::new(),
        };
        self.emit_progress(&info);
        lock(&self.jobs).insert(
            job_id.clone(),
            ActiveJob {
                info,
                token: token.clone(),
            },
        );
        let reporter = Reporter {
            svc: self.clone(),
            job_id: job_id.clone(),
            token,
        };
        let (agent, kb) = (agent_id.to_string(), kb_id.to_string());
        self.runtime.spawn(async move {
            reporter
                .svc
                .clone()
                .run_job(reporter, agent, kb, kind)
                .await;
        });
        job_id
    }

    async fn run_job(self: Arc<Self>, rep: Reporter, agent: String, kb: String, kind: JobKind) {
        let started = std::time::Instant::now();
        let kb_lock = self.kb_lock(&agent, &kb);
        let guard = tokio::select! {
            g = kb_lock.lock() => Some(g),
            _ = rep.token.cancelled() => None,
        };
        let source_id = match &kind {
            JobKind::Source(s) => Some(s.clone()),
            JobKind::Rebuild(_) => None,
        };
        let ran = guard.is_some();
        let result = match guard {
            None => Err(CANCELLED.to_string()),
            Some(_) => match self.store(&agent, &kb) {
                Err(e) => Err(e),
                Ok(store) => match &kind {
                    JobKind::Source(src) => self.run_source(&rep, &store, src).await,
                    JobKind::Rebuild(spec) => self.run_rebuild(&rep, &store, spec).await,
                },
            },
        };
        // The source may be gone (removed while queued); status updates are best effort.
        let store = self.store(&agent, &kb).ok();
        match &result {
            Ok(msg) => {
                if let (Some(store), Some(src)) = (&store, &source_id) {
                    let _ = store.set_source_status(src, SourceStatus::Ready, None);
                }
                rep.update(JobPhase::Done, 0, 0, msg.clone());
            }
            Err(e) if e == CANCELLED => {
                if let (Some(store), Some(src)) = (&store, &source_id) {
                    let _ =
                        store.set_source_status(src, SourceStatus::Cancelled, Some("Cancelled"));
                }
                rep.update(JobPhase::Cancelled, 0, 0, CANCELLED);
            }
            Err(e) => {
                tracing::warn!(agent_id = %agent, kb_id = %kb, error = %e, "Knowledge base job failed");
                if let (Some(store), Some(src)) = (&store, &source_id) {
                    let _ = store.set_source_status(src, SourceStatus::Error, Some(e));
                }
                rep.update(JobPhase::Error, 0, 0, e.clone());
            }
        }
        if let (true, Some(store)) = (ran, &store) {
            self.after_job(store, &kind, &result, started.elapsed())
                .await;
        }
        lock(&self.jobs).remove(&rep.job_id);
        self.emit_changed(&agent, &kb);
    }

    /// Log the job, fold the WAL back in, and optimize when enough space is free.
    async fn after_job(
        &self,
        store: &Arc<KbStore>,
        kind: &JobKind,
        result: &Result<String, String>,
        elapsed: Duration,
    ) {
        let (kind_name, source_id, documents, chunks) = match kind {
            JobKind::Source(src) => {
                let s = store.source(src).ok().flatten();
                (
                    "source",
                    Some(src.clone()),
                    s.as_ref().map_or(0, |s| s.document_count),
                    s.as_ref().map_or(0, |s| s.chunk_count),
                )
            }
            JobKind::Rebuild(_) => {
                let c = store.counts().unwrap_or_default();
                ("rebuild", None, c.documents, c.chunks)
            }
        };
        let (status, message) = match result {
            Ok(m) => ("done", m.clone()),
            Err(e) if e == CANCELLED => ("cancelled", e.clone()),
            Err(e) => ("error", e.clone()),
        };
        let entry = crate::store::JobLogEntry {
            at: now_ms(),
            kind: kind_name.into(),
            source_id,
            duration_ms: elapsed.as_millis() as u64,
            documents,
            chunks,
            status: status.into(),
            message,
        };
        let s = store.clone();
        let _ = blocking(move || {
            s.log_job(&entry)?;
            s.checkpoint();
            let (page, pages, free) = s.pages()?;
            let free_bytes = page * free;
            if free_bytes >= AUTO_OPTIMIZE_BYTES
                && free as f64 >= AUTO_OPTIMIZE_SHARE * pages.max(1) as f64
            {
                tracing::info!(path = %s.path().display(), free_bytes, "Optimizing knowledge base");
                optimize_store(&s)?;
            }
            Ok(())
        })
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "Knowledge base maintenance after job failed"));
    }

    async fn run_source(
        &self,
        rep: &Reporter,
        store: &Arc<KbStore>,
        source_id: &str,
    ) -> Result<String, String> {
        let Some(source) = store.source(source_id)? else {
            return Err(CANCELLED.into());
        };
        store.set_source_status(source_id, SourceStatus::Processing, None)?;
        let meta = store.meta()?;
        let embedder = self.embedders.embedder(&meta.embedding)?;
        let ctx = IngestCtx {
            store: store.clone(),
            meta: meta.clone(),
            embedder,
            rep: rep.clone(),
        };
        match source.kind {
            SourceKind::File => {
                let path = PathBuf::from(&source.uri);
                let mtime = file_mtime(&path);
                // Unmodified since it was indexed. A manual re-sync of a file that failed to
                // extract still tries again.
                let known = store.document_stamps(source_id)?;
                if mtime.is_some() && known.get(&source.uri) == Some(&(mtime, false)) {
                    store.finish_source(source_id, None, &now())?;
                    return Ok("Unchanged".into());
                }
                rep.update(JobPhase::Extracting, 0, 1, source.title.clone());
                let extracted = match blocking(move || extract_file(&path)).await {
                    Ok(x) => x,
                    Err(e) => {
                        if let Some(m) = mtime {
                            ctx.mark_failed(source_id, &source.uri, m).await?;
                        }
                        return Err(e);
                    }
                };
                rep.check()?;
                let title = extracted.title.clone();
                let changed = ctx
                    .ingest(
                        source_id,
                        &source.uri,
                        &title,
                        &extracted.text,
                        extracted.kind,
                        mtime,
                        0,
                        1,
                    )
                    .await?;
                store.finish_source(source_id, Some(&title), &now())?;
                Ok(if changed {
                    "Indexed".into()
                } else {
                    "Unchanged".into()
                })
            }
            SourceKind::Folder => {
                rep.update(JobPhase::Fetching, 0, 0, source.title.clone());
                let dir = PathBuf::from(&source.uri);
                let files = blocking(move || supported_files_in(&dir)).await?;
                rep.check()?;
                let known = store.document_stamps(source_id)?;
                let total = files.len();
                let (mut changed, mut skipped) = (0, 0);
                let mut keep = HashSet::new();
                for (i, file) in files.into_iter().enumerate() {
                    rep.check()?;
                    let uri = file.to_string_lossy().into_owned();
                    keep.insert(uri.clone());
                    let mtime = file_mtime(&file);
                    if mtime.is_some() && known.get(&uri) == Some(&(mtime, false)) {
                        continue;
                    }
                    let name = file
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| uri.clone());
                    rep.update(JobPhase::Extracting, i, total, name);
                    let extracted = match blocking(move || extract_file(&file)).await {
                        Ok(x) => x,
                        Err(e) => {
                            tracing::info!(uri = %uri, error = %e, "Skipping folder file");
                            if let Some(m) = mtime {
                                ctx.mark_failed(source_id, &uri, m).await?;
                            }
                            skipped += 1;
                            continue;
                        }
                    };
                    if ctx
                        .ingest(
                            source_id,
                            &uri,
                            &extracted.title,
                            &extracted.text,
                            extracted.kind,
                            mtime,
                            i,
                            total,
                        )
                        .await?
                    {
                        changed += 1;
                    }
                }
                // Files deleted (or no longer supported) since the last sync.
                let mut removed = 0;
                for uri in known.keys() {
                    if !keep.contains(uri) {
                        store.remove_document(source_id, uri)?;
                        removed += 1;
                    }
                }
                store.finish_source(source_id, None, &now())?;
                let mut msg = format!("{total} files, {changed} updated");
                if removed > 0 {
                    msg.push_str(&format!(", {removed} removed"));
                }
                if skipped > 0 {
                    msg.push_str(&format!(", {skipped} skipped"));
                }
                Ok(msg)
            }
            SourceKind::Url => {
                let opts = source.options.clone().unwrap_or_default().clamped();
                let settings = self.host.settings();
                let crawl_opts = CrawlOptions {
                    max_depth: opts.max_depth,
                    max_pages: opts.max_pages,
                    path_prefix_only: opts.path_prefix_only,
                    delay: Duration::from_millis(settings.crawler.delay_ms),
                    respect_robots: settings.crawler.respect_robots,
                    use_sitemap: settings.crawler.use_sitemap,
                };
                let progress_rep = rep.clone();
                let progress = move |done: usize, total: usize, url: &str| {
                    progress_rep.update(JobPhase::Fetching, done, total, url.to_string());
                };
                let client = self.host.http_client();
                let outcome =
                    crawl(&client, &source.uri, &crawl_opts, &rep.token, &progress).await?;
                rep.check()?;
                let total = outcome.pages.len();
                let mut changed = 0;
                let mut keep = HashSet::new();
                for (i, page) in outcome.pages.iter().enumerate() {
                    keep.insert(page.url.clone());
                    if ctx
                        .ingest(
                            source_id,
                            &page.url,
                            &page.title,
                            &page.text,
                            page.kind,
                            None,
                            i,
                            total,
                        )
                        .await?
                    {
                        changed += 1;
                    }
                }
                // Pages that are no longer reachable.
                let mut removed = 0;
                for uri in store.document_uris(source_id)? {
                    if !keep.contains(&uri) {
                        store.remove_document(source_id, &uri)?;
                        removed += 1;
                    }
                }
                let title = outcome.pages.first().map(|p| p.title.clone());
                store.finish_source(source_id, title.as_deref(), &now())?;
                let mut msg = format!("{total} pages, {changed} updated");
                if removed > 0 {
                    msg.push_str(&format!(", {removed} removed"));
                }
                if !outcome.skipped.is_empty() {
                    msg.push_str(&format!(", {} skipped", outcome.skipped.len()));
                }
                Ok(msg)
            }
        }
    }

    async fn run_rebuild(
        &self,
        rep: &Reporter,
        store: &Arc<KbStore>,
        spec: &EmbeddingSpec,
    ) -> Result<String, String> {
        let embedder = self.embedders.embedder(spec)?;
        let s = store.clone();
        let texts = blocking(move || s.all_embed_texts()).await?;
        let total = texts.len();
        let mut vectors = Vec::with_capacity(total);
        for batch in texts.chunks(batch_size(spec)) {
            rep.check()?;
            rep.update(JobPhase::Embedding, vectors.len(), total, "Re-embedding");
            let input: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
            let out = embed_checked(
                embedder.as_ref(),
                &input,
                EmbedInput::Document,
                spec.dimensions,
            )
            .await?;
            vectors.extend(batch.iter().map(|(id, _)| *id).zip(out));
        }
        rep.check()?;
        rep.update(JobPhase::Indexing, total, total, "Saving");
        let (s, spec) = (store.clone(), spec.clone());
        blocking(move || s.replace_embeddings(&spec, &vectors)).await?;
        Ok(format!("Re-embedded {total} chunks"))
    }

    // ---- search ----

    /// Search an agent's knowledgebases. `kb` narrows to one, by id or (case-insensitive) name.
    pub async fn search(
        &self,
        agent_id: &str,
        kb: Option<&str>,
        query: &str,
        top_k: Option<usize>,
        origin: SearchOrigin,
    ) -> Result<Vec<SearchHit>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Err("Query is empty".into());
        }
        let settings = self.host.settings();
        let top_k = top_k.unwrap_or(settings.top_k).clamp(1, MAX_TOP_K);
        let mut kbs = self.list(agent_id)?;
        if kbs.is_empty() {
            return Err("This agent has no knowledge bases".into());
        }
        if let Some(sel) = kb.map(str::trim).filter(|s| !s.is_empty()) {
            kbs.retain(|k| k.id == sel || k.name.eq_ignore_ascii_case(sel));
            if kbs.is_empty() {
                return Err(format!("No knowledge base named \"{sel}\""));
            }
        }
        // One query embedding per distinct model.
        let mut query_vectors: HashMap<EmbeddingSpec, Vec<f32>> = HashMap::new();
        let mut hits = Vec::new();
        for info in kbs {
            if info.chunk_count == 0 {
                continue;
            }
            let started = std::time::Instant::now();
            let qv = match query_vectors.get(&info.embedding) {
                Some(v) => v.clone(),
                None => {
                    let embedder = self.embedders.embedder(&info.embedding)?;
                    let v = embed_checked(
                        embedder.as_ref(),
                        &[query.to_string()],
                        EmbedInput::Query,
                        info.embedding.dimensions,
                    )
                    .await?
                    .remove(0);
                    query_vectors.insert(info.embedding.clone(), v.clone());
                    v
                }
            };
            let store = self.store(agent_id, &info.id)?;
            let (q, weight) = (query.to_string(), settings.hybrid_weight);
            let found = blocking(move || {
                let found = hybrid_search(&store, &qv, &q, top_k, weight)?;
                let latency = started.elapsed().as_millis() as u64;
                if let Err(e) = store.log_search(now_ms(), latency, found.len(), origin.as_str()) {
                    tracing::warn!(error = %e, "Failed to log knowledge base search");
                }
                Ok(found)
            })
            .await?;
            hits.extend(found.into_iter().map(|(c, score)| SearchHit {
                kb_id: info.id.clone(),
                kb_name: info.name.clone(),
                source_id: c.source_id,
                title: c.title,
                uri: c.uri,
                heading_path: c.heading_path,
                text: c.text,
                score,
            }));
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(top_k);
        Ok(hits)
    }

    // ---- storage ----

    /// Every knowledgebase on disk; `known_agents` are the ids in `customAgents`.
    pub fn storage_overview(&self, known_agents: &HashSet<String>) -> StorageOverview {
        let mut entries = Vec::new();
        for agent in self.agent_ids() {
            for kb in self.kb_ids(&agent).unwrap_or_default() {
                let path = self.kb_path(&agent, &kb);
                let stats = self.stats(&agent, &kb).ok();
                let (name, sources, chunks, embedding) = match &stats {
                    Some(st) => (
                        st.info.name.clone(),
                        st.info.source_count,
                        st.info.chunk_count,
                        Some(st.info.embedding.clone()),
                    ),
                    None => (kb.clone(), 0, 0, None),
                };
                entries.push(StorageEntry {
                    orphan: !known_agents.contains(&agent),
                    agent_id: agent.clone(),
                    kb_id: kb,
                    name,
                    bytes: db_size(&path),
                    source_count: sources,
                    chunk_count: chunks,
                    embedding,
                    stats,
                });
            }
        }
        StorageOverview {
            root: self.root.to_string_lossy().into_owned(),
            total_bytes: entries.iter().map(|e| e.bytes).sum(),
            knowledge_bases: entries,
        }
    }

    // ---- usage, health and maintenance ----

    /// Usage over the last [`USAGE_DAYS`] days and health checks for one knowledgebase.
    pub fn stats(&self, agent_id: &str, kb_id: &str) -> Result<KbStats, String> {
        let info = self.info(agent_id, kb_id)?;
        let store = self.store(agent_id, kb_id)?;
        let usage = store.usage(now_ms(), USAGE_DAYS)?;
        let sources = self.list_sources(agent_id, kb_id)?;
        let (page, pages, free) = store.pages()?;
        let reclaimable = page * free;
        let wal_bytes = db_files(store.path())
            .get(1)
            .and_then(|p| std::fs::metadata(p).ok())
            .map_or(0, |m| m.len());

        let mut issues = Vec::new();
        let issue = |kind: &str, severity: Severity, count: u64| HealthIssue {
            kind: kind.into(),
            severity,
            count,
        };
        let errors = sources
            .iter()
            .filter(|s| s.status == SourceStatus::Error)
            .count();
        if errors > 0 {
            issues.push(issue("sourceErrors", Severity::Critical, errors as u64));
        }
        if info.embedding.provider == Provider::Local
            && !crate::local::is_installed(&self.models_dir())
        {
            issues.push(issue("localModelMissing", Severity::Critical, 0));
        }
        if info.embedding != self.host.settings().embedding.spec() && info.chunk_count > 0 {
            issues.push(issue("modelMismatch", Severity::Info, 0));
        }
        let stale_before = chrono::Utc::now() - chrono::Duration::days(STALE_WEBSITE_DAYS);
        let stale = sources
            .iter()
            .filter(|s| s.kind == SourceKind::Url)
            .filter(|s| {
                s.synced_at
                    .as_deref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                    .is_some_and(|t| t < stale_before)
            })
            .count();
        if stale > 0 {
            issues.push(issue("staleWebsites", Severity::Warning, stale as u64));
        }
        if reclaimable >= RECLAIM_FLAG_BYTES
            && free as f64 >= RECLAIM_FLAG_SHARE * pages.max(1) as f64
        {
            issues.push(issue("reclaimable", Severity::Info, reclaimable));
        }
        if sources.is_empty() {
            issues.push(issue("empty", Severity::Info, 0));
        }
        let website_sources = sources.iter().filter(|s| s.kind == SourceKind::Url).count();
        Ok(KbStats {
            file_sources: sources.len() - website_sources,
            website_sources,
            info,
            usage,
            health: KbHealth {
                reclaimable_bytes: reclaimable,
                wal_bytes,
                issues,
            },
            period_days: USAGE_DAYS,
        })
    }

    /// Compact and check a knowledgebase (waits for its running job).
    pub async fn optimize(&self, agent_id: &str, kb_id: &str) -> Result<OptimizeResult, String> {
        let store = self.store(agent_id, kb_id)?;
        let kb_lock = self.kb_lock(agent_id, kb_id);
        let _guard = kb_lock.lock().await;
        let started = std::time::Instant::now();
        let s = store.clone();
        let result = blocking(move || optimize_store(&s)).await?;
        let entry = crate::store::JobLogEntry {
            at: now_ms(),
            kind: "optimize".into(),
            source_id: None,
            duration_ms: started.elapsed().as_millis() as u64,
            documents: 0,
            chunks: 0,
            status: if result.integrity == "ok" {
                "done"
            } else {
                "error"
            }
            .into(),
            message: format!(
                "{} -> {} bytes, integrity {}",
                result.bytes_before, result.bytes_after, result.integrity
            ),
        };
        let s = store.clone();
        blocking(move || s.log_job(&entry)).await?;
        self.emit_changed(agent_id, kb_id);
        Ok(result)
    }

    /// Delete the knowledgebases of agents that no longer exist. Returns how many.
    pub async fn purge_orphans(&self, known_agents: &HashSet<String>) -> Result<usize, String> {
        let orphans: HashSet<String> = self
            .storage_overview(known_agents)
            .knowledge_bases
            .into_iter()
            .filter(|e| e.orphan)
            .map(|e| e.agent_id)
            .collect();
        let mut n = 0;
        for agent in orphans {
            n += self.kb_ids(&agent)?.len();
            self.delete_agent(&agent).await?;
        }
        Ok(n)
    }
}

fn optimize_store(store: &KbStore) -> Result<OptimizeResult, String> {
    let bytes_before = db_size(store.path());
    let integrity = store.optimize()?;
    Ok(OptimizeResult {
        bytes_before,
        bytes_after: db_size(store.path()),
        integrity,
    })
}

/// `content_hash` of a placeholder document for a file that could not be extracted.
pub const FAILED_HASH: &str = "";

/// Chunk, embed and store one document.
struct IngestCtx {
    store: Arc<KbStore>,
    meta: KbMeta,
    embedder: Arc<dyn Embedder>,
    rep: Reporter,
}

impl IngestCtx {
    /// Returns `false` when the document is unchanged since the last sync.
    #[allow(clippy::too_many_arguments)]
    async fn ingest(
        &self,
        source_id: &str,
        uri: &str,
        title: &str,
        text: &str,
        kind: TextKind,
        mtime: Option<i64>,
        doc_index: usize,
        doc_total: usize,
    ) -> Result<bool, String> {
        self.rep.check()?;
        let content_hash = hash(&format!("{title}\n{text}"));
        if self.store.document_hash(source_id, uri)?.as_deref() == Some(content_hash.as_str()) {
            if let Some(m) = mtime {
                self.store.set_document_mtime(source_id, uri, m)?;
            }
            return Ok(false);
        }
        let chunks = chunk_text(text, kind, self.meta.chunk_size, self.meta.chunk_overlap);
        let inputs: Vec<String> = chunks
            .iter()
            .map(|c| embed_text(title, &c.heading_path, &c.text))
            .collect();
        let dims = self.meta.embedding.dimensions;
        let mut vectors = Vec::with_capacity(inputs.len());
        for batch in inputs.chunks(batch_size(&self.meta.embedding)) {
            self.rep.check()?;
            let message = if doc_total > 1 {
                format!("{title} ({}/{doc_total})", doc_index + 1)
            } else {
                title.to_string()
            };
            self.rep
                .update(JobPhase::Embedding, vectors.len(), inputs.len(), message);
            vectors.extend(
                embed_checked(self.embedder.as_ref(), batch, EmbedInput::Document, dims).await?,
            );
        }
        self.rep.check()?;
        let new_chunks: Vec<NewChunk> = chunks
            .into_iter()
            .zip(vectors)
            .map(|(c, embedding)| NewChunk {
                heading_path: c.heading_path,
                text: c.text,
                embedding,
            })
            .collect();
        let (store, source_id, uri, title) = (
            self.store.clone(),
            source_id.to_string(),
            uri.to_string(),
            title.to_string(),
        );
        let bytes = text.len() as u64;
        blocking(move || {
            store.replace_document(&source_id, &uri, &title, &content_hash, bytes, &new_chunks)?;
            match mtime {
                Some(m) => store.set_document_mtime(&source_id, &uri, m),
                None => Ok(()),
            }
        })
        .await?;
        Ok(true)
    }

    /// Remember the modification time of a file that failed to extract, so it isn't retried
    /// until it changes. A document indexed from an earlier version keeps its passages; a
    /// new file gets an empty placeholder document (no passages, not counted).
    async fn mark_failed(&self, source_id: &str, uri: &str, mtime: i64) -> Result<(), String> {
        let (store, source_id, uri) = (self.store.clone(), source_id.to_string(), uri.to_string());
        blocking(move || {
            if store.document_hash(&source_id, &uri)?.is_none() {
                store.replace_document(&source_id, &uri, &uri, FAILED_HASH, 0, &[])?;
            }
            store.set_document_mtime(&source_id, &uri, mtime)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::tests::{page, route, serve};
    use crate::embed::fake::FakeProvider;

    pub struct TestHost {
        pub settings: Mutex<KnowledgeSettings>,
        pub events: Mutex<Vec<(String, Value)>>,
    }

    impl KnowledgeHost for TestHost {
        fn settings(&self) -> KnowledgeSettings {
            lock(&self.settings).clone()
        }
        fn http_client(&self) -> reqwest::Client {
            reqwest::Client::new()
        }
        fn emit(&self, event: &str, payload: Value) {
            lock(&self.events).push((event.into(), payload));
        }
    }

    fn service(dir: &Path) -> (Arc<KnowledgeService>, Arc<TestHost>) {
        let mut settings = KnowledgeSettings::default();
        settings.embedding.dimensions = 256;
        settings.chunk_size = 60;
        settings.chunk_overlap = 10;
        settings.crawler.delay_ms = 0;
        let host = Arc::new(TestHost {
            settings: Mutex::new(settings),
            events: Mutex::new(Vec::new()),
        });
        let svc = KnowledgeService::new(
            dir.join("knowledgebases"),
            host.clone(),
            Arc::new(FakeProvider),
            tokio::runtime::Handle::current(),
        );
        (svc, host)
    }

    async fn wait_idle(svc: &KnowledgeService) {
        for _ in 0..500 {
            if svc.jobs(None).is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("jobs did not finish: {:?}", svc.jobs(None));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn file_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, host) = service(dir.path());
        assert!(svc.create("../x", "n", "").is_err());
        assert!(svc.create("agent1", " ", "").is_err());
        let kb = svc.create("agent1", "Manuals", "product manuals").unwrap();
        assert_eq!(kb.embedding.dimensions, 256);
        assert_eq!(svc.list("agent1").unwrap().len(), 1);

        let f = dir.path().join("guide.md");
        std::fs::write(
            &f,
            "# Setup\nInstall the widget with the installer tool.\n\n# Usage\nPress the big red button to launch rockets.\n",
        )
        .unwrap();
        let path = f.to_string_lossy().into_owned();
        assert!(svc
            .add_files("agent1", &kb.id, &["relative.md".into()])
            .is_err());
        let added = svc
            .add_files("agent1", &kb.id, &[path.clone(), path.clone()])
            .unwrap();
        assert_eq!(added.len(), 1);
        wait_idle(&svc).await;

        let src = &svc.list_sources("agent1", &kb.id).unwrap()[0];
        assert_eq!(src.status, SourceStatus::Ready, "{src:?}");
        assert_eq!(src.chunk_count, 2);
        let hits = svc
            .search(
                "agent1",
                None,
                "red button rockets",
                Some(1),
                SearchOrigin::Test,
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("red button"), "{hits:?}");
        assert_eq!(hits[0].heading_path, "Usage");
        assert_eq!(hits[0].kb_name, "Manuals");
        assert!(svc
            .search(
                "agent1",
                Some("manuals"),
                "widget",
                None,
                SearchOrigin::Test
            )
            .await
            .is_ok());
        assert!(svc
            .search("agent1", Some("nope"), "widget", None, SearchOrigin::Test)
            .await
            .is_err());
        assert!(svc
            .search("agent2", None, "widget", None, SearchOrigin::Test)
            .await
            .is_err());

        // Unchanged re-sync is skipped; a missing file is an error on the source.
        svc.resync_source("agent1", &kb.id, &src.id).unwrap();
        wait_idle(&svc).await;
        let done = lock(&host.events)
            .iter()
            .rev()
            .find(|(e, p)| e == PROGRESS_EVENT && p["phase"] == "done")
            .cloned()
            .unwrap();
        assert_eq!(done.1["message"], "Unchanged");
        std::fs::remove_file(&f).unwrap();
        svc.resync_source("agent1", &kb.id, &src.id).unwrap();
        wait_idle(&svc).await;
        let src = &svc.list_sources("agent1", &kb.id).unwrap()[0];
        assert_eq!(src.status, SourceStatus::Error);
        assert!(src.error.as_deref().unwrap().contains("moved or deleted"));

        // Copy, then delete.
        let copied = svc.copy_agent("agent1", "agent2").await.unwrap();
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].chunk_count, 2);
        let known: HashSet<String> = ["agent1".to_string()].into();
        let overview = svc.storage_overview(&known);
        assert_eq!(overview.knowledge_bases.len(), 2);
        assert!(overview
            .knowledge_bases
            .iter()
            .any(|e| e.agent_id == "agent2" && e.orphan));
        assert_eq!(svc.purge_orphans(&known).await.unwrap(), 1);
        assert!(svc.list("agent2").unwrap().is_empty());

        svc.remove_source("agent1", &kb.id, &src.id).await.unwrap();
        assert_eq!(svc.list("agent1").unwrap()[0].chunk_count, 0);
        svc.delete("agent1", &kb.id).await.unwrap();
        assert!(svc.list("agent1").unwrap().is_empty());
        assert!(!dir.path().join("knowledgebases").join("agent1").exists());
        assert!(lock(&host.events).iter().any(|(e, _)| e == CHANGED_EVENT));
    }

    /// Write `body` to `path` and give it a modification time `secs` after the epoch, so
    /// tests don't depend on the filesystem's timestamp resolution.
    fn write_at(path: &Path, body: &str, secs: u64) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    fn last_done_message(host: &TestHost) -> String {
        lock(&host.events)
            .iter()
            .rev()
            .find(|(e, p)| e == PROGRESS_EVENT && p["phase"] == "done")
            .map(|(_, p)| p["message"].as_str().unwrap_or_default().to_string())
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn folder_source_follows_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, host) = service(dir.path());
        let kb = svc.create("agent1", "Docs", "").unwrap();
        let folder = dir.path().join("docs");
        write_at(
            &folder.join("a.md"),
            "# A\nAlpha widgets are blue.\n",
            1_000,
        );
        write_at(
            &folder.join("sub/b.md"),
            "# B\nBeta gadgets are green.\n",
            1_000,
        );
        write_at(&folder.join("broken.pdf"), "not a pdf", 1_000);
        write_at(&folder.join("image.png"), "x", 1_000);
        let uri = folder.to_string_lossy().into_owned();

        let yes = |_: usize| true;
        assert!(svc.add_folder("agent1", &kb.id, "relative", yes).is_err());
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(svc
            .add_folder("agent1", &kb.id, &empty.to_string_lossy(), yes)
            .unwrap_err()
            .contains("no supported files"));
        let src = svc
            .add_folder("agent1", &kb.id, &uri, yes)
            .unwrap()
            .unwrap();
        assert_eq!(src.kind, SourceKind::Folder);
        assert_eq!(src.title, "docs");
        assert!(svc.add_folder("agent1", &kb.id, &uri, yes).is_err());
        wait_idle(&svc).await;
        let s = svc.list_sources("agent1", &kb.id).unwrap().remove(0);
        assert_eq!(s.status, SourceStatus::Ready, "{s:?}");
        // The unreadable PDF is a placeholder: not counted, no passages.
        assert_eq!(s.document_count, 2);
        assert_eq!(last_done_message(&host), "3 files, 2 updated, 1 skipped");

        // Nothing changed on disk: the auto-sync pass queues nothing.
        assert_eq!(svc.auto_sync_pass(), 0);

        // Edit one file, add one, delete one.
        write_at(
            &folder.join("a.md"),
            "# A\nAlpha widgets are now red.\n",
            2_000,
        );
        write_at(&folder.join("c.txt"), "Gamma gizmos hum quietly.", 2_000);
        std::fs::remove_file(folder.join("sub/b.md")).unwrap();
        assert_eq!(svc.auto_sync_pass(), 1);
        wait_idle(&svc).await;
        assert_eq!(
            last_done_message(&host),
            "3 files, 2 updated, 1 removed, 1 skipped"
        );
        let s = svc.list_sources("agent1", &kb.id).unwrap().remove(0);
        assert_eq!(s.document_count, 2);
        let hits = svc
            .search("agent1", None, "gizmos hum", Some(1), SearchOrigin::Test)
            .await
            .unwrap();
        assert!(hits[0].text.contains("Gamma"), "{hits:?}");
        assert_eq!(svc.auto_sync_pass(), 0);

        // A manual re-sync skips unmodified files but retries the one that failed.
        svc.resync_source("agent1", &kb.id, &s.id).unwrap();
        wait_idle(&svc).await;
        assert_eq!(last_done_message(&host), "3 files, 0 updated, 1 skipped");

        // A deleted folder errors once and is then left alone.
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(svc.auto_sync_pass(), 1);
        wait_idle(&svc).await;
        let s = svc.list_sources("agent1", &kb.id).unwrap().remove(0);
        assert_eq!(s.status, SourceStatus::Error);
        assert_eq!(svc.auto_sync_pass(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn large_folders_ask_first() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _host) = service(dir.path());
        let kb = svc.create("agent1", "Docs", "").unwrap();
        let folder = dir.path().join("many");
        std::fs::create_dir_all(&folder).unwrap();
        for i in 0..LARGE_FOLDER_FILES {
            std::fs::write(folder.join(format!("{i}.txt")), "x").unwrap();
        }
        let uri = folder.to_string_lossy().into_owned();
        let mut asked = None;
        let declined = svc
            .add_folder("agent1", &kb.id, &uri, |n| {
                asked = Some(n);
                false
            })
            .unwrap();
        assert!(declined.is_none());
        assert_eq!(asked, Some(LARGE_FOLDER_FILES));
        assert!(svc.list_sources("agent1", &kb.id).unwrap().is_empty());
        // Small folders are added without asking.
        std::fs::remove_file(folder.join("0.txt")).unwrap();
        let added = svc
            .add_folder("agent1", &kb.id, &uri, |_| panic!("asked"))
            .unwrap();
        assert!(added.is_some());
        svc.cancel_all();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn file_source_skips_unmodified_files() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, host) = service(dir.path());
        let kb = svc.create("agent1", "Docs", "").unwrap();
        let f = dir.path().join("notes.md");
        write_at(&f, "# Notes\nOne fish.\n", 1_000);
        svc.add_files("agent1", &kb.id, &[f.to_string_lossy().into_owned()])
            .unwrap();
        wait_idle(&svc).await;
        assert_eq!(svc.auto_sync_pass(), 0);
        // Touched but not edited: re-read, unchanged, no new embedding.
        write_at(&f, "# Notes\nOne fish.\n", 2_000);
        assert_eq!(svc.auto_sync_pass(), 1);
        wait_idle(&svc).await;
        assert_eq!(last_done_message(&host), "Unchanged");
        assert_eq!(svc.auto_sync_pass(), 0);
        write_at(&f, "# Notes\nTwo fish.\n", 3_000);
        assert_eq!(svc.auto_sync_pass(), 1);
        wait_idle(&svc).await;
        assert_eq!(last_done_message(&host), "Indexed");
        let src = svc.list_sources("agent1", &kb.id).unwrap().remove(0);
        assert_eq!(src.status, SourceStatus::Ready);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn url_source_and_rebuild() {
        let srv = serve(|_| {
            vec![
                route(
                    "/docs/",
                    200,
                    "text/html",
                    page(
                        "Docs Home",
                        "Welcome to the documentation of the frobnicator.",
                        &["install"],
                    ),
                ),
                route(
                    "/docs/install",
                    200,
                    "text/html",
                    page(
                        "Install",
                        "Run the frobnicator installer with admin rights.",
                        &[],
                    ),
                ),
            ]
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let (svc, host) = service(dir.path());
        let kb = svc.create("a", "Web", "").unwrap();
        let src = svc
            .add_url(
                "a",
                &kb.id,
                &format!("{}/docs/", srv.base),
                UrlOptions {
                    max_depth: 1,
                    max_pages: 10,
                    path_prefix_only: true,
                },
            )
            .unwrap();
        assert!(svc
            .add_url(
                "a",
                &kb.id,
                &format!("{}/docs/", srv.base),
                UrlOptions::default()
            )
            .is_err());
        assert!(svc
            .add_url("a", &kb.id, "file:///etc/passwd", UrlOptions::default())
            .is_err());
        wait_idle(&svc).await;
        let s = svc.list_sources("a", &kb.id).unwrap().remove(0);
        assert_eq!(s.status, SourceStatus::Ready, "{s:?}");
        assert_eq!(s.document_count, 2);
        assert_eq!(s.title, "Docs Home");
        assert_eq!(s.id, src.id);
        let hits = svc
            .search(
                "a",
                None,
                "installer admin rights",
                Some(1),
                SearchOrigin::Test,
            )
            .await
            .unwrap();
        assert_eq!(hits[0].title, "Install");

        // Rebuild with another model size.
        lock(&host.settings).embedding.bedrock_model_id = "cohere.embed-v4:0".into();
        lock(&host.settings).embedding.dimensions = 512;
        svc.rebuild("a", &kb.id).unwrap();
        wait_idle(&svc).await;
        let info = svc.list("a").unwrap().remove(0);
        assert_eq!(info.embedding.dimensions, 512);
        assert_eq!(info.embedding.model_id, "cohere.embed-v4:0");
        let hits = svc
            .search(
                "a",
                None,
                "installer admin rights",
                Some(1),
                SearchOrigin::Test,
            )
            .await
            .unwrap();
        assert_eq!(hits[0].title, "Install");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_marks_source_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path());
        let kb = svc.create("a", "K", "").unwrap();
        // Hold the knowledge base lock so the job stays queued, then cancel it.
        let kb_lock = svc.kb_lock("a", &kb.id);
        let guard = kb_lock.lock().await;
        let f = dir.path().join("n.txt");
        std::fs::write(&f, "hello").unwrap();
        svc.add_files("a", &kb.id, &[f.to_string_lossy().into_owned()])
            .unwrap();
        let jobs = svc.jobs(Some("a"));
        assert_eq!(jobs.len(), 1);
        assert!(svc.cancel_job(&jobs[0].job_id));
        wait_idle(&svc).await;
        drop(guard);
        let s = svc.list_sources("a", &kb.id).unwrap().remove(0);
        assert_eq!(s.status, SourceStatus::Cancelled);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn usage_health_and_optimize() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path());
        let kb = svc.create("a", "K", "").unwrap();
        let st = svc.stats("a", &kb.id).unwrap();
        assert_eq!(st.usage.searches_in_period, 0);
        assert_eq!(st.usage.daily_searches.len(), USAGE_DAYS);
        assert!(st.health.issues.iter().any(|i| i.kind == "empty"));

        let f = dir.path().join("big.txt");
        std::fs::write(&f, "alpha beta gamma delta. ".repeat(4000)).unwrap();
        let f2 = dir.path().join("gone.txt");
        std::fs::write(&f2, "temporary").unwrap();
        svc.add_files(
            "a",
            &kb.id,
            &[
                f.to_string_lossy().into_owned(),
                f2.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        wait_idle(&svc).await;
        svc.search("a", None, "gamma delta", None, SearchOrigin::Agent)
            .await
            .unwrap();
        svc.search("a", None, "alpha", None, SearchOrigin::Agent)
            .await
            .unwrap();
        svc.search("a", None, "beta", None, SearchOrigin::Test)
            .await
            .unwrap();

        let st = svc.stats("a", &kb.id).unwrap();
        assert_eq!(
            st.usage.searches_in_period, 2,
            "test searches are counted apart"
        );
        assert_eq!(st.usage.test_searches_in_period, 1);
        assert_eq!(*st.usage.daily_searches.last().unwrap(), 2);
        assert!(st.usage.latency_p50_ms.is_some());
        assert!(st.usage.last_search_at.is_some());
        assert_eq!(st.usage.last_job.as_ref().unwrap().kind, "source");
        assert_eq!(st.file_sources, 2);

        // A broken source is a critical issue.
        std::fs::remove_file(&f2).unwrap();
        let gone = svc
            .list_sources("a", &kb.id)
            .unwrap()
            .into_iter()
            .find(|s| s.title == "gone.txt")
            .unwrap();
        svc.resync_source("a", &kb.id, &gone.id).unwrap();
        wait_idle(&svc).await;
        let st = svc.stats("a", &kb.id).unwrap();
        let err = st
            .health
            .issues
            .iter()
            .find(|i| i.kind == "sourceErrors")
            .unwrap();
        assert_eq!((err.severity, err.count), (Severity::Critical, 1));

        // Removing the big source leaves free pages; optimize gives them back.
        let big = svc
            .list_sources("a", &kb.id)
            .unwrap()
            .into_iter()
            .find(|s| s.title == "big.txt")
            .unwrap();
        svc.remove_source("a", &kb.id, &big.id).await.unwrap();
        let r = svc.optimize("a", &kb.id).await.unwrap();
        assert_eq!(r.integrity, "ok");
        assert!(r.bytes_after <= r.bytes_before, "{r:?}");
        let st = svc.stats("a", &kb.id).unwrap();
        assert!(st.usage.last_optimized_at.is_some());
        assert!(!st.health.issues.iter().any(|i| i.kind == "reclaimable"));
        assert_eq!(st.usage.searches_total, 2, "the log survives optimize");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn interrupted_sources_show_as_errors() {
        let dir = tempfile::tempdir().unwrap();
        let (svc, _) = service(dir.path());
        let kb = svc.create("a", "K", "").unwrap();
        svc.store("a", &kb.id)
            .unwrap()
            .add_source("s", SourceKind::File, "/x.txt", "x.txt", None, "t")
            .unwrap();
        let s = svc.list_sources("a", &kb.id).unwrap().remove(0);
        assert_eq!(s.status, SourceStatus::Error);
        assert!(s.error.unwrap().contains("Interrupted"));
    }
}
