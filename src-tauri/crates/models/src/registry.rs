//! Model registry types and lookups. Port of `src/common/models/models.ts`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::collation::locale_compare;

/// Fields a model allows prompt-cache points on (`CacheableField`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheableField {
    Messages,
    System,
    Tools,
}

/// `ModelProvider`; also the provider segment of a full model ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelProvider {
    Anthropic,
    Amazon,
    Deepseek,
    Stability,
    Openai,
    Moonshotai,
    Xai,
}

impl ModelProvider {
    /// The provider string as it appears in model IDs (`anthropic`, `openai`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Amazon => "amazon",
            Self::Deepseek => "deepseek",
            Self::Stability => "stability",
            Self::Openai => "openai",
            Self::Moonshotai => "moonshotai",
            Self::Xai => "xai",
        }
    }
}

/// `ModelCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelCategory {
    Text,
    Image,
}

/// `InferenceProfileType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceProfileType {
    /// Single region, no prefix.
    Base,
    /// `global.` - all commercial regions.
    Global,
    /// `us.`
    RegionalUs,
    /// `eu.`
    RegionalEu,
    /// `apac.`
    RegionalApac,
    /// `jp.` - Japan domestic only.
    Jp,
}

/// Thinking API types a model accepts (`ThinkingType` in `src/types/llm.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingType {
    Enabled,
    Adaptive,
}

/// `InferenceProfile`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceProfile {
    #[serde(rename = "type")]
    pub profile_type: InferenceProfileType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    pub regions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_suffix: Option<String>,
}

/// Dollar price per 1000 tokens (`ModelConfig['pricing']`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// `ModelConfig['cache']`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfig {
    pub supported: bool,
    pub cacheable_fields: Vec<CacheableField>,
}

/// Unified model configuration (`ModelConfig`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub base_id: String,
    pub name: String,
    pub provider: ModelProvider,
    pub category: ModelCategory,
    pub tool_use: bool,
    pub max_tokens_limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_thinking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_thinking_types: Option<Vec<ThinkingType>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_streaming_tool_use: Option<bool>,
    pub inference_profiles: Vec<InferenceProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Pricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheConfig>,
}

