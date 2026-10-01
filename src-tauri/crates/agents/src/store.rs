//! Read access to the app's config store (`src/preload/store.ts`).
//!
//! The TS services call `store.get(key)` whenever they need a value, so settings changes apply to
//! the next run. [`StoreReader::snapshot`] returns the whole store object (`store.all()` in the
//! app crate) and is called at the same points.

use serde_json::Value;

/// A source of store snapshots.
pub trait StoreReader: Send + Sync {
    /// The whole store object.
    fn snapshot(&self) -> Value;
}

impl<F> StoreReader for F
where
    F: Fn() -> Value + Send + Sync,
{
    fn snapshot(&self) -> Value {
        self()
    }
}

/// `store.get('projectPath')`, empty treated as unset.
pub fn project_path(store: &Value) -> Option<&str> {
    store
        .get("projectPath")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// `store.get('llm')?.modelId`, empty treated as unset.
pub fn default_model_id(store: &Value) -> Option<String> {
    store
        .pointer("/llm/modelId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `(store.get('agentChatConfig') || {}).enablePromptCache || false`.
pub fn prompt_cache_enabled(store: &Value) -> bool {
    tools::util::js::truthy(store.pointer("/agentChatConfig/enablePromptCache"))
}
