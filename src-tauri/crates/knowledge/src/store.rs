//! One knowledgebase = one SQLite file: metadata, sources, documents, chunks, an FTS5 index
//! over the chunks and a sqlite-vec table with their embeddings.
//!
//! The `meta` table is the source of truth for the knowledgebase's name and the embedding
//! model / chunking it was built with. The vector table's name is in `meta.vec_table` so a
//! rebuild can fill a new table and switch over in one transaction.
//!
//! Methods are synchronous; callers run them on a blocking thread.

use crate::types::{EmbeddingSpec, Provider, SourceInfo, SourceKind, SourceStatus, UrlOptions};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once};

/// 1: the knowledgebase itself. 2: adds `search_log` and `job_log` (usage and maintenance).
const SCHEMA_VERSION: i64 = 2;

/// Search log rows older than this are pruned.
const SEARCH_LOG_DAYS: i64 = 90;
/// Job log rows kept.
const JOB_LOG_KEEP: i64 = 200;

const DAY_MS: i64 = 86_400_000;

/// Register sqlite-vec for every connection opened afterwards.
fn register_vec() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        // SAFETY: `sqlite3_vec_init` has the extension entry point signature sqlite expects;
        // sqlite-vec's docs register it exactly this way.
        type Init = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::os::raw::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::os::raw::c_int;
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<*const (), Init>(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

pub type Result<T> = std::result::Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Settings fixed at creation.
#[derive(Debug, Clone, PartialEq)]
pub struct KbMeta {
    pub name: String,
    pub description: String,
    pub embedding: EmbeddingSpec,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub created_at: String,
}

/// A chunk ready to be written.
pub struct NewChunk {
    pub heading_path: String,
    pub text: String,
    pub embedding: Vec<f32>,
}

/// A stored chunk with its document's title and URI.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredChunk {
    pub id: i64,
    pub source_id: String,
    pub title: String,
    pub uri: String,
    pub heading_path: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub sources: usize,
    pub documents: usize,
    pub chunks: usize,
}

pub struct KbStore {
    path: PathBuf,
    conn: Mutex<Connection>,
}

/// The text that gets embedded for a chunk: document title and heading path give the model
/// context the chunk text alone may lack.
pub fn embed_text(title: &str, heading_path: &str, text: &str) -> String {
    let mut head = String::new();
    if !title.is_empty() {
        head.push_str(title);
    }
    if !heading_path.is_empty() {
        if !head.is_empty() {
            head.push_str(crate::chunk::HEADING_SEP);
        }
        head.push_str(heading_path);
    }
    if head.is_empty() {
        text.to_string()
    } else {
        format!("{head}\n{text}")
    }
}

fn f32_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn vec_table_sql(table: &str, dims: usize) -> String {
    format!(
        "CREATE VIRTUAL TABLE {table} USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{dims}] distance_metric=cosine)"
    )
}

/// Quote each term for FTS5 (`"a" OR "b"`). Trigram matching needs 3+ characters; shorter
/// terms are returned separately for a `LIKE` fallback.
fn fts_query(query: &str) -> (Option<String>, Vec<String>) {
    let mut long = Vec::new();
    let mut short = Vec::new();
    for term in query.split_whitespace() {
        let term: String = term
            .chars()
            .filter(|c| !matches!(c, '"' | '*' | '^' | '(' | ')' | ':' | '{' | '}'))
            .collect();
        if term.is_empty() {
            continue;
        }
        if term.chars().count() >= 3 {
            long.push(format!("\"{term}\""));
        } else {
            short.push(term);
        }
    }
    let q = (!long.is_empty()).then(|| long.join(" OR "));
    (q, short)
}