/// One selectable model/profile combination (`LLM` in `src/types/llm.ts`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Llm {
    pub model_id: String,
    pub model_name: String,
    pub tool_use: bool,
    pub regions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_thinking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_thinking_types: Option<Vec<ThinkingType>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_inference_profile: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_profile_arn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Entry returned by [`get_image_generation_models_for_region`] (`{ id, name }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageModel {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Data {
    pub text_models: Vec<ModelConfig>,
    pub image_models: Vec<ModelConfig>,
    /// `getModelsForRegion(region)` model IDs as computed by the TS code (test fixture).
    #[cfg_attr(not(test), allow(dead_code))]
    pub region_order: BTreeMap<String, Vec<String>>,
    /// `getImageGenerationModelsForRegion(region)` IDs as computed by the TS code (test fixture).
    #[cfg_attr(not(test), allow(dead_code))]
    pub image_region_order: BTreeMap<String, Vec<String>>,
}

pub(crate) static DATA: LazyLock<Data> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/models.json")).expect(
        "embedded data/models.json is valid (regenerate with scripts/port/gen-models-json.mjs)",
    )
});

static ALL_MODELS: LazyLock<Vec<Llm>> = LazyLock::new(generate_models_from_configs);

/// The text model registry (`MODEL_REGISTRY`), in registry order.
pub fn model_registry() -> &'static [ModelConfig] {
    &DATA.text_models
}

/// The image generation registry (`IMAGE_GENERATION_MODELS`), in registry order.
pub fn image_generation_models() -> &'static [ModelConfig] {
    &DATA.image_models
}

fn is_arn_model_id(model_id: &str) -> bool {
    model_id.starts_with("arn:aws:bedrock:")
}

/// Strips a cross-region prefix (`us.`, `eu.`, `apac.`, `jp.`, `global.`) from a model ID.
///
/// `us.anthropic.claude-3-7-sonnet-20250219-v1:0` -> `anthropic.claude-3-7-sonnet-20250219-v1:0`
pub fn get_base_model_id(model_id: &str) -> &str {
    for prefix in ["us.", "eu.", "apac.", "jp.", "global."] {
        if let Some(rest) = model_id.strip_prefix(prefix) {
            return rest;
        }
    }
    model_id
}

fn generate_full_model_id(config: &ModelConfig, profile: &InferenceProfile) -> String {
    if is_arn_model_id(&config.base_id) {
        return config.base_id.clone();
    }
    match profile.prefix.as_deref() {
        // JS truthiness: an empty prefix counts as no prefix.
        Some(prefix) if !prefix.is_empty() => {
            format!("{prefix}.{}.{}", config.provider.as_str(), config.base_id)
        }
        _ => format!("{}.{}", config.provider.as_str(), config.base_id),
    }
}

fn create_llm_from_config(config: &ModelConfig, profile: &InferenceProfile) -> Llm {
    let model_name = match profile.display_suffix.as_deref() {
        Some(suffix) if !suffix.is_empty() => format!("{} {suffix}", config.name),
        _ => config.name.clone(),
    };
    Llm {
        model_id: generate_full_model_id(config, profile),
        model_name,
        tool_use: config.tool_use,
        regions: profile.regions.clone(),
        max_tokens_limit: Some(config.max_tokens_limit),
        supports_thinking: config.supports_thinking,
        supported_thinking_types: config.supported_thinking_types.clone(),
        is_inference_profile: None,
        inference_profile_arn: None,
        description: None,
    }
}

fn generate_models_from_configs() -> Vec<Llm> {
    model_registry()
        .iter()
        .filter(|c| c.category == ModelCategory::Text)
        .flat_map(|c| {
            c.inference_profiles
                .iter()
                .map(move |p| create_llm_from_config(c, p))
        })
        .collect()
}

/// Every text model/profile combination (`allModels`), in registry order.
pub fn all_models() -> &'static [Llm] {
    &ALL_MODELS
}

/// Models whose profile covers `region`, sorted by display name (`getModelsForRegion`).
pub fn get_models_for_region(region: &str) -> Vec<Llm> {
    let mut models: Vec<Llm> = all_models()
        .iter()
        .filter(|m| m.regions.iter().any(|r| r == region))
        .cloned()
        .collect();
    models.sort_by(|a, b| locale_compare(&a.model_name, &b.model_name));
    models
}

/// Model IDs whose `supportsThinking` is `true` (`getThinkingSupportedModelIds`).
pub fn get_thinking_supported_model_ids() -> Vec<String> {
    all_models()
        .iter()
        .filter(|m| m.supports_thinking == Some(true))
        .map(|m| m.model_id.clone())
        .collect()
}

/// Thinking API types the model accepts; empty when unknown (`getSupportedThinkingTypes`).
pub fn get_supported_thinking_types(model_id: &str) -> Vec<ThinkingType> {
    get_model_config(model_id)
        .and_then(|c| c.supported_thinking_types.clone())
        .unwrap_or_default()
}

/// Image generation models offered in `region`: Amazon first, then Stability, each sorted by
/// name (`getImageGenerationModelsForRegion`). `id` is the config's `baseId`.
pub fn get_image_generation_models_for_region(region: &str) -> Vec<ImageModel> {
    let mut models: Vec<ImageModel> = image_generation_models()
        .iter()
        .filter(|c| {
            c.inference_profiles
                .iter()
                .any(|p| p.regions.iter().any(|r| r == region))
        })
        .map(|c| ImageModel {
            id: c.base_id.clone(),
            name: c.name.clone(),
        })
        .collect();
    let provider_order = |m: &ImageModel| u8::from(!m.id.starts_with("amazon"));
    models.sort_by(|a, b| {
        provider_order(a)
            .cmp(&provider_order(b))
            .then_with(|| locale_compare(&a.name, &b.name))
    });
    models
}

/// Output ceiling for a model ID, by exact then substring match against [`all_models`];
/// 8192 when unknown (`getModelMaxTokens`).
pub fn get_model_max_tokens(model_id: &str) -> u32 {
    let models = all_models();
    let model = models.iter().find(|m| m.model_id == model_id).or_else(|| {
        models
            .iter()
            .find(|m| m.model_id.contains(model_id) || model_id.contains(m.model_id.as_str()))
    });
    match model.and_then(|m| m.max_tokens_limit) {
        Some(limit) if limit != 0 => limit,
        _ => 8192,
    }
}

/// Resolves any bare, prefixed, or provider-qualified model ID to its text-registry config
/// (`getModelConfig`).
///
/// Matching is by substring, and one base ID can be a prefix of another (`claude-opus-5` of
/// `claude-opus-5-5`), so the longest matching base ID wins; ties go to the earlier registry
/// entry (JS `Array.prototype.sort` is stable).
pub fn get_model_config(model_id: &str) -> Option<&'static ModelConfig> {
    let base = get_base_model_id(model_id);
    let mut best: Option<&'static ModelConfig> = None;
    for c in model_registry() {
        let qualified = format!("{}.{}", c.provider.as_str(), c.base_id);
        let matches = base.contains(c.base_id.as_str()) || base.contains(qualified.as_str());
        if matches && best.is_none_or(|b| c.base_id.len() > b.base_id.len()) {
            best = Some(c);
        }
    }
    best
}

/// The lesser of the requested max tokens and the model's ceiling; unknown models and unset
/// requests pass through untouched (`clampMaxTokensToModelLimit`).
pub fn clamp_max_tokens_to_model_limit(model_id: &str, requested: Option<u32>) -> Option<u32> {
    let requested = requested?;
    match get_model_config(model_id).map(|c| c.max_tokens_limit) {
        Some(limit) if limit != 0 => Some(requested.min(limit)),
        _ => Some(requested),
    }
}

/// `false` only when the model explicitly opts out of streaming tool use
/// (`supportsStreamingWithToolUse`).
pub fn supports_streaming_with_tool_use(model_id: &str) -> bool {
    get_model_config(model_id).and_then(|c| c.supports_streaming_tool_use) != Some(false)
}
