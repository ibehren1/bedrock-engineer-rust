//! Embedders: Bedrock (Titan / Cohere) and the on-device model.

use crate::types::{EmbeddingSpec, Provider};
use async_trait::async_trait;
use bedrock::embed::{self as bedrock_embed, EmbeddingFamily, InputType};
use bedrock::{AwsSettings, SdkConfigSource};
use std::sync::Arc;

pub use bedrock_embed::InputType as EmbedInput;

#[async_trait]
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    /// One vector per text, in order.
    async fn embed(&self, texts: &[String], input: InputType) -> Result<Vec<Vec<f32>>, String>;
}

/// Builds the embedder for a knowledgebase's spec.
pub trait EmbedderProvider: Send + Sync {
    fn embedder(&self, spec: &EmbeddingSpec) -> Result<Arc<dyn Embedder>, String>;
}

/// Parallel Titan calls (one text per request).
const TITAN_CONCURRENCY: usize = 8;

pub struct BedrockEmbedder {
    sdk: Arc<dyn SdkConfigSource>,
    aws: AwsSettings,
    model_id: String,
    family: EmbeddingFamily,
    dimensions: usize,
    /// Built on first use and kept for the embedder's life (one ingest job or search), so a
    /// job doesn't reload the AWS config and open new connections for every batch.
    client: tokio::sync::OnceCell<bedrock_embed::EmbeddingClient>,
}

impl BedrockEmbedder {
    pub fn new(
        sdk: Arc<dyn SdkConfigSource>,
        aws: AwsSettings,
        spec: &EmbeddingSpec,
    ) -> Result<Self, String> {
        let family = bedrock_embed::embedding_family(&spec.model_id)
            .ok_or_else(|| format!("Unsupported embedding model: {}", spec.model_id))?;
        Ok(Self {
            sdk,
            aws,
            model_id: spec.model_id.clone(),
            family,
            dimensions: spec.dimensions,
            client: tokio::sync::OnceCell::new(),
        })
    }
}

fn describe(e: bedrock::Error) -> String {
    match e.service_name() {
        Some("AccessDeniedException") => format!(
            "No access to the embedding model ({}). Enable it in the Bedrock console for this region, or switch to local embeddings in Settings.",
            e.message()
        ),
        Some("ValidationException") | Some("ResourceNotFoundException") => format!(
            "The embedding model is not available: {}. It may not be offered in this region; try another region or local embeddings.",
            e.message()
        ),
        Some("UnrecognizedClientException") => {
            "AWS authentication failed. Please check your credentials.".to_string()
        }
        _ => e.message(),
    }
}

#[async_trait]
impl Embedder for BedrockEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn embed(&self, texts: &[String], input: InputType) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let client = self
            .client
            .get_or_try_init(|| bedrock_embed::embedding_client(self.sdk.as_ref(), &self.aws))
            .await
            .map_err(describe)?;
        match self.family {
            EmbeddingFamily::Titan => bedrock_embed::embed_titan(
                client,
                &self.model_id,
                texts,
                input,
                self.dimensions,
                TITAN_CONCURRENCY,
            )
            .await
            .map_err(describe),
            EmbeddingFamily::Cohere => {
                let mut out = Vec::with_capacity(texts.len());
                for batch in texts.chunks(bedrock_embed::COHERE_MAX_BATCH) {
                    out.extend(
                        bedrock_embed::embed_cohere(
                            client,
                            &self.model_id,
                            batch,
                            input,
                            self.dimensions,
                        )
                        .await
                        .map_err(describe)?,
                    );
                }
                Ok(out)
            }
        }
    }
}

/// The app's embedders: Bedrock with the current AWS settings, or the on-device model.
pub struct DefaultEmbedders {
    pub sdk: Arc<dyn SdkConfigSource>,
    /// The `aws` settings at the time of the call.
    pub aws: Arc<dyn Fn() -> AwsSettings + Send + Sync>,
    pub local: Arc<crate::local::LocalModel>,
}

impl EmbedderProvider for DefaultEmbedders {
    fn embedder(&self, spec: &EmbeddingSpec) -> Result<Arc<dyn Embedder>, String> {
        match spec.provider {
            Provider::Bedrock => Ok(Arc::new(BedrockEmbedder::new(
                self.sdk.clone(),
                (self.aws)(),
                spec,
            )?)),
            Provider::Local => self.local.embedder(spec),
        }
    }
}

/// How many texts to send to an embedder at once (progress is reported per batch).
pub fn batch_size(spec: &EmbeddingSpec) -> usize {
    match spec.provider {
        Provider::Local => 32,
        Provider::Bedrock => 32,
    }
}

/// Embed and check the dimensions.
pub async fn embed_checked(
    embedder: &dyn Embedder,
    texts: &[String],
    input: InputType,
    dims: usize,
) -> Result<Vec<Vec<f32>>, String> {
    let out = embedder.embed(texts, input).await?;
    if out.len() != texts.len() {
        return Err(format!(
            "Embedding model returned {} vectors for {} texts",
            out.len(),
            texts.len()
        ));
    }
    if let Some(v) = out.iter().find(|v| v.len() != dims) {
        return Err(format!(
            "Embedding model returned {} dimensions, the knowledge base expects {dims}",
            v.len()
        ));
    }
    Ok(out)
}

#[cfg(test)]
pub mod fake {
    //! Deterministic embeddings for tests: hashed character trigrams, normalized, so texts
    //! sharing words end up close.
    use super::*;

    pub struct FakeEmbedder(pub usize);

    pub fn vector(text: &str, dims: usize) -> Vec<f32> {
        let mut v = vec![0f32; dims];
        let lower = text.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        for w in chars.windows(3) {
            let mut h: u64 = 1469598103934665603;
            for c in w {
                h ^= *c as u64;
                h = h.wrapping_mul(1099511628211);
            }
            v[(h % dims as u64) as usize] += 1.0;
        }
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 {
            v.iter_mut().for_each(|x| *x /= n);
        } else {
            v[0] = 1.0;
        }
        v
    }

    #[async_trait]
    impl Embedder for FakeEmbedder {
        fn dimensions(&self) -> usize {
            self.0
        }
        async fn embed(&self, texts: &[String], _: InputType) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts.iter().map(|t| vector(t, self.0)).collect())
        }
    }

    pub struct FakeProvider;

    impl EmbedderProvider for FakeProvider {
        fn embedder(&self, spec: &EmbeddingSpec) -> Result<Arc<dyn Embedder>, String> {
            Ok(Arc::new(FakeEmbedder(spec.dimensions)))
        }
    }
}
