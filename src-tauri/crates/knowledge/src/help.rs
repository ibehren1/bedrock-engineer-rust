//! The user guide's knowledge base, behind the Help page's search and the Help chat.
//!
//! It is built ahead of time from `docs/USER_GUIDE.md` with the on-device model (the
//! `build-help-kb` binary) and shipped with the app as `docs/help-kb.db`, so it needs no AWS and
//! no indexing on the user's machine. At run time the shipped file is copied into
//! `<knowledgebases>/helpAgent/user-guide.db` (SQLite needs a writable file) whenever its guide
//! hash changes, i.e. on first use and after an update.
//!
//! Searching it is hybrid (vector + keyword) when the on-device model is installed, since
//! queries must be embedded with the model the guide was; otherwise keyword-only (BM25 over the
//! FTS5 index). Both work offline.

use crate::chunk::{chunk_text, TextKind};
use crate::embed::{EmbedInput, Embedder};
use crate::search::hybrid_search;
use crate::store::{embed_text, KbMeta, KbStore, NewChunk};
use crate::types::{EmbeddingSpec, Provider, LOCAL_MODEL_DIMENSIONS, LOCAL_MODEL_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Owner folder of the guide's knowledge base (the Help agent's id).
pub const HELP_OWNER: &str = "helpAgent";
/// The guide knowledge base's id, under [`HELP_OWNER`].
pub const HELP_KB_ID: &str = "user-guide";
/// File name of the shipped database (next to `USER_GUIDE.md` in `docs/`).
pub const SHIPPED_FILE: &str = "help-kb.db";

/// `meta` keys.
const GUIDE_HASH: &str = "x:guide_sha256";
const BUILT_WITH: &str = "x:built_with";

/// Chunking for the guide: its sections are short, so smaller chunks keep excerpts focused.
const CHUNK_SIZE: usize = 300;
const CHUNK_OVERLAP: usize = 40;
const SOURCE_ID: &str = "user-guide";
const TITLE: &str = "Bedrock Engineer user guide";

pub fn guide_hash(guide: &str) -> String {
    format!("{:x}", Sha256::digest(guide.as_bytes()))
}

pub fn embedding_spec() -> EmbeddingSpec {
    EmbeddingSpec {
        provider: Provider::Local,
        model_id: LOCAL_MODEL_ID.into(),
        dimensions: LOCAL_MODEL_DIMENSIONS,
    }
}

/// The guide hash a built database was made from (`None` if it can't be read).
pub fn built_hash(db: &Path) -> Option<String> {
    KbStore::open(db).ok()?.extra(GUIDE_HASH).ok().flatten()
}

/// Build the guide's knowledge base at `out` (replacing it), embedding with `embedder`
/// (the on-device model). The result is a single self-contained SQLite file.
pub async fn build(guide: &str, out: &Path, embedder: &dyn Embedder) -> Result<usize, String> {
    let chunks = chunk_text(guide, TextKind::Markdown, CHUNK_SIZE, CHUNK_OVERLAP);
    let inputs: Vec<String> = chunks
        .iter()
        .map(|c| embed_text(TITLE, &c.heading_path, &c.text))
        .collect();
    let vectors = embedder.embed(&inputs, EmbedInput::Document).await?;
    if vectors.len() != chunks.len() {
        return Err(format!(
            "Expected {} embeddings, got {}",
            chunks.len(),
            vectors.len()
        ));
    }
    let tmp = tempdir_for(out)?;
    let work = tmp.join("build.db");
    let store = KbStore::create(
        &work,
        &KbMeta {
            name: TITLE.into(),
            description: "Built from docs/USER_GUIDE.md".into(),
            embedding: embedding_spec(),
            chunk_size: CHUNK_SIZE,
            chunk_overlap: CHUNK_OVERLAP,
            created_at: String::new(),
        },
    )?;
    store.add_source(
        SOURCE_ID,
        crate::types::SourceKind::File,
        "docs/USER_GUIDE.md",
        TITLE,
        None,
        "",
    )?;
    let new_chunks: Vec<NewChunk> = chunks
        .into_iter()
        .zip(vectors)
        .map(|(c, embedding)| NewChunk {
            heading_path: c.heading_path,
            text: c.text,
            embedding,
        })
        .collect();
    let count = new_chunks.len();
    store.replace_document(
        SOURCE_ID,
        "docs/USER_GUIDE.md",
        TITLE,
        &guide_hash(guide),
        guide.len() as u64,
        &new_chunks,
    )?;
    store.set_extra(GUIDE_HASH, &guide_hash(guide))?;
    store.set_extra(
        BUILT_WITH,
        &format!("{LOCAL_MODEL_ID}@{}", crate::local::REVISION),
    )?;
    if out.exists() {
        std::fs::remove_file(out).map_err(|e| e.to_string())?;
    }
    // VACUUM INTO writes one compact file without the WAL.
    store.copy_to(out)?;
    drop(store);
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(count)
}