impl KbStore {
    /// Create a new knowledgebase file. Fails if it exists.
    pub fn create(path: &Path, meta: &KbMeta) -> Result<KbStore> {
        if path.exists() {
            return Err(format!("{} already exists", path.display()));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let store = Self::open_conn(path)?;
        {
            let mut c = store.lock();
            let tx = c.transaction().map_err(err)?;
            tx.execute_batch(
                "CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE sources(
                    id TEXT PRIMARY KEY, kind TEXT NOT NULL, uri TEXT NOT NULL, title TEXT NOT NULL,
                    options_json TEXT, status TEXT NOT NULL, error TEXT,
                    doc_count INTEGER NOT NULL DEFAULT 0, chunk_count INTEGER NOT NULL DEFAULT 0,
                    bytes INTEGER NOT NULL DEFAULT 0, added_at TEXT NOT NULL, synced_at TEXT);
                 CREATE TABLE documents(
                    id INTEGER PRIMARY KEY, source_id TEXT NOT NULL, uri TEXT NOT NULL,
                    title TEXT NOT NULL, content_hash TEXT NOT NULL, bytes INTEGER NOT NULL,
                    UNIQUE(source_id, uri));
                 CREATE TABLE chunks(
                    id INTEGER PRIMARY KEY, document_id INTEGER NOT NULL, source_id TEXT NOT NULL,
                    ord INTEGER NOT NULL, heading_path TEXT NOT NULL, text TEXT NOT NULL);
                 CREATE INDEX chunks_document ON chunks(document_id);
                 CREATE INDEX chunks_source ON chunks(source_id);
                 CREATE VIRTUAL TABLE chunks_fts USING fts5(text, heading_path, tokenize='trigram');",
            )
            .map_err(err)?;
            tx.execute_batch(&vec_table_sql("vec_chunks", meta.embedding.dimensions))
                .map_err(err)?;
            let kv = [
                ("schema_version", SCHEMA_VERSION.to_string()),
                ("name", meta.name.clone()),
                ("description", meta.description.clone()),
                ("provider", meta.embedding.provider.as_str().to_string()),
                ("model_id", meta.embedding.model_id.clone()),
                ("dims", meta.embedding.dimensions.to_string()),
                ("chunk_size", meta.chunk_size.to_string()),
                ("chunk_overlap", meta.chunk_overlap.to_string()),
                ("created_at", meta.created_at.clone()),
                ("vec_table", "vec_chunks".to_string()),
            ];
            for (k, v) in kv {
                tx.execute(
                    "INSERT INTO meta(key, value) VALUES (?1, ?2)",
                    params![k, v],
                )
                .map_err(err)?;
            }
            tx.commit().map_err(err)?;
        }
        store.migrate()?;
        Ok(store)
    }

