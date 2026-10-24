//! Settings (the `knowledgebase` store key) and the JSON shapes the commands return.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where a knowledgebase's vectors come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    #[default]
    Bedrock,
    Local,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Bedrock => "bedrock",
            Provider::Local => "local",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "bedrock" => Some(Provider::Bedrock),
            "local" => Some(Provider::Local),
            _ => None,
        }
    }
}

/// The embedding model a knowledgebase was built with. Fixed at creation: vectors from
/// different models (or dimensions) can't be compared.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpec {
    pub provider: Provider,
    pub model_id: String,
    pub dimensions: usize,
}

/// The local model offered in Settings (`intfloat/multilingual-e5-small`, 384 dimensions).
pub const LOCAL_MODEL_ID: &str = "multilingual-e5-small";
pub const LOCAL_MODEL_DIMENSIONS: usize = 384;

pub const TITAN_V2: &str = "amazon.titan-embed-text-v2:0";

/// Bedrock embedding models offered in Settings: `(model id, allowed dimensions, default)`.
pub const BEDROCK_MODELS: &[(&str, &[usize], usize)] = &[
    (TITAN_V2, &[256, 512, 1024], 1024),
    ("cohere.embed-multilingual-v3", &[1024], 1024),
    ("cohere.embed-english-v3", &[1024], 1024),
    ("cohere.embed-v4:0", &[256, 512, 1024, 1536], 1536),
];

/// `knowledgebase.embedding`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbeddingSettings {
    pub provider: Provider,
    pub bedrock_model_id: String,
    pub dimensions: usize,
    pub local_model: String,
}

impl Default for EmbeddingSettings {
    fn default() -> Self {
        Self {
            provider: Provider::Bedrock,
            bedrock_model_id: TITAN_V2.into(),
            dimensions: 1024,
            local_model: LOCAL_MODEL_ID.into(),
        }
    }
}

impl EmbeddingSettings {
    /// The spec a knowledgebase created now gets. Unknown Bedrock models keep the configured
    /// dimensions; known ones fall back to their default when the configured value isn't
    /// supported.
    pub fn spec(&self) -> EmbeddingSpec {
        match self.provider {
            Provider::Local => EmbeddingSpec {
                provider: Provider::Local,
                model_id: LOCAL_MODEL_ID.into(),
                dimensions: LOCAL_MODEL_DIMENSIONS,
            },
            Provider::Bedrock => {
                let model_id = if self.bedrock_model_id.trim().is_empty() {
                    TITAN_V2.to_string()
                } else {
                    self.bedrock_model_id.trim().to_string()
                };
                let dimensions = match BEDROCK_MODELS
                    .iter()
                    .find(|(id, _, _)| bedrock_model_base(&model_id) == *id)
                {
                    Some((_, allowed, default)) if !allowed.contains(&self.dimensions) => *default,
                    _ if self.dimensions == 0 => 1024,
                    _ => self.dimensions,
                };
                EmbeddingSpec {
                    provider: Provider::Bedrock,
                    model_id,
                    dimensions,
                }
            }
        }
    }
}

/// A model id without a cross-region inference profile prefix (`us.cohere.embed-v4:0` ->
/// `cohere.embed-v4:0`).
pub fn bedrock_model_base(model_id: &str) -> &str {
    for prefix in [
        "us.", "eu.", "apac.", "jp.", "au.", "ca.", "us-gov.", "global.",
    ] {
        if let Some(rest) = model_id.strip_prefix(prefix) {
            return rest;
        }
    }
    model_id
}

/// `knowledgebase.crawler`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CrawlerSettings {
    pub max_depth: u32,
    pub max_pages: u32,
    pub delay_ms: u64,
    pub respect_robots: bool,
    pub use_sitemap: bool,
}

impl Default for CrawlerSettings {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_pages: 50,
            delay_ms: 500,
            respect_robots: true,
            use_sitemap: true,
        }
    }
}

/// The `knowledgebase` store key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KnowledgeSettings {
    pub embedding: EmbeddingSettings,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,
    pub hybrid_weight: f64,
    pub crawler: CrawlerSettings,
}

