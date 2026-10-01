//! Amazon Translate — port of `src/main/api/bedrock/services/translateService.ts`
//! (`bedrock_translate_text`, `bedrock_translate_batch`, `bedrock_get_translation_cache`,
//! `bedrock_clear_translation_cache`, `bedrock_get_translation_cache_stats`).
//!
//! [`TranslateService`] owns the translation cache (1000 entries, oldest evicted first), so the
//! app should keep one instance for its lifetime, like the TS singleton. Unlike the TS, which
//! built its client from the `aws` settings at startup, each call uses the settings passed in.

use crate::error::{Error, Result};
use crate::sdk::SdkConfigSource;
use crate::settings::AwsSettings;
use aws_sdk_translate::types::{Formality, Profanity, TranslationSettings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// `maxCacheSize`.
pub const MAX_CACHE_SIZE: usize = 1000;
/// Longest accepted text, in UTF-16 code units (`text.length > 10000`).
pub const MAX_TEXT_LENGTH: usize = 10_000;

/// `TranslateTextOptions`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateTextOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    pub target_language: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
}

/// `appliedSettings`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profanity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formality: Option<String>,
}

/// `TranslationResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
    pub original_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_terminologies: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_settings: Option<AppliedSettings>,
}

/// `getCacheStats()` (`hitRate` is never set by the TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
}

/// `simpleHash(str)`: the Java-style 31x string hash over UTF-16 code units, `Math.abs`, base 36.
pub fn simple_hash(s: &str) -> String {
    let mut hash: i32 = 0;
    for unit in s.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit));
    }
    to_base36(i64::from(hash).unsigned_abs())
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// `getCacheKey(text, source, target)`: `${source}-${target}-${text.substring(0, 100)}-${hash}`.
/// The preview is the first 100 UTF-16 code units (a split surrogate pair becomes U+FFFD).
pub fn cache_key(text: &str, source_language: &str, target_language: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().take(100).collect();
    let preview = String::from_utf16_lossy(&units);
    format!(
        "{source_language}-{target_language}-{preview}-{}",
        simple_hash(text)
    )
}

#[derive(Debug, Default)]
struct Cache {
    order: VecDeque<String>,
    entries: HashMap<String, TranslationResult>,
}

impl Cache {
    fn get(&self, key: &str) -> Option<TranslationResult> {
        self.entries.get(key).cloned()
    }

    /// `cacheTranslation`: evict the oldest key when full, then insert. Re-setting an existing
    /// key keeps its position (JS `Map.set`).
    fn insert(&mut self, key: String, value: TranslationResult) {
        if self.entries.len() >= MAX_CACHE_SIZE {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
    }
}

fn translation_error(e: &Error, text: &str) -> Error {
    let (code, request_id) = match e {
        Error::Service(s) => (s.name.clone(), s.request_id.clone()),
        other => (other.name(), None),
    };
    let mut fields = Map::new();
    fields.insert("code".into(), json!(code));
    fields.insert("originalText".into(), json!(text));
    if let Some(id) = request_id {
        fields.insert("requestId".into(), json!(id));
    }
    Error::Custom {
        name: code,
        message: e.message(),
        fields,
    }
}

/// The TS `TranslateService`.
#[derive(Debug, Default)]
pub struct TranslateService {
    cache: Mutex<Cache>,
}

impl TranslateService {
    pub fn new() -> Self {
        Self::default()
    }