    /// Bring an older file up to [`SCHEMA_VERSION`]. Every step is idempotent.
    fn migrate(&self) -> Result<()> {
        let c = self.lock();
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS search_log(
                id INTEGER PRIMARY KEY, at INTEGER NOT NULL, latency_ms INTEGER NOT NULL,
                hits INTEGER NOT NULL, origin TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS search_log_at ON search_log(at);
             CREATE TABLE IF NOT EXISTS job_log(
                id INTEGER PRIMARY KEY, at INTEGER NOT NULL, kind TEXT NOT NULL, source_id TEXT,
                duration_ms INTEGER NOT NULL, documents INTEGER NOT NULL, chunks INTEGER NOT NULL,
                status TEXT NOT NULL, message TEXT NOT NULL);",
        )
        .map_err(err)?;
        c.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SCHEMA_VERSION.to_string()],
        )
        .map_err(err)?;
        Ok(())
    }

    /// Open an existing knowledgebase file.
    pub fn open(path: &Path) -> Result<KbStore> {
        if !path.is_file() {
            return Err(format!("Knowledge base not found: {}", path.display()));
        }
        let store = Self::open_conn(path)?;
        let version: Option<String> = store
            .lock()
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        match version.and_then(|v| v.parse::<i64>().ok()) {
            Some(v) if v <= SCHEMA_VERSION => {
                if v < SCHEMA_VERSION {
                    store.migrate()?;
                }
                Ok(store)
            }
            Some(v) => Err(format!(
                "Knowledge base was created by a newer version of the app (schema {v})"
            )),
            None => Err(format!("Not a knowledge base: {}", path.display())),
        }
    }

    fn open_conn(path: &Path) -> Result<KbStore> {
        register_vec();
        let conn = Connection::open(path).map_err(err)?;
        conn.busy_timeout(std::time::Duration::from_secs(10))
            .map_err(err)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=OFF;",
        )
        .map_err(err)?;
        Ok(KbStore {
            path: path.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn meta_value(c: &Connection, key: &str) -> Result<Option<String>> {
        c.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(err)
    }

    fn vec_table(c: &Connection) -> Result<String> {
        let t = Self::meta_value(c, "vec_table")?.unwrap_or_else(|| "vec_chunks".into());
        // Only names this module writes.
        if t.starts_with("vec_chunks") && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            Ok(t)
        } else {
            Err(format!("Invalid vector table name: {t}"))
        }
    }

    pub fn meta(&self) -> Result<KbMeta> {
        let c = self.lock();
        let get = |k: &str| -> Result<String> { Ok(Self::meta_value(&c, k)?.unwrap_or_default()) };
        let num = |k: &str| -> Result<usize> { Ok(get(k)?.parse().unwrap_or(0)) };
        Ok(KbMeta {
            name: get("name")?,
            description: get("description")?,
            embedding: EmbeddingSpec {
                provider: Provider::parse(&get("provider")?).unwrap_or_default(),
                model_id: get("model_id")?,
                dimensions: num("dims")?,
            },
            chunk_size: num("chunk_size")?,
            chunk_overlap: num("chunk_overlap")?,
            created_at: get("created_at")?,
        })
    }

    pub fn set_name(&self, name: &str, description: &str) -> Result<()> {
        let c = self.lock();
        for (k, v) in [("name", name), ("description", description)] {
            c.execute(
                "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )
            .map_err(err)?;
        }
        Ok(())
    }

    pub fn counts(&self) -> Result<Counts> {
        let c = self.lock();
        let n = |sql: &str| -> Result<usize> {
            c.query_row(sql, [], |r| r.get::<_, i64>(0))
                .map(|v| v as usize)
                .map_err(err)
        };
        Ok(Counts {
            sources: n("SELECT COUNT(*) FROM sources")?,
            documents: n("SELECT COUNT(*) FROM documents")?,
            chunks: n("SELECT COUNT(*) FROM chunks")?,
        })
    }

    // ---- sources ----

    pub fn add_source(
        &self,
        id: &str,
        kind: SourceKind,
        uri: &str,
        title: &str,
        options: Option<&UrlOptions>,
        added_at: &str,
    ) -> Result<()> {
        let options_json = options.map(|o| serde_json::to_string(o).unwrap_or_default());
        self.lock()
            .execute(
                "INSERT INTO sources(id, kind, uri, title, options_json, status, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
                params![id, kind.as_str(), uri, title, options_json, added_at],
            )
            .map_err(err)?;
        Ok(())
    }

    fn source_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SourceInfo> {
        let kind: String = r.get("kind")?;
        let kind = if kind == "url" {
            SourceKind::Url
        } else {
            SourceKind::File
        };
        let options: Option<String> = r.get("options_json")?;
        Ok(SourceInfo {
            id: r.get("id")?,
            kind,
            uri: r.get("uri")?,
            title: r.get("title")?,
            options: options.and_then(|o| serde_json::from_str(&o).ok()),
            status: SourceStatus::parse(&r.get::<_, String>("status")?),
            error: r.get("error")?,
            document_count: r.get::<_, i64>("doc_count")? as usize,
            chunk_count: r.get::<_, i64>("chunk_count")? as usize,
            bytes: r.get::<_, i64>("bytes")? as u64,
            added_at: r.get("added_at")?,
            synced_at: r.get("synced_at")?,
        })
    }

    pub fn list_sources(&self) -> Result<Vec<SourceInfo>> {
        let c = self.lock();
        let mut st = c
            .prepare("SELECT * FROM sources ORDER BY added_at, rowid")
            .map_err(err)?;
        let rows = st
            .query_map([], Self::source_from_row)
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn source(&self, id: &str) -> Result<Option<SourceInfo>> {
        self.lock()
            .query_row(
                "SELECT * FROM sources WHERE id = ?1",
                [id],
                Self::source_from_row,
            )
            .optional()
            .map_err(err)
    }

    /// Replace a URL source's crawl options (used from its next sync).
    pub fn set_source_options(&self, id: &str, options: &UrlOptions) -> Result<()> {
        let json = serde_json::to_string(options).map_err(|e| e.to_string())?;
        let n = self
            .lock()
            .execute(
                "UPDATE sources SET options_json = ?2 WHERE id = ?1 AND kind = 'url'",
                params![id, json],
            )
            .map_err(err)?;
        if n == 0 {
            return Err("Website source not found".into());
        }
        Ok(())
    }

    pub fn set_source_status(
        &self,
        id: &str,
        status: SourceStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.lock()
            .execute(
                "UPDATE sources SET status = ?2, error = ?3 WHERE id = ?1",
                params![id, status.as_str(), error],
            )
            .map_err(err)?;
        Ok(())
    }

    /// Mark a source synced and refresh its counts from its documents.
    pub fn finish_source(&self, id: &str, title: Option<&str>, synced_at: &str) -> Result<()> {
        let c = self.lock();
        c.execute(
            "UPDATE sources SET status = 'ready', error = NULL, synced_at = ?2,
                title = COALESCE(?3, title),
                doc_count = (SELECT COUNT(*) FROM documents WHERE source_id = ?1),
                chunk_count = (SELECT COUNT(*) FROM chunks WHERE source_id = ?1),
                bytes = (SELECT COALESCE(SUM(bytes), 0) FROM documents WHERE source_id = ?1)
             WHERE id = ?1",
            params![id, synced_at, title],
        )
        .map_err(err)?;
        Ok(())
    }

    /// Delete a source and everything indexed from it.
    pub fn remove_source(&self, id: &str) -> Result<()> {
        let mut c = self.lock();
        let table = Self::vec_table(&c)?;
        let tx = c.transaction().map_err(err)?;
        let ids: Vec<i64> = {
            let mut st = tx
                .prepare("SELECT id FROM chunks WHERE source_id = ?1")
                .map_err(err)?;
            let ids = st
                .query_map([id], |r| r.get(0))
                .map_err(err)?
                .collect::<rusqlite::Result<_>>()
                .map_err(err)?;
            ids
        };
        Self::delete_chunk_ids(&tx, &table, &ids)?;
        tx.execute("DELETE FROM documents WHERE source_id = ?1", [id])
            .map_err(err)?;
        tx.execute("DELETE FROM sources WHERE id = ?1", [id])
            .map_err(err)?;
        tx.commit().map_err(err)
    }

    fn delete_chunk_ids(tx: &Connection, table: &str, ids: &[i64]) -> Result<()> {
        let mut del_vec = tx
            .prepare_cached(&format!("DELETE FROM {table} WHERE chunk_id = ?1"))
            .map_err(err)?;
        let mut del_fts = tx
            .prepare_cached("DELETE FROM chunks_fts WHERE rowid = ?1")
            .map_err(err)?;
        let mut del = tx
            .prepare_cached("DELETE FROM chunks WHERE id = ?1")
            .map_err(err)?;
        for id in ids {
            del_vec.execute([id]).map_err(err)?;
            del_fts.execute([id]).map_err(err)?;
            del.execute([id]).map_err(err)?;
        }
        Ok(())
    }

    // ---- documents ----

    /// `content_hash` of the source's document at `uri`, if indexed.
    pub fn document_hash(&self, source_id: &str, uri: &str) -> Result<Option<String>> {
        self.lock()
            .query_row(
                "SELECT content_hash FROM documents WHERE source_id = ?1 AND uri = ?2",
                params![source_id, uri],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)
    }

    /// URIs of the source's documents.
    pub fn document_uris(&self, source_id: &str) -> Result<Vec<String>> {
        let c = self.lock();
        let mut st = c
            .prepare("SELECT uri FROM documents WHERE source_id = ?1")
            .map_err(err)?;
        let v = st
            .query_map([source_id], |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<String>>>()
            .map_err(err)?;
        Ok(v)
    }

    /// Replace (or insert) one document and its chunks atomically.
    pub fn replace_document(
        &self,
        source_id: &str,
        uri: &str,
        title: &str,
        content_hash: &str,
        bytes: u64,
        chunks: &[NewChunk],
    ) -> Result<()> {
        let mut c = self.lock();
        let table = Self::vec_table(&c)?;
        let dims: usize = Self::meta_value(&c, "dims")?
            .and_then(|d| d.parse().ok())
            .unwrap_or(0);
        if let Some(bad) = chunks.iter().find(|ch| ch.embedding.len() != dims) {
            return Err(format!(
                "Embedding has {} dimensions, the knowledge base expects {dims}",
                bad.embedding.len()
            ));
        }
        let tx = c.transaction().map_err(err)?;
        Self::delete_document_tx(&tx, &table, source_id, uri)?;
        tx.execute(
            "INSERT INTO documents(source_id, uri, title, content_hash, bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![source_id, uri, title, content_hash, bytes as i64],
        )
        .map_err(err)?;
        let doc_id = tx.last_insert_rowid();
        {
            let mut ins = tx
                .prepare_cached(
                    "INSERT INTO chunks(document_id, source_id, ord, heading_path, text) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(err)?;
            let mut ins_fts = tx
                .prepare_cached(
                    "INSERT INTO chunks_fts(rowid, text, heading_path) VALUES (?1, ?2, ?3)",
                )
                .map_err(err)?;
            let mut ins_vec = tx
                .prepare_cached(&format!(
                    "INSERT INTO {table}(chunk_id, embedding) VALUES (?1, ?2)"
                ))
                .map_err(err)?;
            for (ord, ch) in chunks.iter().enumerate() {
                ins.execute(params![
                    doc_id,
                    source_id,
                    ord as i64,
                    ch.heading_path,
                    ch.text
                ])
                .map_err(err)?;
                let id = tx.last_insert_rowid();
                ins_fts
                    .execute(params![id, ch.text, ch.heading_path])
                    .map_err(err)?;
                ins_vec
                    .execute(params![id, f32_blob(&ch.embedding)])
                    .map_err(err)?;
            }
        }
        tx.commit().map_err(err)
    }

    fn delete_document_tx(tx: &Connection, table: &str, source_id: &str, uri: &str) -> Result<()> {
        let doc: Option<i64> = tx
            .query_row(
                "SELECT id FROM documents WHERE source_id = ?1 AND uri = ?2",
                params![source_id, uri],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let Some(doc) = doc else { return Ok(()) };
        let ids: Vec<i64> = {
            let mut st = tx
                .prepare("SELECT id FROM chunks WHERE document_id = ?1")
                .map_err(err)?;
            let ids = st
                .query_map([doc], |r| r.get(0))
                .map_err(err)?
                .collect::<rusqlite::Result<_>>()
                .map_err(err)?;
            ids
        };
        Self::delete_chunk_ids(tx, table, &ids)?;
        tx.execute("DELETE FROM documents WHERE id = ?1", [doc])
            .map_err(err)?;
        Ok(())
    }

    pub fn remove_document(&self, source_id: &str, uri: &str) -> Result<()> {
        let mut c = self.lock();
        let table = Self::vec_table(&c)?;
        let tx = c.transaction().map_err(err)?;
        Self::delete_document_tx(&tx, &table, source_id, uri)?;
        tx.commit().map_err(err)
    }

    // ---- search ----

    /// Nearest chunks by cosine distance: `(chunk id, distance)`, closest first.
    pub fn search_vector(&self, query: &[f32], k: usize) -> Result<Vec<(i64, f64)>> {
        let c = self.lock();
        let table = Self::vec_table(&c)?;
        let mut st = c
            .prepare(&format!(
                "SELECT chunk_id, distance FROM {table} WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance"
            ))
            .map_err(err)?;
        let rows = st
            .query_map(params![f32_blob(query), k as i64], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(rows)
    }

    /// Keyword matches, best first (BM25 via FTS5 trigram; `LIKE` for terms under 3 chars).
    pub fn search_keyword(&self, query: &str, k: usize) -> Result<Vec<i64>> {
        let (fts, short) = fts_query(query);
        let c = self.lock();
        if let Some(q) = fts {
            let mut st = c
                .prepare(
                    "SELECT rowid FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2",
                )
                .map_err(err)?;
            return st
                .query_map(params![q, k as i64], |r| r.get(0))
                .map_err(err)?
                .collect::<rusqlite::Result<Vec<i64>>>()
                .map_err(err);
        }
        if short.is_empty() {
            return Ok(Vec::new());
        }
        let clauses = vec!["text LIKE ? ESCAPE '\\'"; short.len()].join(" OR ");
        let mut st = c
            .prepare(&format!("SELECT id FROM chunks WHERE {clauses} LIMIT {k}"))
            .map_err(err)?;
        let pats: Vec<String> = short
            .iter()
            .map(|t| {
                format!(
                    "%{}%",
                    t.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                )
            })
            .collect();
        let v = st
            .query_map(rusqlite::params_from_iter(pats.iter()), |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<i64>>>()
            .map_err(err)?;
        Ok(v)
    }

    pub fn chunks_by_id(&self, ids: &[i64]) -> Result<Vec<StoredChunk>> {
        let c = self.lock();
        let mut st = c
            .prepare_cached(
                "SELECT c.id, c.source_id, d.title, d.uri, c.heading_path, c.text
                 FROM chunks c JOIN documents d ON d.id = c.document_id WHERE c.id = ?1",
            )
            .map_err(err)?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(ch) = st
                .query_row([id], |r| {
                    Ok(StoredChunk {
                        id: r.get(0)?,
                        source_id: r.get(1)?,
                        title: r.get(2)?,
                        uri: r.get(3)?,
                        heading_path: r.get(4)?,
                        text: r.get(5)?,
                    })
                })
                .optional()
                .map_err(err)?
            {
                out.push(ch);
            }
        }
        Ok(out)
    }

    // ---- rebuild ----

    /// Every chunk's id and embed text, for re-embedding.
    pub fn all_embed_texts(&self) -> Result<Vec<(i64, String)>> {
        let c = self.lock();
        let mut st = c
            .prepare(
                "SELECT c.id, d.title, c.heading_path, c.text FROM chunks c
                 JOIN documents d ON d.id = c.document_id ORDER BY c.id",
            )
            .map_err(err)?;
        let v = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    embed_text(
                        &r.get::<_, String>(1)?,
                        &r.get::<_, String>(2)?,
                        &r.get::<_, String>(3)?,
                    ),
                ))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(v)
    }

    /// Replace every embedding with new ones from another model, switching the knowledgebase
    /// to `spec`. All-or-nothing: the old table stays in use until the new one is complete.
    pub fn replace_embeddings(
        &self,
        spec: &EmbeddingSpec,
        vectors: &[(i64, Vec<f32>)],
    ) -> Result<()> {
        if let Some((_, v)) = vectors.iter().find(|(_, v)| v.len() != spec.dimensions) {
            return Err(format!(
                "Embedding has {} dimensions, expected {}",
                v.len(),
                spec.dimensions
            ));
        }
        let mut c = self.lock();
        let old = Self::vec_table(&c)?;
        let new = format!("vec_chunks_{}", chrono::Utc::now().timestamp_millis());
        let tx = c.transaction().map_err(err)?;
        tx.execute_batch(&vec_table_sql(&new, spec.dimensions))
            .map_err(err)?;
        {
            let mut ins = tx
                .prepare(&format!(
                    "INSERT INTO {new}(chunk_id, embedding) VALUES (?1, ?2)"
                ))
                .map_err(err)?;
            for (id, v) in vectors {
                ins.execute(params![id, f32_blob(v)]).map_err(err)?;
            }
        }
        tx.execute_batch(&format!("DROP TABLE {old}"))
            .map_err(err)?;
        for (k, v) in [
            ("vec_table", new.clone()),
            ("provider", spec.provider.as_str().to_string()),
            ("model_id", spec.model_id.clone()),
            ("dims", spec.dimensions.to_string()),
        ] {
            tx.execute("UPDATE meta SET value = ?2 WHERE key = ?1", params![k, v])
                .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    // ---- usage and maintenance ----

    /// Record one search (`origin`: `agent` for the model's tool calls, `test` for the editor's
    /// search box). Rows older than [`SEARCH_LOG_DAYS`] are dropped now and then.
    pub fn log_search(&self, at_ms: i64, latency_ms: u64, hits: usize, origin: &str) -> Result<()> {
        let c = self.lock();
        c.execute(
            "INSERT INTO search_log(at, latency_ms, hits, origin) VALUES (?1, ?2, ?3, ?4)",
            params![at_ms, latency_ms as i64, hits as i64, origin],
        )
        .map_err(err)?;
        if c.last_insert_rowid() % 100 == 0 {
            c.execute(
                "DELETE FROM search_log WHERE at < ?1",
                [at_ms - SEARCH_LOG_DAYS * DAY_MS],
            )
            .map_err(err)?;
        }
        Ok(())
    }

    /// Record a finished ingestion / rebuild / optimize job; the last [`JOB_LOG_KEEP`] are kept.
    pub fn log_job(&self, entry: &JobLogEntry) -> Result<()> {
        let c = self.lock();
        c.execute(
            "INSERT INTO job_log(at, kind, source_id, duration_ms, documents, chunks, status, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.at,
                entry.kind,
                entry.source_id,
                entry.duration_ms as i64,
                entry.documents as i64,
                entry.chunks as i64,
                entry.status,
                entry.message
            ],
        )
        .map_err(err)?;
        c.execute(
            "DELETE FROM job_log WHERE id <= (SELECT MAX(id) FROM job_log) - ?1",
            [JOB_LOG_KEEP],
        )
        .map_err(err)?;
        Ok(())
    }

    /// Search and job figures for the last `days` days ending at `now_ms`.
    pub fn usage(&self, now_ms: i64, days: usize) -> Result<Usage> {
        let c = self.lock();
        let since = now_ms - days as i64 * DAY_MS;
        let mut daily = vec![0u32; days];
        {
            let mut st = c
                .prepare(
                    "SELECT at FROM search_log WHERE origin = 'agent' AND at >= ?1 AND at <= ?2",
                )
                .map_err(err)?;
            let rows = st
                .query_map([since, now_ms], |r| r.get::<_, i64>(0))
                .map_err(err)?;
            for at in rows {
                let at = at.map_err(err)?;
                let back = ((now_ms - at) / DAY_MS) as usize;
                if back < days {
                    daily[days - 1 - back] += 1;
                }
            }
        }
        let mut latencies: Vec<i64> = {
            let mut st = c
                .prepare("SELECT latency_ms FROM search_log WHERE at >= ?1 ORDER BY latency_ms")
                .map_err(err)?;
            let v = st
                .query_map([since], |r| r.get(0))
                .map_err(err)?
                .collect::<rusqlite::Result<Vec<i64>>>()
                .map_err(err)?;
            v
        };
        latencies.sort_unstable();
        let pct = |p: f64| -> Option<u64> {
            (!latencies.is_empty()).then(|| {
                let i = ((latencies.len() as f64 - 1.0) * p).round() as usize;
                latencies[i] as u64
            })
        };
        let (total, test_searches, last_search_at, avg_hits): (i64, i64, Option<i64>, Option<f64>) =
            c.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM search_log WHERE origin = 'agent'),
                    (SELECT COUNT(*) FROM search_log WHERE origin = 'test' AND at >= ?1),
                    (SELECT MAX(at) FROM search_log WHERE origin = 'agent'),
                    (SELECT AVG(hits) FROM search_log WHERE at >= ?1)",
                [since],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(err)?;
        let last_job = c
            .query_row(
                "SELECT at, kind, source_id, duration_ms, documents, chunks, status, message
                 FROM job_log WHERE kind != 'optimize' ORDER BY id DESC LIMIT 1",
                [],
                JobLogEntry::from_row,
            )
            .optional()
            .map_err(err)?;
        let last_optimized_at: Option<i64> = c
            .query_row(
                "SELECT MAX(at) FROM job_log WHERE kind = 'optimize'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let avg_chunk_tokens: Option<f64> = c
            .query_row("SELECT AVG(LENGTH(text)) FROM chunks", [], |r| r.get(0))
            .map_err(err)?;
        Ok(Usage {
            daily_searches: daily.clone(),
            searches_in_period: daily.iter().map(|d| *d as u64).sum(),
            searches_total: total as u64,
            test_searches_in_period: test_searches as u64,
            last_search_at,
            latency_p50_ms: pct(0.5),
            latency_p95_ms: pct(0.95),
            avg_hits,
            last_job,
            last_optimized_at,
            avg_chunk_chars: avg_chunk_tokens.map(|v| v.round() as u64),
        })
    }

    /// Page counts: `(page size, total pages, free pages)`.
    pub fn pages(&self) -> Result<(u64, u64, u64)> {
        let c = self.lock();
        let get = |p: &str| -> Result<u64> {
            c.query_row(&format!("PRAGMA {p}"), [], |r| r.get::<_, i64>(0))
                .map(|v| v as u64)
                .map_err(err)
        };
        Ok((
            get("page_size")?,
            get("page_count")?,
            get("freelist_count")?,
        ))
    }

    /// Merge the keyword index, check integrity, fold the WAL back in, and rewrite the file
    /// without its free pages. Returns the integrity check result (`"ok"` when healthy).
    pub fn optimize(&self) -> Result<String> {
        let c = self.lock();
        c.execute_batch("INSERT INTO chunks_fts(chunks_fts) VALUES('optimize');")
            .map_err(err)?;
        let integrity: String = c
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(err)?;
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA optimize; PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(err)?;
        Ok(integrity)
    }

    /// Fold the WAL into the main file (after a job), without blocking readers.
    pub fn checkpoint(&self) {
        let _ = self.lock().execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    }

    /// A consistent copy of the database at `dest` (works while other connections write).
    pub fn copy_to(&self, dest: &Path) -> Result<()> {
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        self.lock()
            .execute("VACUUM INTO ?1", [dest.to_string_lossy()])
            .map_err(err)?;
        Ok(())
    }
}

/// A finished job, as logged.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobLogEntry {
    pub at: i64,
    /// `source` / `rebuild` / `optimize`.
    pub kind: String,
    pub source_id: Option<String>,
    pub duration_ms: u64,
    pub documents: usize,
    pub chunks: usize,
    /// `done` / `error` / `cancelled`.
    pub status: String,
    pub message: String,
}

impl JobLogEntry {
    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            at: r.get(0)?,
            kind: r.get(1)?,
            source_id: r.get(2)?,
            duration_ms: r.get::<_, i64>(3)? as u64,
            documents: r.get::<_, i64>(4)? as usize,
            chunks: r.get::<_, i64>(5)? as usize,
            status: r.get(6)?,
            message: r.get(7)?,
        })
    }
}

/// Usage over a period (see [`KbStore::usage`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Agent searches per day, oldest first, today last.
    pub daily_searches: Vec<u32>,
    pub searches_in_period: u64,
    pub searches_total: u64,
    /// Searches from the editor's search box.
    pub test_searches_in_period: u64,
    pub last_search_at: Option<i64>,
    pub latency_p50_ms: Option<u64>,
    pub latency_p95_ms: Option<u64>,
    pub avg_hits: Option<f64>,
    pub last_job: Option<JobLogEntry>,
    pub last_optimized_at: Option<i64>,
    pub avg_chunk_chars: Option<u64>,
}