impl Default for KnowledgeSettings {
    fn default() -> Self {
        Self {
            embedding: EmbeddingSettings::default(),
            chunk_size: 400,
            chunk_overlap: 60,
            top_k: 6,
            hybrid_weight: 0.6,
            crawler: CrawlerSettings::default(),
        }
    }
}

/// Hard limits, whatever the settings say.
pub const MAX_CHUNK_SIZE: usize = 2000;
pub const MIN_CHUNK_SIZE: usize = 50;
pub const MAX_TOP_K: usize = 50;
pub const MAX_CRAWL_PAGES: u32 = 1000;
pub const MAX_CRAWL_DEPTH: u32 = 10;

impl KnowledgeSettings {
    /// From `store.knowledgebase`; a malformed key falls back to the defaults.
    pub fn from_store(store: &Value) -> Self {
        store
            .get("knowledgebase")
            .cloned()
            .and_then(|v| serde_json::from_value::<Self>(v).ok())
            .unwrap_or_default()
            .clamped()
    }

    pub fn clamped(mut self) -> Self {
        self.chunk_size = self.chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        self.chunk_overlap = self.chunk_overlap.min(self.chunk_size / 2);
        self.top_k = self.top_k.clamp(1, MAX_TOP_K);
        if !self.hybrid_weight.is_finite() {
            self.hybrid_weight = 0.6;
        }
        self.hybrid_weight = self.hybrid_weight.clamp(0.0, 1.0);
        self.crawler.max_pages = self.crawler.max_pages.clamp(1, MAX_CRAWL_PAGES);
        self.crawler.max_depth = self.crawler.max_depth.min(MAX_CRAWL_DEPTH);
        self.crawler.delay_ms = self.crawler.delay_ms.min(60_000);
        self
    }
}

/// One knowledgebase, as listed to the renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbInfo {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub embedding: EmbeddingSpec,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub created_at: String,
    pub source_count: usize,
    pub document_count: usize,
    pub chunk_count: usize,
    /// Size of the database file(s) on disk.
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    File,
    Url,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::File => "file",
            SourceKind::Url => "url",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceStatus {
    Queued,
    Processing,
    Ready,
    Error,
    Cancelled,
}

impl SourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceStatus::Queued => "queued",
            SourceStatus::Processing => "processing",
            SourceStatus::Ready => "ready",
            SourceStatus::Error => "error",
            SourceStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "queued" => SourceStatus::Queued,
            "processing" => SourceStatus::Processing,
            "ready" => SourceStatus::Ready,
            "cancelled" => SourceStatus::Cancelled,
            _ => SourceStatus::Error,
        }
    }
}

/// Crawl options stored with a URL source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UrlOptions {
    pub max_depth: u32,
    pub max_pages: u32,
    /// Only follow links under the start URL's directory (otherwise anywhere on the host).
    pub path_prefix_only: bool,
}

impl Default for UrlOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_pages: 50,
            path_prefix_only: true,
        }
    }
}

impl UrlOptions {
    pub fn clamped(mut self) -> Self {
        self.max_pages = self.max_pages.clamp(1, MAX_CRAWL_PAGES);
        self.max_depth = self.max_depth.min(MAX_CRAWL_DEPTH);
        self
    }
}

/// A file or URL added to a knowledgebase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub kind: SourceKind,
    /// File path or start URL.
    pub uri: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<UrlOptions>,
    pub status: SourceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub document_count: usize,
    pub chunk_count: usize,
    /// Extracted text size.
    pub bytes: u64,
    pub added_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kb_id: String,
    pub kb_name: String,
    pub source_id: String,
    pub title: String,
    /// File path or page URL.
    pub uri: String,
    pub heading_path: String,
    pub text: String,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobPhase {
    Queued,
    Fetching,
    Extracting,
    Embedding,
    Indexing,
    Done,
    Error,
    Cancelled,
}

impl JobPhase {
    pub fn is_finished(self) -> bool {
        matches!(self, JobPhase::Done | JobPhase::Error | JobPhase::Cancelled)
    }
}

