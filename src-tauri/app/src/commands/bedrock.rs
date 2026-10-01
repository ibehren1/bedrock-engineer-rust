//! Converse (the Express `/converse*` routes, BRIDGE.md "Converse"), the other Express routes
//! `lib/api.ts` fetched, and the `bedrock:*` IPC handlers
//! (`src/main/handlers/bedrock-handlers.ts`, plus `api.bedrock.*` methods the preload ran
//! itself). Settings are read from the store on every call.

use super::ipc_params;
use crate::backend::Backend;
use crate::errors;
use crate::settings::{aws_settings, converse_settings};
use crate::state::{store_all, StoreMutex};
use bedrock::translate::{CacheStats, TranslateTextOptions, TranslationResult};
use bedrock::{CancellationToken, ConverseRequest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, Request};
use tauri::State;

const CATEGORY: &str = "bedrock:ipc";

/// How long a `converse_cancel` for an id that isn't running yet is remembered.
const TOMBSTONE_TTL: Duration = Duration::from_secs(120);
/// Upper bound on remembered pre-cancelled ids (oldest are dropped first).
const MAX_TOMBSTONES: usize = 256;

/// Outcome of [`StreamRegistry::start`].
#[derive(Debug)]
pub enum StreamStart {
    /// Registered; the request runs under this token.
    Started(CancellationToken),
    /// `converse_cancel` arrived before the request registered: don't start it.
    Cancelled,
    /// Another request with this id is running.
    Duplicate,
}

#[derive(Default)]
struct Streams {
    running: HashMap<String, CancellationToken>,
    /// Ids cancelled before they started ("tombstones"), with when they were cancelled.
    cancelled: HashMap<String, Instant>,
}

impl Streams {
    fn prune(&mut self, now: Instant) {
        self.cancelled
            .retain(|_, at| now.duration_since(*at) < TOMBSTONE_TTL);
    }
}

/// Cancellation tokens of running `converse_stream` / `converse` requests, by request id.
///
/// Async command arguments are deserialized inside the spawned task, so a `converse_cancel`
/// (sync) sent right after the request can run before the request has registered. That cancel
/// is remembered for [`TOMBSTONE_TTL`] and the request returns `Cancelled` when it starts.
#[derive(Default)]
pub struct StreamRegistry {
    inner: Mutex<Streams>,
}

impl StreamRegistry {
    /// Register `id`, unless it was already cancelled (the tombstone is consumed) or is running.
    pub fn start(&self, id: &str) -> StreamStart {
        self.start_at(id, Instant::now())
    }

    fn start_at(&self, id: &str, now: Instant) -> StreamStart {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        s.prune(now);
        if s.cancelled.remove(id).is_some() {
            return StreamStart::Cancelled;
        }
        if s.running.contains_key(id) {
            return StreamStart::Duplicate;
        }
        let token = CancellationToken::new();
        s.running.insert(id.to_string(), token.clone());
        StreamStart::Started(token)
    }

    /// Cancel `id` if it's running; otherwise remember it so a late [`start`](Self::start) sees it.
    pub fn cancel(&self, id: &str) {
        self.cancel_at(id, Instant::now())
    }

    fn cancel_at(&self, id: &str, now: Instant) {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = s.running.remove(id) {
            token.cancel();
            return;
        }
        s.prune(now);
        if s.cancelled.len() >= MAX_TOMBSTONES && !s.cancelled.contains_key(id) {
            if let Some(oldest) = s
                .cancelled
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(k, _)| k.clone())
            {
                s.cancelled.remove(&oldest);
            }
        }
        s.cancelled.insert(id.to_string(), now);
    }

    /// Unregister `id` (the request finished).
    fn finish(&self, id: &str) {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        s.running.remove(id);
    }

    #[cfg(test)]
    fn tombstones(&self) -> usize {
        self.inner.lock().unwrap().cancelled.len()
    }
}

/// Removes the request's token when the command returns, however it returns.
struct StreamGuard<'a> {
    registry: &'a StreamRegistry,
    id: String,
}

impl Drop for StreamGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish(&self.id);
    }
}

