//! The on-device embedding model: `intfloat/multilingual-e5-small` (384 dimensions, about 100
//! languages), downloaded on demand into `<root>/.models/multilingual-e5-small` and run with
//! candle on the CPU.
//!
//! Files come from a pinned Hugging Face revision and are checked against their SHA-256 before
//! they are put in place. e5 expects `"query: "` / `"passage: "` prefixes; embeddings are the
//! mean of the last hidden states over the attention mask, L2-normalized.

use crate::embed::{EmbedInput, Embedder};
use crate::types::{EmbeddingSpec, LOCAL_MODEL_DIMENSIONS, LOCAL_MODEL_ID};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub const DOWNLOAD_EVENT: &str = "knowledge:model-download";

const REPO: &str = "intfloat/multilingual-e5-small";
pub const REVISION: &str = "614241f622f53c4eeff9890bdc4f31cfecc418b3";

/// `(file, size, sha256)`.
const FILES: &[(&str, u64, &str)] = &[
    (
        "config.json",
        655,
        "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959",
    ),
    (
        "tokenizer.json",
        17_082_730,
        "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
    ),
    (
        "model.safetensors",
        470_641_600,
        "1a55775f53449dac10a2bcbc312469fac40b96d53198c407081a831f81c98477",
    ),
];

/// Longest input the model takes, in tokens.
const MAX_TOKENS: usize = 512;

pub fn model_dir(models_dir: &Path) -> PathBuf {
    models_dir.join(LOCAL_MODEL_ID)
}

/// Whether every model file is present with the expected size.
pub fn is_installed(models_dir: &Path) -> bool {
    let dir = model_dir(models_dir);
    FILES
        .iter()
        .all(|(name, size, _)| std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() == *size))
}