/// `knowledge:progress` payload, and the rows of `knowledge_jobs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub job_id: String,
    pub agent_id: String,
    pub kb_id: String,
    /// `None` for a rebuild of the whole knowledgebase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub phase: JobPhase,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

/// One knowledgebase in the storage overview.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageEntry {
    pub agent_id: String,
    pub kb_id: String,
    pub name: String,
    pub bytes: u64,
    pub source_count: usize,
    pub chunk_count: usize,
    pub embedding: Option<EmbeddingSpec>,
    /// The owning agent no longer exists in `customAgents`.
    pub orphan: bool,
    /// Usage and health; `None` if the file can't be read.
    pub stats: Option<KbStats>,
}

/// Who ran a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOrigin {
    /// The model, through the MCP tool.
    Agent,
    /// The search box in the agent editor.
    Test,
}

impl SearchOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            SearchOrigin::Agent => "agent",
            SearchOrigin::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Something about a knowledgebase that needs attention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    /// `sourceErrors` / `staleWebsites` / `modelMismatch` / `localModelMissing` /
    /// `reclaimable` / `integrity` / `empty`.
    pub kind: String,
    pub severity: Severity,
    /// Affected sources, or bytes for `reclaimable`.
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbHealth {
    /// Free pages in the file that an optimize would give back.
    pub reclaimable_bytes: u64,
    /// Size of the write-ahead log.
    pub wal_bytes: u64,
    pub issues: Vec<HealthIssue>,
}

/// Everything the dashboards show for one knowledgebase.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbStats {
    pub info: KbInfo,
    pub usage: crate::store::Usage,
    pub health: KbHealth,
    /// Sources by kind.
    pub file_sources: usize,
    pub website_sources: usize,
    /// Days of [`crate::store::Usage::daily_searches`].
    pub period_days: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeResult {
    pub bytes_before: u64,
    pub bytes_after: u64,
    /// `"ok"` when the integrity check passed.
    pub integrity: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub root: String,
    pub total_bytes: u64,
    pub knowledge_bases: Vec<StorageEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_default_and_clamp() {
        let s = KnowledgeSettings::from_store(&json!({}));
        assert_eq!(s, KnowledgeSettings::default());
        let s = KnowledgeSettings::from_store(&json!({
            "knowledgebase": { "chunkSize": 10, "chunkOverlap": 999, "topK": 0, "hybridWeight": 3.0,
                               "crawler": { "maxPages": 0 } }
        }));
        assert_eq!(s.chunk_size, MIN_CHUNK_SIZE);
        assert_eq!(s.chunk_overlap, MIN_CHUNK_SIZE / 2);
        assert_eq!(s.top_k, 1);
        assert_eq!(s.hybrid_weight, 1.0);
        assert_eq!(s.crawler.max_pages, 1);
        assert_eq!(s.crawler.max_depth, 2);
        // Malformed key: defaults.
        let s = KnowledgeSettings::from_store(&json!({ "knowledgebase": "nope" }));
        assert_eq!(s, KnowledgeSettings::default());
    }

    #[test]
    fn embedding_spec_from_settings() {
        let mut e = EmbeddingSettings::default();
        assert_eq!(e.spec().model_id, TITAN_V2);
        assert_eq!(e.spec().dimensions, 1024);
        e.dimensions = 300;
        assert_eq!(
            e.spec().dimensions,
            1024,
            "unsupported Titan dims fall back"
        );
        e.bedrock_model_id = "us.cohere.embed-v4:0".into();
        e.dimensions = 512;
        assert_eq!(e.spec().dimensions, 512);
        e.bedrock_model_id = "cohere.embed-multilingual-v3".into();
        assert_eq!(e.spec().dimensions, 1024);
        e.provider = Provider::Local;
        assert_eq!(
            e.spec(),
            EmbeddingSpec {
                provider: Provider::Local,
                model_id: LOCAL_MODEL_ID.into(),
                dimensions: 384
            }
        );
    }
}