fn tempdir_for(out: &Path) -> Result<PathBuf, String> {
    let dir = out
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".help-kb-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpHit {
    /// `"26. Accounting: … › 26.4 A monthly budget"`.
    pub heading_path: String,
    pub text: String,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Hybrid,
    Keyword,
}

/// The installed guide knowledge base.
pub struct HelpKb {
    shipped: PathBuf,
    installed: PathBuf,
    store: Mutex<Option<Arc<KbStore>>>,
}

impl HelpKb {
    /// `shipped`: the bundled `help-kb.db`; `kb_root`: `<userData>/knowledgebases`.
    pub fn new(shipped: PathBuf, kb_root: &Path) -> Self {
        Self {
            shipped,
            installed: kb_root.join(HELP_OWNER).join(format!("{HELP_KB_ID}.db")),
            store: Mutex::new(None),
        }
    }

    pub fn installed_path(&self) -> &Path {
        &self.installed
    }

    /// The open store, copying the shipped file into place first when it's missing or from a
    /// different guide.
    fn store(&self) -> Result<Arc<KbStore>, String> {
        let mut slot = self.store.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(s) = slot.as_ref() {
            return Ok(s.clone());
        }
        let shipped_hash = built_hash(&self.shipped)
            .ok_or_else(|| format!("The help index is missing ({})", self.shipped.display()))?;
        if built_hash(&self.installed).as_deref() != Some(shipped_hash.as_str()) {
            install(&self.shipped, &self.installed)?;
        }
        let store = Arc::new(KbStore::open(&self.installed)?);
        *slot = Some(store.clone());
        Ok(store)
    }

    /// The guide hash of the installed index (installing it if needed).
    pub fn guide_hash(&self) -> Result<Option<String>, String> {
        self.store()?.extra(GUIDE_HASH)
    }

    /// Best `top_k` excerpts for `query`. With an embedder (the on-device model) the search is
    /// hybrid, otherwise keyword-only.
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
        embedder: Option<Arc<dyn Embedder>>,
    ) -> Result<(SearchMode, Vec<HelpHit>), String> {
        let store = self.store()?;
        let query_vector = match embedder {
            Some(e) => e
                .embed(&[query.to_string()], EmbedInput::Query)
                .await
                .ok()
                .and_then(|mut v| v.pop()),
            None => None,
        };
        let q = query.to_string();
        tokio::task::spawn_blocking(move || {
            let hits = match &query_vector {
                Some(v) => (
                    SearchMode::Hybrid,
                    hybrid_search(&store, v, &q, top_k, 0.6)?
                        .into_iter()
                        .map(|(c, s)| hit(c.heading_path, c.text, s))
                        .collect(),
                ),
                None => {
                    let ids = store.search_keyword(&q, top_k)?;
                    let n = ids.len();
                    let by_id = store.chunks_by_id(&ids)?;
                    (
                        SearchMode::Keyword,
                        by_id
                            .into_iter()
                            .enumerate()
                            .map(|(i, c)| hit(c.heading_path, c.text, (n - i) as f64 / n as f64))
                            .collect(),
                    )
                }
            };
            Ok(hits)
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

fn hit(heading_path: String, text: String, score: f64) -> HelpHit {
    HelpHit {
        heading_path,
        text,
        score,
    }
}

/// Copy the shipped database into place (atomically: write beside, then rename).
fn install(shipped: &Path, dest: &Path) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    for f in crate::store::db_files(dest) {
        let _ = std::fs::remove_file(f);
    }
    let tmp = dest.with_extension("db.tmp");
    std::fs::copy(shipped, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    tracing::info!(dest = %dest.display(), "Installed the user guide's knowledge base");
    Ok(())
}

#[cfg(test)]
mod tests;