pub fn download_size() -> u64 {
    FILES.iter().map(|(_, s, _)| s).sum()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelStatus {
    pub installed: bool,
    pub bytes: u64,
    pub downloading: bool,
    pub total_bytes: u64,
    /// Whether this build can run the model.
    pub supported: bool,
}

/// Download, status and loading of the local model.
pub struct LocalModel {
    models_dir: PathBuf,
    downloading: AtomicBool,
    cancel: Mutex<Option<CancellationToken>>,
    #[cfg(feature = "local-embeddings")]
    loaded: Mutex<Option<Arc<engine::E5>>>,
}

impl LocalModel {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            downloading: AtomicBool::new(false),
            cancel: Mutex::new(None),
            #[cfg(feature = "local-embeddings")]
            loaded: Mutex::new(None),
        }
    }

    pub fn status(&self) -> LocalModelStatus {
        let dir = model_dir(&self.models_dir);
        let installed = is_installed(&self.models_dir);
        LocalModelStatus {
            installed,
            bytes: if installed {
                FILES
                    .iter()
                    .filter_map(|(n, _, _)| std::fs::metadata(dir.join(n)).ok())
                    .map(|m| m.len())
                    .sum()
            } else {
                0
            },
            downloading: self.downloading.load(Ordering::SeqCst),
            total_bytes: download_size(),
            supported: cfg!(feature = "local-embeddings"),
        }
    }

    /// Download the model files (skipping ones already in place). Progress goes to `emit` as
    /// `{ state, received, total, file, error? }`. Only one download runs at a time.
    pub async fn download(
        &self,
        client: &reqwest::Client,
        emit: &(dyn Fn(Value) + Send + Sync),
    ) -> Result<(), String> {
        if !cfg!(feature = "local-embeddings") {
            return Err("Local embeddings are not available in this build".into());
        }
        if self.downloading.swap(true, Ordering::SeqCst) {
            return Err("The model is already downloading".into());
        }
        let token = CancellationToken::new();
        *self.cancel.lock().unwrap_or_else(|p| p.into_inner()) = Some(token.clone());
        let result = self.download_files(client, emit, &token).await;
        self.downloading.store(false, Ordering::SeqCst);
        *self.cancel.lock().unwrap_or_else(|p| p.into_inner()) = None;
        let total = download_size();
        match &result {
            Ok(()) => {
                emit(json!({ "state": "done", "received": total, "total": total, "file": "" }))
            }
            Err(e) => emit(
                json!({ "state": "error", "received": 0, "total": total, "file": "", "error": e }),
            ),
        }
        result
    }

    async fn download_files(
        &self,
        client: &reqwest::Client,
        emit: &(dyn Fn(Value) + Send + Sync),
        token: &CancellationToken,
    ) -> Result<(), String> {
        let dir = model_dir(&self.models_dir);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        let total = download_size();
        let mut done: u64 = 0;
        for (name, size, sha) in FILES {
            let dest = dir.join(name);
            if std::fs::metadata(&dest).is_ok_and(|m| m.len() == *size) {
                done += size;
                continue;
            }
            let url = format!("https://huggingface.co/{REPO}/resolve/{REVISION}/{name}");
            let part = dir.join(format!("{name}.part"));
            let mut resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Download failed ({name}): {e}"))?;
            if !resp.status().is_success() {
                return Err(format!(
                    "Download failed ({name}): HTTP {}",
                    resp.status().as_u16()
                ));
            }
            let mut file = tokio::fs::File::create(&part)
                .await
                .map_err(|e| format!("Cannot write {}: {e}", part.display()))?;
            let mut hasher = Sha256::new();
            let mut received: u64 = 0;
            let mut last_emit = std::time::Instant::now();
            loop {
                let chunk = tokio::select! {
                    c = resp.chunk() => c.map_err(|e| format!("Download failed ({name}): {e}"))?,
                    _ = token.cancelled() => {
                        drop(file);
                        let _ = tokio::fs::remove_file(&part).await;
                        return Err("Download cancelled".into());
                    }
                };
                let Some(chunk) = chunk else { break };
                hasher.update(&chunk);
                received += chunk.len() as u64;
                file.write_all(&chunk)
                    .await
                    .map_err(|e| format!("Cannot write {}: {e}", part.display()))?;
                if last_emit.elapsed() > std::time::Duration::from_millis(250) {
                    last_emit = std::time::Instant::now();
                    emit(
                        json!({ "state": "downloading", "received": done + received, "total": total, "file": name }),
                    );
                }
            }
            file.flush().await.map_err(|e| e.to_string())?;
            drop(file);
            let digest: String = hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if digest != *sha || received != *size {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(format!(
                    "Downloaded {name} does not match the expected checksum"
                ));
            }
            tokio::fs::rename(&part, &dest)
                .await
                .map_err(|e| format!("Cannot move {} into place: {e}", dest.display()))?;
            done += size;
            emit(json!({ "state": "downloading", "received": done, "total": total, "file": name }));
        }
        Ok(())
    }

    /// Stop a running download.
    pub fn cancel_download(&self) {
        if let Some(t) = self
            .cancel
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            t.cancel();
        }
    }

    /// Delete the model files (cancelling a download first).
    pub fn delete(&self) -> Result<(), String> {
        self.cancel_download();
        #[cfg(feature = "local-embeddings")]
        {
            *self.loaded.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }
        let dir = model_dir(&self.models_dir);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Cannot delete {}: {e}", dir.display())),
        }
    }

    pub fn embedder(self: &Arc<Self>, spec: &EmbeddingSpec) -> Result<Arc<dyn Embedder>, String> {
        if spec.model_id != LOCAL_MODEL_ID || spec.dimensions != LOCAL_MODEL_DIMENSIONS {
            return Err(format!("Unknown local embedding model: {}", spec.model_id));
        }
        if !cfg!(feature = "local-embeddings") {
            return Err("Local embeddings are not available in this build".into());
        }
        if !is_installed(&self.models_dir) {
            return Err("The local embedding model is not downloaded yet. Download it in Settings > Knowledge Base.".into());
        }
        Ok(Arc::new(LocalEmbedder(self.clone())))
    }

    #[cfg(feature = "local-embeddings")]
    fn engine(&self) -> Result<Arc<engine::E5>, String> {
        let mut loaded = self.loaded.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = loaded.as_ref() {
            return Ok(e.clone());
        }
        let e = Arc::new(engine::E5::load(&model_dir(&self.models_dir))?);
        *loaded = Some(e.clone());
        Ok(e)
    }
}

struct LocalEmbedder(Arc<LocalModel>);

/// Texts per forward pass.
#[cfg(feature = "local-embeddings")]
const BATCH: usize = 16;

#[async_trait]
impl Embedder for LocalEmbedder {
    fn dimensions(&self) -> usize {
        LOCAL_MODEL_DIMENSIONS
    }