    /// `translateText(options)`. Failures of the Translate call itself come back as the TS
    /// `TranslationError` object: `{ name: code, message, code, originalText, requestId? }`.
    pub async fn translate_text(
        &self,
        configs: &dyn SdkConfigSource,
        aws: &AwsSettings,
        options: &TranslateTextOptions,
    ) -> Result<TranslationResult> {
        let source_language = options.source_language.as_deref().unwrap_or("auto");
        let target_language = options.target_language.as_str();
        let text = options.text.as_str();
        let key = options
            .cache_key
            .clone()
            .unwrap_or_else(|| cache_key(text, source_language, target_language));
        if let Some(hit) = self.cached(&key) {
            tracing::debug!(cache_key = %key, "Translation cache hit");
            return Ok(hit);
        }
        if text.trim().is_empty() {
            return Err(Error::plain("Text cannot be empty"));
        }
        if text.encode_utf16().count() > MAX_TEXT_LENGTH {
            return Err(Error::plain("Text too long (max 10,000 characters)"));
        }

        let result = async {
            let conf = configs.sdk_config(aws).await?;
            let client = aws_sdk_translate::Client::new(&conf);
            let out = client
                .translate_text()
                .text(text)
                .source_language_code(source_language)
                .target_language_code(target_language)
                .settings(
                    TranslationSettings::builder()
                        .profanity(Profanity::Mask)
                        .formality(Formality::Formal)
                        .build(),
                )
                .send()
                .await
                .map_err(Error::from)?;
            if out.translated_text.is_empty() {
                return Err(Error::plain("No translation received from AWS Translate"));
            }
            Ok(TranslationResult {
                translated_text: out.translated_text,
                source_language: non_empty_or(Some(out.source_language_code), source_language),
                target_language: non_empty_or(Some(out.target_language_code), target_language),
                original_text: text.to_string(),
                applied_terminologies: out.applied_terminologies.map(|ts| {
                    ts.into_iter()
                        .filter_map(|t| t.name.filter(|n| !n.is_empty()))
                        .collect()
                }),
                applied_settings: out.applied_settings.map(|s| AppliedSettings {
                    profanity: s.profanity.map(|p| p.as_str().to_string()),
                    formality: s.formality.map(|f| f.as_str().to_string()),
                }),
            })
        }
        .await;

        match result {
            Ok(r) => {
                if let Ok(mut c) = self.cache.lock() {
                    c.insert(key, r.clone());
                }
                tracing::info!(source = %r.source_language, target = %r.target_language, "Translation completed");
                Ok(r)
            }
            Err(e) => {
                let err = translation_error(&e, text);
                tracing::error!(error = %err, source_language, target_language, "Translation failed");
                Err(err)
            }
        }
    }

    /// `translateBatch(texts)`: translates all texts concurrently and returns the successful
    /// results in input order (failures are logged and skipped).
    pub async fn translate_batch(
        &self,
        configs: &dyn SdkConfigSource,
        aws: &AwsSettings,
        texts: &[TranslateTextOptions],
    ) -> Vec<TranslationResult> {
        let tasks = texts.iter().map(|options| {
            let options = TranslateTextOptions {
                cache_key: Some(cache_key(
                    &options.text,
                    options.source_language.as_deref().unwrap_or("auto"),
                    &options.target_language,
                )),
                ..options.clone()
            };
            async move { self.translate_text(configs, aws, &options).await }
        });
        let results = join_all(tasks).await;
        let mut ok = Vec::new();
        let mut failed = 0usize;
        for r in results {
            match r {
                Ok(v) => ok.push(v),
                Err(_) => failed += 1,
            }
        }
        if failed > 0 {
            tracing::warn!(
                successful = ok.len(),
                failed,
                "Some batch translations failed"
            );
        }
        ok
    }

    /// `getCachedTranslation(text, sourceLanguage, targetLanguage)`.
    pub fn get_cached_translation(
        &self,
        text: &str,
        source_language: &str,
        target_language: &str,
    ) -> Option<TranslationResult> {
        self.cached(&cache_key(text, source_language, target_language))
    }

    /// `clearCache()`.
    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.lock() {
            *c = Cache::default();
        }
    }