/// Register `id` and return its token plus the guard that unregisters it, or the error to reject
/// with (`AbortError` if already cancelled, a plain error for a duplicate id).
fn register<'a>(
    registry: &'a StreamRegistry,
    id: String,
) -> Result<(CancellationToken, StreamGuard<'a>), String> {
    match registry.start(&id) {
        StreamStart::Started(token) => Ok((token, StreamGuard { registry, id })),
        StreamStart::Cancelled => Err(errors::bedrock(bedrock::Error::Cancelled)),
        StreamStart::Duplicate => Err(errors::plain(format!(
            "A Converse request with id {id} is already running"
        ))),
    }
}

/// Add `eventsSent` to a `converse_stream` error so the shim can wait for the channel events
/// sent before the failure (the invoke rejection can overtake them).
fn with_events_sent(err: String, sent: u32) -> String {
    match serde_json::from_str::<Value>(&err) {
        Ok(Value::Object(mut map)) => {
            map.insert("eventsSent".into(), json!(sent));
            Value::Object(map).to_string()
        }
        _ => json!({ "name": "Error", "message": err, "eventsSent": sent }).to_string(),
    }
}

/// `POST /converse/stream`: each `ConverseStreamOutput` member goes to `on_event`; resolves with
/// the number of events sent. Errors (before or inside the stream) reject with the error JSON
/// plus `eventsSent`; `converse_cancel` (even one that arrives before this starts) makes it
/// return promptly with `AbortError`.
#[tauri::command]
pub async fn converse_stream(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    stream_id: String,
    request: ConverseRequest,
    on_event: Channel<Value>,
) -> Result<u32, String> {
    let (token, _guard) =
        register(&backend.streams, stream_id).map_err(|e| with_events_sent(e, 0))?;
    let settings = converse_settings(&store_all(&store));
    let mut sent: u32 = 0;
    let result = backend
        .converse
        .converse_stream(&settings, &request, &token, |event| {
            on_event.send(event).map_err(|e| e.to_string())?;
            sent += 1;
            Ok(())
        })
        .await;
    match result {
        Ok(()) => Ok(sent),
        Err(e) => {
            if !matches!(e, bedrock::Error::Cancelled) {
                tracing::error!(category = CATEGORY, model_id = %request.model_id, error = %e, "Converse stream failed");
            }
            Err(with_events_sent(errors::bedrock(e), sent))
        }
    }
}

/// Stop a running `converse_stream` / `converse` by id. An id that isn't running yet is
/// remembered for a while, so a cancel that overtakes its request still stops it.
#[tauri::command]
pub fn converse_cancel(backend: State<'_, Backend>, stream_id: String) {
    backend.streams.cancel(&stream_id);
}

/// `POST /converse` → `ConverseCommandOutput` JSON. With `request_id`, `converse_cancel` with that
/// id aborts it (`AbortError`).
#[tauri::command]
pub async fn converse(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: ConverseRequest,
    request_id: Option<String>,
) -> Result<Value, String> {
    let registered = request_id
        .map(|id| register(&backend.streams, id))
        .transpose()?;
    let token = registered
        .as_ref()
        .map(|(t, _)| t.clone())
        .unwrap_or_default();
    let settings = converse_settings(&store_all(&store));
    backend
        .converse
        .converse(&settings, &request, &token)
        .await
        .map_err(errors::bedrock)
}

/// `POST /retrieveAndGenerate`.
#[tauri::command]
pub async fn retrieve_and_generate(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Value,
) -> Result<Value, String> {
    let aws = aws_settings(&store_all(&store));
    bedrock::agent::retrieve_and_generate(backend.sdk.as_ref(), &aws, &request)
        .await
        .map_err(errors::bedrock)
}

/// `GET /listModels`.
#[tauri::command]
pub fn list_models(store: State<'_, StoreMutex>) -> Vec<models::Llm> {
    bedrock::catalog::list_models(&aws_settings(&store_all(&store)))
}

/// `GET /listAgentTags` (never existed in Express; `[]`).
#[tauri::command]
pub fn list_agent_tags() -> Vec<String> {
    bedrock::catalog::list_agent_tags()
}

/// `POST /structured-output`.
#[tauri::command]
pub async fn get_structured_output(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: bedrock::structured_output::StructuredOutputRequest,
) -> Result<Value, String> {
    let settings = converse_settings(&store_all(&store));
    bedrock::structured_output::get_structured_output(&backend.converse, &settings, &request)
        .await
        .map_err(errors::bedrock)
}