    #[cfg(feature = "local-embeddings")]
    async fn embed(&self, texts: &[String], input: EmbedInput) -> Result<Vec<Vec<f32>>, String> {
        let prefix = match input {
            EmbedInput::Query => "query: ",
            EmbedInput::Document => "passage: ",
        };
        let texts: Vec<String> = texts.iter().map(|t| format!("{prefix}{t}")).collect();
        let model = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let engine = model.engine()?;
            let mut out = Vec::with_capacity(texts.len());
            for batch in texts.chunks(BATCH) {
                out.extend(engine.embed(batch)?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    #[cfg(not(feature = "local-embeddings"))]
    async fn embed(&self, _: &[String], _: EmbedInput) -> Result<Vec<Vec<f32>>, String> {
        Err("Local embeddings are not available in this build".into())
    }
}

#[cfg(feature = "local-embeddings")]
mod engine {
    use super::MAX_TOKENS;
    use candle_core::{DType, Device, Tensor};
    use candle_nn::VarBuilder;
    use candle_transformers::models::bert::{BertModel, Config};
    use std::path::Path;
    use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

    pub struct E5 {
        model: BertModel,
        tokenizer: Tokenizer,
        device: Device,
    }

    fn err(e: impl std::fmt::Display) -> String {
        format!("Local embedding model: {e}")
    }

    impl E5 {
        pub fn load(dir: &Path) -> Result<Self, String> {
            let device = Device::Cpu;
            let config: Config = serde_json::from_str(
                &std::fs::read_to_string(dir.join("config.json")).map_err(err)?,
            )
            .map_err(err)?;
            let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(err)?;
            tokenizer
                .with_truncation(Some(TruncationParams {
                    max_length: MAX_TOKENS,
                    ..Default::default()
                }))
                .map_err(err)?;
            tokenizer.with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                ..Default::default()
            }));
            // SAFETY: the file is ours (checksummed on download) and not modified while mapped.
            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(
                    &[dir.join("model.safetensors")],
                    DType::F32,
                    &device,
                )
            }
            .map_err(err)?;
            let model = BertModel::load(vb, &config).map_err(err)?;
            Ok(Self {
                model,
                tokenizer,
                device,
            })
        }

        /// Mean-pooled, L2-normalized embeddings.
        pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            if texts.is_empty() {
                return Ok(Vec::new());
            }
            let encodings = self
                .tokenizer
                .encode_batch(texts.to_vec(), true)
                .map_err(err)?;
            let ids: Vec<Tensor> = encodings
                .iter()
                .map(|e| Tensor::new(e.get_ids(), &self.device))
                .collect::<candle_core::Result<_>>()
                .map_err(err)?;
            let masks: Vec<Tensor> = encodings
                .iter()
                .map(|e| Tensor::new(e.get_attention_mask(), &self.device))
                .collect::<candle_core::Result<_>>()
                .map_err(err)?;
            let input_ids = Tensor::stack(&ids, 0).map_err(err)?;
            let mask = Tensor::stack(&masks, 0).map_err(err)?;
            let token_types = input_ids.zeros_like().map_err(err)?;
            let hidden = self
                .model
                .forward(&input_ids, &token_types, Some(&mask))
                .map_err(err)?;
            // Mean over real tokens: sum(hidden * mask) / sum(mask).
            let mask_f = mask
                .to_dtype(DType::F32)
                .map_err(err)?
                .unsqueeze(2)
                .map_err(err)?;
            let summed = hidden
                .broadcast_mul(&mask_f)
                .and_then(|t| t.sum(1))
                .map_err(err)?;
            let counts = mask_f.sum(1).map_err(err)?;
            let mean = summed.broadcast_div(&counts).map_err(err)?;
            let norm = mean
                .sqr()
                .and_then(|t| t.sum_keepdim(1))
                .and_then(|t| t.sqrt())
                .map_err(err)?;
            let normed = mean.broadcast_div(&norm).map_err(err)?;
            normed.to_vec2::<f32>().map_err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_without_files() {
        let dir = tempfile::tempdir().unwrap();
        let m = LocalModel::new(dir.path().to_path_buf());
        let s = m.status();
        assert!(!s.installed);
        assert_eq!(s.total_bytes, download_size());
        assert!(download_size() > 400_000_000);
        assert!(m.delete().is_ok());
        let m = Arc::new(m);
        let spec = EmbeddingSpec {
            provider: crate::types::Provider::Local,
            model_id: LOCAL_MODEL_ID.into(),
            dimensions: LOCAL_MODEL_DIMENSIONS,
        };
        assert!(m.embedder(&spec).err().unwrap().contains("not downloaded"));
    }

    /// Downloads the real model (~490 MB) and embeds with it.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn downloads_and_embeds() {
        let dir = std::env::temp_dir().join("bedrock-engineer-e5-test");
        let m = Arc::new(LocalModel::new(dir.clone()));
        m.download(&reqwest::Client::new(), &|_| {}).await.unwrap();
        assert!(m.status().installed);
        let spec = EmbeddingSpec {
            provider: crate::types::Provider::Local,
            model_id: LOCAL_MODEL_ID.into(),
            dimensions: LOCAL_MODEL_DIMENSIONS,
        };
        let e = m.embedder(&spec).unwrap();
        let docs = e
            .embed(
                &[
                    "The capital of France is Paris.".to_string(),
                    "猫はとてもかわいい動物です。".to_string(),
                ],
                EmbedInput::Document,
            )
            .await
            .unwrap();
        let q = e
            .embed(
                &["What is the capital of France?".to_string()],
                EmbedInput::Query,
            )
            .await
            .unwrap();
        assert_eq!(docs[0].len(), 384);
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!((dot(&docs[0], &docs[0]) - 1.0).abs() < 1e-3);
        assert!(dot(&q[0], &docs[0]) > dot(&q[0], &docs[1]));
    }
}