    /// `getCacheStats()`.
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.lock().map(|c| c.entries.len()).unwrap_or(0),
            max_size: MAX_CACHE_SIZE,
        }
    }

    /// `healthCheck()`: translate "Hello" en → ja.
    pub async fn health_check(&self, configs: &dyn SdkConfigSource, aws: &AwsSettings) -> bool {
        let options = TranslateTextOptions {
            text: "Hello".into(),
            source_language: Some("en".into()),
            target_language: "ja".into(),
            cache_key: None,
        };
        self.translate_text(configs, aws, &options).await.is_ok()
    }

    fn cached(&self, key: &str) -> Option<TranslationResult> {
        self.cache.lock().ok().and_then(|c| c.get(key))
    }
}

fn non_empty_or(v: Option<String>, default: &str) -> String {
    v.filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Minimal `Promise.all` for same-typed futures, preserving order.
async fn join_all<F: std::future::Future>(futures: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut pending: Vec<std::pin::Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();
    let mut results: Vec<Option<F::Output>> = (0..pending.len()).map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut done = true;
        for (i, fut) in pending.iter_mut().enumerate() {
            if results[i].is_some() {
                continue;
            }
            match fut.as_mut().poll(cx) {
                std::task::Poll::Ready(v) => results[i] = Some(v),
                std::task::Poll::Pending => done = false,
            }
        }
        if done {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    })
    .await;
    results.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_hash_matches_js() {
        // Values computed with the TS simpleHash.
        assert_eq!(simple_hash(""), "0");
        assert_eq!(simple_hash("a"), "2p");
        assert_eq!(simple_hash("Hello"), "15fz5e");
        assert_eq!(simple_hash("こんにちは"), "htdio7");
        assert_eq!(
            simple_hash("The quick brown fox jumps over the lazy dog, repeatedly and at length."),
            "qux3ze"
        );
    }

    #[test]
    fn cache_key_format() {
        assert_eq!(cache_key("Hello", "auto", "ja"), "auto-ja-Hello-15fz5e");
        let long = "x".repeat(150);
        let key = cache_key(&long, "en", "ja");
        assert!(key.starts_with(&format!("en-ja-{}-", "x".repeat(100))));
    }

    #[test]
    fn cache_evicts_oldest_first() {
        let mut c = Cache::default();
        for i in 0..MAX_CACHE_SIZE {
            c.insert(format!("k{i}"), TranslationResult::default());
        }
        // Full: re-setting k0 evicts k0 itself first, then re-adds it at the end.
        c.insert("k0".into(), TranslationResult::default());
        assert_eq!(c.entries.len(), MAX_CACHE_SIZE);
        c.insert("new".into(), TranslationResult::default());
        assert_eq!(c.entries.len(), MAX_CACHE_SIZE);
        assert!(c.get("new").is_some());
        assert!(c.get("k0").is_some());
        assert!(c.get("k1").is_none());
        assert!(c.get("k2").is_some());
    }

    #[test]
    fn result_json_shape() {
        let r = TranslationResult {
            translated_text: "こんにちは".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
            original_text: "Hello".into(),
            applied_terminologies: Some(vec![]),
            applied_settings: Some(AppliedSettings {
                profanity: Some("MASK".into()),
                formality: None,
            }),
        };
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({
                "translatedText": "こんにちは", "sourceLanguage": "en", "targetLanguage": "ja",
                "originalText": "Hello", "appliedTerminologies": [],
                "appliedSettings": { "profanity": "MASK" }
            })
        );
        assert_eq!(
            serde_json::to_value(CacheStats {
                size: 1,
                max_size: 1000
            })
            .unwrap(),
            json!({ "size": 1, "maxSize": 1000 })
        );
    }

    #[test]
    fn translation_errors_carry_code_and_text() {
        let e = translation_error(
            &Error::Service(crate::error::ServiceError {
                name: "UnsupportedLanguagePairException".into(),
                message: "nope".into(),
                fault: None,
                http_status_code: Some(400),
                request_id: Some("rid".into()),
            }),
            "Hello",
        );
        assert_eq!(
            e.to_json(),
            json!({
                "name": "UnsupportedLanguagePairException", "message": "nope",
                "code": "UnsupportedLanguagePairException", "originalText": "Hello", "requestId": "rid"
            })
        );
    }
}