/// `POST /website-recommendations`.
#[tauri::command]
pub async fn get_website_recommendations(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: bedrock::structured_output::WebsiteRecommendationsRequest,
) -> Result<Value, String> {
    let settings = converse_settings(&store_all(&store));
    bedrock::structured_output::get_website_recommendations(&backend.converse, &settings, &request)
        .await
        .map_err(errors::bedrock)
}

/// `api.bedrock.applyGuardrail(request)`.
#[tauri::command]
pub async fn bedrock_apply_guardrail(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Value,
) -> Result<Value, String> {
    let aws = aws_settings(&store_all(&store));
    bedrock::guardrail::apply_guardrail(backend.sdk.as_ref(), &aws, &request)
        .await
        .map_err(errors::bedrock)
}

/// `api.bedrock.listApplicationInferenceProfiles()` (failures are logged and yield `[]`).
#[tauri::command]
pub async fn bedrock_list_application_inference_profiles(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
) -> Result<Vec<bedrock::inference_profile::ApplicationInferenceProfile>, String> {
    let aws = aws_settings(&store_all(&store));
    Ok(
        bedrock::inference_profile::list_application_inference_profiles(backend.sdk.as_ref(), &aws)
            .await,
    )
}

/// `bedrock:translateText`.
#[tauri::command]
pub async fn bedrock_translate_text(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    text: String,
    source_language: Option<String>,
    target_language: String,
    cache_key: Option<String>,
) -> Result<TranslationResult, String> {
    let aws = aws_settings(&store_all(&store));
    let options = TranslateTextOptions {
        source_language,
        target_language,
        text,
        cache_key,
    };
    backend
        .translate
        .translate_text(backend.sdk.as_ref(), &aws, &options)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:translateBatch` (failed items are logged and left out).
#[tauri::command]
pub async fn bedrock_translate_batch(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    texts: Vec<TranslateTextOptions>,
) -> Result<Vec<TranslationResult>, String> {
    let aws = aws_settings(&store_all(&store));
    Ok(backend
        .translate
        .translate_batch(backend.sdk.as_ref(), &aws, &texts)
        .await)
}

/// `bedrock:getTranslationCache`.
#[tauri::command]
pub fn bedrock_get_translation_cache(
    backend: State<'_, Backend>,
    text: String,
    source_language: String,
    target_language: String,
) -> Option<TranslationResult> {
    backend
        .translate
        .get_cached_translation(&text, &source_language, &target_language)
}

/// `bedrock:clearTranslationCache` → `{ success: true }`.
#[tauri::command]
pub fn bedrock_clear_translation_cache(backend: State<'_, Backend>) -> Value {
    backend.translate.clear_cache();
    tracing::info!(category = CATEGORY, "Translation cache cleared");
    json!({ "success": true })
}

/// `bedrock:getTranslationCacheStats`.
#[tauri::command]
pub fn bedrock_get_translation_cache_stats(backend: State<'_, Backend>) -> CacheStats {
    backend.translate.cache_stats()
}

/// `bedrock:getModelMaxTokens` → `{ maxTokens }`.
#[tauri::command]
pub fn bedrock_get_model_max_tokens(model_id: String) -> Value {
    bedrock::catalog::get_model_max_tokens(&model_id)
}

// The handlers below were only reached by the preload tools (which run in Rust now); they are
// registered so `window.ipc.invoke('bedrock:…')` keeps working. Each reads the handler's params
// object from the whole argument object (see `ipc_params`).

/// `bedrock:generateImage`.
#[tauri::command]
pub async fn bedrock_generate_image(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<bedrock::image::GeneratedImage, String> {
    let req: bedrock::image::GenerateImageRequest =
        serde_json::from_value(ipc_params(&request)).map_err(errors::plain)?;
    let aws = aws_settings(&store_all(&store));
    bedrock::image::generate_image(backend.sdk.as_ref(), &aws, &req)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:recognizeImage` (`{ imagePaths, prompt?, modelId? }`; only the first image).
#[tauri::command]
pub async fn bedrock_recognize_image(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<String, String> {
    let req = bedrock::image_recognition::RecognizeImageRequest::from_ipc(&ipc_params(&request))
        .map_err(errors::bedrock)?;
    let settings = converse_settings(&store_all(&store));
    bedrock::image_recognition::recognize_image(
        backend.sdk.as_ref(),
        &backend.converse,
        &settings,
        &req,
    )
    .await
    .map_err(errors::bedrock)
}

/// `bedrock:retrieve` (`{ knowledgeBaseId, query, retrievalConfiguration? }`).
#[tauri::command]
pub async fn bedrock_retrieve(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<Value, String> {
    let input = bedrock::agent::retrieve_input_from_ipc(&ipc_params(&request));
    let aws = aws_settings(&store_all(&store));
    bedrock::agent::retrieve(backend.sdk.as_ref(), &aws, &input)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:invokeAgent`.
#[tauri::command]
pub async fn bedrock_invoke_agent(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<bedrock::agent::InvokeAgentResult, String> {
    let params = ipc_params(&request);
    let aws = aws_settings(&store_all(&store));
    bedrock::agent::invoke_agent(backend.sdk.as_ref(), &aws, &params)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:invokeFlow`.
#[tauri::command]
pub async fn bedrock_invoke_flow(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<Value, String> {
    let input = bedrock::flow::invoke_flow_input_from_ipc(&ipc_params(&request));
    let aws = aws_settings(&store_all(&store));
    bedrock::flow::invoke_flow(backend.sdk.as_ref(), &aws, &input)
        .await
        .map_err(errors::bedrock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_before_start_is_remembered_and_consumed() {
        let r = StreamRegistry::default();
        r.cancel("a");
        assert!(matches!(r.start("a"), StreamStart::Cancelled));
        // The tombstone is used up: a later request with the id (never happens) would run.
        assert_eq!(r.tombstones(), 0);
    }

    #[test]
    fn cancel_while_running_cancels_the_token() {
        let r = StreamRegistry::default();
        let StreamStart::Started(token) = r.start("a") else {
            panic!("not started")
        };
        r.cancel("a");
        assert!(token.is_cancelled());
        assert_eq!(r.tombstones(), 0);
    }

    #[test]
    fn duplicate_ids_are_rejected_without_touching_the_running_request() {
        let r = StreamRegistry::default();
        let StreamStart::Started(token) = r.start("a") else {
            panic!("not started")
        };
        let err = register(&r, "a".into()).err().expect("duplicate accepted");
        assert!(err.contains("already running"));
        r.cancel("a");
        assert!(token.is_cancelled(), "the original is still registered");
    }

    #[test]
    fn guard_unregisters_on_drop() {
        let r = StreamRegistry::default();
        {
            let (_token, _guard) = register(&r, "a".into()).unwrap();
        }
        assert!(matches!(r.start("a"), StreamStart::Started(_)));
    }

    #[test]
    fn pre_cancelled_register_is_abort_error() {
        let r = StreamRegistry::default();
        r.cancel("a");
        let err = register(&r, "a".into()).err().expect("started");
        assert_eq!(errors::parse(&err).0, "AbortError");
    }

    #[test]
    fn tombstones_expire() {
        let r = StreamRegistry::default();
        let t0 = Instant::now();
        r.cancel_at("a", t0);
        assert!(matches!(
            r.start_at("a", t0 + TOMBSTONE_TTL + Duration::from_secs(1)),
            StreamStart::Started(_)
        ));
    }

    #[test]
    fn tombstones_are_bounded_dropping_the_oldest() {
        let r = StreamRegistry::default();
        let t0 = Instant::now();
        for i in 0..MAX_TOMBSTONES + 10 {
            r.cancel_at(&format!("id{i}"), t0 + Duration::from_millis(i as u64));
        }
        assert_eq!(r.tombstones(), MAX_TOMBSTONES);
        let late = t0 + Duration::from_millis((MAX_TOMBSTONES + 10) as u64);
        assert!(matches!(r.start_at("id0", late), StreamStart::Started(_)));
        let newest = format!("id{}", MAX_TOMBSTONES + 9);
        assert!(matches!(r.start_at(&newest, late), StreamStart::Cancelled));
    }

    #[test]
    fn events_sent_is_added_to_error_json() {
        let err = errors::bedrock(bedrock::Error::Service(bedrock::ServiceError::new(
            "ModelStreamErrorException",
            "broke",
        )));
        let (name, message, v) = errors::parse(&with_events_sent(err, 7));
        assert_eq!(name, "ModelStreamErrorException");
        assert_eq!(message, "broke");
        assert_eq!(v["eventsSent"], 7);

        let (name, message, v) = errors::parse(&with_events_sent("plain".into(), 2));
        assert_eq!((name.as_str(), message.as_str()), ("Error", "plain"));
        assert_eq!(v["eventsSent"], 2);
    }
}