/// Database file plus its WAL / shared-memory files.
pub fn db_files(path: &Path) -> [PathBuf; 3] {
    let s = path.as_os_str().to_owned();
    let with = |suffix: &str| {
        let mut p = s.clone();
        p.push(suffix);
        PathBuf::from(p)
    };
    [path.to_path_buf(), with("-wal"), with("-shm")]
}

/// Total size of the database files.
pub fn db_size(path: &Path) -> u64 {
    db_files(path)
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn meta(dims: usize) -> KbMeta {
        KbMeta {
            name: "Docs".into(),
            description: "d".into(),
            embedding: EmbeddingSpec {
                provider: Provider::Bedrock,
                model_id: "fake".into(),
                dimensions: dims,
            },
            chunk_size: 100,
            chunk_overlap: 10,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn chunk(text: &str, v: [f32; 3]) -> NewChunk {
        NewChunk {
            heading_path: "H".into(),
            text: text.into(),
            embedding: v.to_vec(),
        }
    }

    #[test]
    fn fts_query_quotes_terms() {
        assert_eq!(
            fts_query(r#"hello "wor*ld" ab"#),
            (Some(r#""hello" OR "world""#.into()), vec!["ab".into()])
        );
        assert_eq!(fts_query("  "), (None, vec![]));
    }

    #[test]
    fn create_open_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("kb.db");
        let s = KbStore::create(&path, &meta(3)).unwrap();
        assert!(KbStore::create(&path, &meta(3)).is_err());
        assert_eq!(s.meta().unwrap(), meta(3));

        s.add_source("s1", SourceKind::File, "/x.md", "x.md", None, "t0")
            .unwrap();
        s.replace_document(
            "s1",
            "/x.md",
            "x.md",
            "h1",
            42,
            &[
                chunk("the quick brown fox", [1.0, 0.0, 0.0]),
                chunk("lazy dog sleeps", [0.0, 1.0, 0.0]),
                chunk("日本語のテキスト", [0.0, 0.0, 1.0]),
            ],
        )
        .unwrap();
        s.finish_source("s1", None, "t1").unwrap();
        let src = s.source("s1").unwrap().unwrap();
        assert_eq!(src.status, SourceStatus::Ready);
        assert_eq!((src.document_count, src.chunk_count, src.bytes), (1, 3, 42));
        assert_eq!(
            s.document_hash("s1", "/x.md").unwrap().as_deref(),
            Some("h1")
        );

        let near = s.search_vector(&[0.9, 0.1, 0.0], 2).unwrap();
        assert_eq!(near.len(), 2);
        let texts = s
            .chunks_by_id(&near.iter().map(|n| n.0).collect::<Vec<_>>())
            .unwrap();
        assert_eq!(texts[0].text, "the quick brown fox");
        assert_eq!(texts[0].title, "x.md");

        let kw = s.search_keyword("dog", 5).unwrap();
        assert_eq!(s.chunks_by_id(&kw).unwrap()[0].text, "lazy dog sleeps");
        let kw = s.search_keyword("日本語", 5).unwrap();
        assert_eq!(kw.len(), 1);
        let kw = s.search_keyword("ox", 5).unwrap();
        assert_eq!(kw.len(), 1, "short terms use LIKE");

        // Wrong dimensions are rejected.
        assert!(s
            .replace_document(
                "s1",
                "/y",
                "y",
                "h",
                1,
                &[NewChunk {
                    heading_path: String::new(),
                    text: "t".into(),
                    embedding: vec![1.0; 4],
                }]
            )
            .is_err());

        // Replacing a document drops its old chunks.
        s.replace_document(
            "s1",
            "/x.md",
            "x.md",
            "h2",
            5,
            &[chunk("new text", [1.0, 0.0, 0.0])],
        )
        .unwrap();
        assert_eq!(s.counts().unwrap().chunks, 1);
        assert!(s.search_keyword("dog", 5).unwrap().is_empty());
        drop(s);

        let s = KbStore::open(&path).unwrap();
        s.set_name("Renamed", "").unwrap();
        assert_eq!(s.meta().unwrap().name, "Renamed");
        s.remove_source("s1").unwrap();
        assert_eq!(s.counts().unwrap(), Counts::default());
        assert!(s.search_vector(&[1.0, 0.0, 0.0], 5).unwrap().is_empty());
    }

    #[test]
    fn rebuild_switches_model_and_dims() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kb.db");
        let s = KbStore::create(&path, &meta(3)).unwrap();
        s.add_source(
            "s",
            SourceKind::Url,
            "https://x",
            "x",
            Some(&UrlOptions::default()),
            "t",
        )
        .unwrap();
        s.replace_document(
            "s",
            "https://x/a",
            "Page A",
            "h",
            1,
            &[chunk("alpha", [1.0, 0.0, 0.0])],
        )
        .unwrap();
        let texts = s.all_embed_texts().unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].1, "Page A › H\nalpha");
        let spec = EmbeddingSpec {
            provider: Provider::Local,
            model_id: "m2".into(),
            dimensions: 2,
        };
        assert!(s
            .replace_embeddings(&spec, &[(texts[0].0, vec![1.0; 3])])
            .is_err());
        s.replace_embeddings(&spec, &[(texts[0].0, vec![0.0, 1.0])])
            .unwrap();
        assert_eq!(s.meta().unwrap().embedding, spec);
        let near = s.search_vector(&[0.0, 1.0], 1).unwrap();
        assert_eq!(near[0].0, texts[0].0);
        assert_eq!(
            s.source("s").unwrap().unwrap().options,
            Some(UrlOptions::default())
        );
        let o = UrlOptions {
            max_depth: 4,
            max_pages: 300,
            path_prefix_only: false,
        };
        s.set_source_options("s", &o).unwrap();
        assert_eq!(s.source("s").unwrap().unwrap().options, Some(o.clone()));
        assert!(s.set_source_options("missing", &o).is_err());
    }

    #[test]
    fn version_1_files_are_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kb.db");
        let s = KbStore::create(&path, &meta(3)).unwrap();
        // Pretend an older app made it: no log tables, version 1.
        s.lock()
            .execute_batch(
                "DROP TABLE search_log; DROP TABLE job_log;
                 UPDATE meta SET value = '1' WHERE key = 'schema_version';",
            )
            .unwrap();
        drop(s);
        let s = KbStore::open(&path).unwrap();
        s.log_search(1_000, 5, 2, "agent").unwrap();
        let u = s.usage(2_000, 7).unwrap();
        assert_eq!(u.searches_total, 1);
        assert_eq!(u.daily_searches, vec![0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(u.latency_p50_ms, Some(5));
    }

    #[test]
    fn job_log_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let s = KbStore::create(&dir.path().join("kb.db"), &meta(3)).unwrap();
        for i in 0..(JOB_LOG_KEEP + 5) {
            s.log_job(&JobLogEntry {
                at: i,
                kind: "source".into(),
                source_id: None,
                duration_ms: 1,
                documents: 0,
                chunks: 0,
                status: "done".into(),
                message: String::new(),
            })
            .unwrap();
        }
        let n: i64 = s
            .lock()
            .query_row("SELECT COUNT(*) FROM job_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, JOB_LOG_KEEP);
        assert_eq!(
            s.usage(1, 1).unwrap().last_job.unwrap().at,
            JOB_LOG_KEEP + 4
        );
    }

    #[test]
    fn copy_and_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kb.db");
        let s = KbStore::create(&path, &meta(3)).unwrap();
        s.add_source("s", SourceKind::File, "/f", "f", None, "t")
            .unwrap();
        let dest = dir.path().join("copy").join("kb.db");
        s.copy_to(&dest).unwrap();
        let c = KbStore::open(&dest).unwrap();
        assert_eq!(c.list_sources().unwrap().len(), 1);
        assert!(db_size(&path) > 0);
        assert!(KbStore::open(&dir.path().join("none.db")).is_err());
    }
}
