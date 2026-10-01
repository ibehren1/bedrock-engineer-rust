//! Test-for-test port of `src/common/models/__tests__/registry.test.ts`.
//! Each `describe` block is a module; `test.each` tables are loops over the same rows.

use crate::*;

fn ids() -> Vec<&'static str> {
    all_models().iter().map(|m| m.model_id.as_str()).collect()
}

fn find(model_id: &str) -> Option<&'static Llm> {
    all_models().iter().find(|m| m.model_id == model_id)
}

fn region_ids(region: &str) -> Vec<String> {
    get_models_for_region(region)
        .into_iter()
        .map(|m| m.model_id)
        .collect()
}

fn name_of(model_id: &str) -> Option<&'static str> {
    get_model_config(model_id).map(|c| c.name.as_str())
}

/// Jest `toBeCloseTo(expected, 5)`: |actual - expected| < 10^-5 / 2.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.5e-5,
        "expected {actual} to be close to {expected}"
    );
}

fn has(ids: &[impl AsRef<str>], id: &str) -> bool {
    ids.iter().any(|i| i.as_ref() == id)
}

/// describe('OpenAI GPT model registry integration')
mod openai_gpt {
    use super::*;

    const OPENAI_GPT_MODELS: [(&str, &str); 6] = [
        ("gpt-6-astra", "GPT-6 Astra"),
        ("gpt-6-sol", "GPT-6 Sol"),
        ("gpt-6-luna", "GPT-6 Luna"),
        ("gpt-5.6-sol", "GPT-5.6 Sol"),
        ("gpt-5.6-terra", "GPT-5.6 Terra"),
        ("gpt-5.6-luna", "GPT-5.6 Luna"),
    ];

    #[test]
    fn exposed_as_global_and_us_profiles_no_bare_id() {
        let ids = ids();
        for (base, name) in OPENAI_GPT_MODELS {
            assert!(has(&ids, &format!("global.openai.{base}")), "{base}");
            assert!(has(&ids, &format!("us.openai.{base}")), "{base}");
            assert!(!has(&ids, &format!("openai.{base}")), "{base}");

            let us = find(&format!("us.openai.{base}")).unwrap();
            assert_eq!(us.model_name, format!("{name} (US)"));
            assert!(us.tool_use);
            assert_eq!(us.supports_thinking, Some(true));
        }
    }

    #[test]
    fn gpt_5_5_and_5_4_not_registered() {
        let ids = ids();
        assert!(!ids.iter().any(|id| id.contains("gpt-5.5")));
        assert!(!ids.iter().any(|id| id.contains("gpt-5.4")));
    }

    #[test]
    fn available_in_the_three_us_regions() {
        for region in ["us-east-1", "us-east-2", "us-west-2"] {
            let ids = region_ids(region);
            for (base, _) in OPENAI_GPT_MODELS {
                assert!(has(&ids, &format!("us.openai.{base}")), "{region} {base}");
                assert!(
                    has(&ids, &format!("global.openai.{base}")),
                    "{region} {base}"
                );
            }
        }
    }

    #[test]
    fn get_model_config_resolves_prefixed_ids() {
        for (base, name) in OPENAI_GPT_MODELS {
            assert_eq!(name_of(&format!("us.openai.{base}")), Some(name));
            assert_eq!(name_of(&format!("global.openai.{base}")), Some(name));
        }
    }

    #[test]
    fn every_model_has_pricing() {
        for (base, _) in OPENAI_GPT_MODELS {
            let pricing = get_model_config(&format!("us.openai.{base}"))
                .and_then(|c| c.pricing)
                .unwrap_or_else(|| panic!("{base} has no pricing"));
            assert!(pricing.input > 0.0);
            assert!(pricing.output > 0.0);
        }
    }

    #[test]
    fn pricing_gpt_5_6_sol() {
        let calc = PricingCalculator::new("us.openai.gpt-5.6-sol");
        assert_close(calc.calculate_input_cost(1_000_000), 5.5);
        assert_close(calc.calculate_output_cost(1_000_000), 33.0);
    }

    #[test]
    fn pricing_gpt_6_astra() {
        let calc = PricingCalculator::new("us.openai.gpt-6-astra");
        assert_close(calc.calculate_input_cost(1_000_000), 11.0);
        assert_close(calc.calculate_output_cost(1_000_000), 55.0);
    }

    #[test]
    fn pricing_gpt_6_sol() {
        let calc = PricingCalculator::new("us.openai.gpt-6-sol");
        assert_close(calc.calculate_input_cost(1_000_000), 2.0);
        assert_close(calc.calculate_output_cost(1_000_000), 10.0);
    }

    #[test]
    fn pricing_gpt_6_luna() {
        let calc = PricingCalculator::new("us.openai.gpt-6-luna");
        assert_close(calc.calculate_input_cost(1_000_000), 0.1);
        assert_close(calc.calculate_output_cost(1_000_000), 0.5);
    }

    #[test]
    fn shared_suffix_tiers_resolve_distinctly() {
        assert_eq!(name_of("us.openai.gpt-6-sol"), Some("GPT-6 Sol"));
        assert_eq!(name_of("us.openai.gpt-5.6-sol"), Some("GPT-5.6 Sol"));
        assert_eq!(name_of("us.openai.gpt-6-luna"), Some("GPT-6 Luna"));
        assert_eq!(name_of("us.openai.gpt-5.6-luna"), Some("GPT-5.6 Luna"));
        let input = |id| {
            get_model_config(id)
                .and_then(|c| c.pricing)
                .map(|p| p.input)
        };
        assert_eq!(input("us.openai.gpt-6-sol"), Some(0.002));
        assert_eq!(input("us.openai.gpt-6-luna"), Some(0.0001));
    }

    #[test]
    fn gpt_6_astra_allows_128k_output() {
        assert_eq!(
            get_model_config("us.openai.gpt-6-astra").map(|c| c.max_tokens_limit),
            Some(128000)
        );
    }
}

/// describe('OpenAI GPT-6.1 Sol registry integration')
mod openai_gpt_6_1_sol {
    use super::*;

    #[test]
    fn us_profile_only() {
        let ids = ids();
        assert!(has(&ids, "us.openai.gpt-6.1-sol"));
        assert!(!has(&ids, "global.openai.gpt-6.1-sol"));
        assert!(!has(&ids, "openai.gpt-6.1-sol"));
    }

    #[test]
    fn display_name_and_capabilities() {
        let m = find("us.openai.gpt-6.1-sol").unwrap();
        assert_eq!(m.model_name, "GPT-6.1 Sol (US)");
        assert!(m.tool_use);
        assert_eq!(m.supports_thinking, Some(true));
        assert_eq!(m.max_tokens_limit, Some(131072));
    }

    #[test]
    fn offered_in_us_regions_only() {
        for region in ["us-east-1", "us-east-2", "us-west-2"] {
            assert!(
                has(&region_ids(region), "us.openai.gpt-6.1-sol"),
                "{region}"
            );
        }
        for region in ["eu-west-1", "ap-northeast-1"] {
            assert!(
                !has(&region_ids(region), "us.openai.gpt-6.1-sol"),
                "{region}"
            );
        }
    }

    #[test]
    fn does_not_resolve_to_gpt_6_sol() {
        assert_eq!(name_of("us.openai.gpt-6.1-sol"), Some("GPT-6.1 Sol"));
        assert_eq!(name_of("us.openai.gpt-6-sol"), Some("GPT-6 Sol"));
    }

    #[test]
    fn no_explicit_prompt_cache() {
        assert_eq!(
            get_model_config("us.openai.gpt-6.1-sol").unwrap().cache,
            None
        );
    }

    #[test]
    fn pricing_us_cris_short_context() {
        let calc = PricingCalculator::new("us.openai.gpt-6.1-sol");
        assert_close(calc.calculate_input_cost(1_000_000), 2.2);
        assert_close(calc.calculate_output_cost(1_000_000), 11.0);
        assert_close(calc.calculate_cache_read_cost(1_000_000), 0.11);
        assert_close(calc.calculate_cache_write_cost(1_000_000), 2.75);
    }
}

/// Shared shape of the Opus 5.5 / Sonnet 5.5 describe blocks.
fn assert_geo_profiles_only(base: &str) {
    let ids = ids();
    for prefix in ["global", "us", "eu", "jp"] {
        assert!(
            has(&ids, &format!("{prefix}.anthropic.{base}")),
            "{prefix} {base}"
        );
    }
    assert!(!has(&ids, &format!("anthropic.{base}")));
}

fn assert_adaptive_capabilities(model_id: &str, display: &str) {
    let m = find(model_id).unwrap();
    assert_eq!(m.model_name, display);
    assert!(m.tool_use);
    assert_eq!(m.supports_thinking, Some(true));
    assert_eq!(
        m.supported_thinking_types,
        Some(vec![ThinkingType::Adaptive])
    );
    assert_eq!(m.max_tokens_limit, Some(128000));
}

fn assert_full_cache(model_id: &str) {
    let cache = get_model_config(model_id).unwrap().cache.clone().unwrap();
    assert!(cache.supported);
    assert_eq!(
        cache.cacheable_fields,
        vec![
            CacheableField::Messages,
            CacheableField::System,
            CacheableField::Tools
        ]
    );
}

/// describe('Claude Opus 5.5 registry integration')
mod claude_opus_5_5 {
    use super::*;

    #[test]
    fn geo_profiles_no_bare_id() {
        assert_geo_profiles_only("claude-opus-5-5");
    }

    #[test]
    fn display_names_and_capabilities() {
        assert_adaptive_capabilities("us.anthropic.claude-opus-5-5", "Claude Opus 5.5 (US)");
    }

    #[test]
    fn pricing_per_1k() {
        let calc = PricingCalculator::new("us.anthropic.claude-opus-5-5");
        assert_close(calc.calculate_input_cost(1_000_000), 4.0);
        assert_close(calc.calculate_output_cost(1_000_000), 20.0);
        assert_close(calc.calculate_cache_read_cost(1_000_000), 0.4);
    }

    #[test]
    fn prompt_caching_declared() {
        assert_full_cache("us.anthropic.claude-opus-5-5");
    }

    #[test]
    fn not_confused_with_opus_5() {
        assert_eq!(
            name_of("us.anthropic.claude-opus-5-5"),
            Some("Claude Opus 5.5")
        );
        assert_eq!(
            name_of("global.anthropic.claude-opus-5-5"),
            Some("Claude Opus 5.5")
        );
        assert_eq!(name_of("us.anthropic.claude-opus-5"), Some("Claude Opus 5"));
        assert_eq!(
            name_of("us.anthropic.claude-fable-5-1"),
            Some("Claude Fable 5.1")
        );
        assert_eq!(
            name_of("global.anthropic.claude-fable-5"),
            Some("Claude Fable 5")
        );
    }
}

/// describe('Claude Sonnet 5.5 registry integration')
mod claude_sonnet_5_5 {
    use super::*;

    #[test]
    fn geo_profiles_no_bare_id() {
        assert_geo_profiles_only("claude-sonnet-5-5");
    }

    #[test]
    fn display_names_and_capabilities() {
        assert_adaptive_capabilities("us.anthropic.claude-sonnet-5-5", "Claude Sonnet 5.5 (US)");
    }

    #[test]
    fn pricing_matches_sonnet_5() {
        let calc = PricingCalculator::new("us.anthropic.claude-sonnet-5-5");
        assert_close(calc.calculate_input_cost(1_000_000), 2.0);
        assert_close(calc.calculate_output_cost(1_000_000), 10.0);
        assert_close(calc.calculate_cache_read_cost(1_000_000), 0.2);
    }

    #[test]
    fn prompt_caching_declared() {
        assert_full_cache("us.anthropic.claude-sonnet-5-5");
    }

    #[test]
    fn not_confused_with_sonnet_5() {
        assert_eq!(
            name_of("us.anthropic.claude-sonnet-5-5"),
            Some("Claude Sonnet 5.5")
        );
        assert_eq!(
            name_of("global.anthropic.claude-sonnet-5-5"),
            Some("Claude Sonnet 5.5")
        );
        assert_eq!(
            name_of("us.anthropic.claude-sonnet-5"),
            Some("Claude Sonnet 5")
        );
    }
}

/// describe('xAI Grok model registry integration')
mod xai_grok {
    use super::*;

    #[test]
    fn exposed_as_global_and_us_profiles_no_bare_id() {
        let ids = ids();
        for base in ["grok-4.6", "grok-4.7"] {
            assert!(has(&ids, &format!("global.xai.{base}")));
            assert!(has(&ids, &format!("us.xai.{base}")));
            assert!(!has(&ids, &format!("xai.{base}")));
        }
    }

    #[test]
    fn display_names_and_capabilities() {
        for (base, name) in [("grok-4.6", "Grok 4.6"), ("grok-4.7", "Grok 4.7")] {
            let us = find(&format!("us.xai.{base}")).unwrap();
            assert_eq!(us.model_name, format!("{name} (US)"));
            assert!(us.tool_use);
            assert_eq!(us.supports_thinking, Some(true));
            assert_eq!(us.max_tokens_limit, Some(32768));

            let global = find(&format!("global.xai.{base}")).unwrap();
            assert_eq!(global.model_name, format!("{name} (Global)"));
        }
    }

    #[test]
    fn grok_4_7_and_opus_4_7_never_resolve_to_each_other() {
        for grok in ["us.xai.grok-4.7", "global.xai.grok-4.7", "xai.grok-4.7"] {
            let c = get_model_config(grok).unwrap();
            assert_eq!(c.name, "Grok 4.7");
            assert_eq!(c.provider, ModelProvider::Xai);
            assert_eq!(c.max_tokens_limit, 32768);
            assert_eq!(
                c.supported_thinking_types,
                Some(vec![ThinkingType::Enabled])
            );
        }
        for opus in [
            "us.anthropic.claude-opus-4-7",
            "global.anthropic.claude-opus-4-7",
            "jp.anthropic.claude-opus-4-7",
        ] {
            let c = get_model_config(opus).unwrap();
            assert_eq!(c.name, "Claude Opus 4.7");
            assert_eq!(c.provider, ModelProvider::Anthropic);
            assert_eq!(
                c.supported_thinking_types,
                Some(vec![ThinkingType::Adaptive])
            );
        }
    }

    #[test]
    fn grok_4_3_not_registered() {
        assert!(!ids().iter().any(|id| id.contains("grok-4.3")));
    }

    #[test]
    fn get_model_config_resolves_prefixed_ids() {
        assert_eq!(name_of("us.xai.grok-4.6"), Some("Grok 4.6"));
        assert_eq!(name_of("global.xai.grok-4.6"), Some("Grok 4.6"));
        assert_eq!(name_of("us.xai.grok-4.7"), Some("Grok 4.7"));
        assert_eq!(name_of("global.xai.grok-4.7"), Some("Grok 4.7"));
    }

    #[test]
    fn no_explicit_prompt_cache() {
        assert_eq!(get_model_config("us.xai.grok-4.6").unwrap().cache, None);
        assert_eq!(get_model_config("us.xai.grok-4.7").unwrap().cache, None);
    }

    #[test]
    fn only_global_profile_outside_us() {
        for base in ["grok-4.6", "grok-4.7"] {
            let eu = region_ids("eu-west-1");
            assert!(has(&eu, &format!("global.xai.{base}")));
            assert!(!has(&eu, &format!("us.xai.{base}")));

            let us = region_ids("us-east-1");
            assert!(has(&us, &format!("global.xai.{base}")));
            assert!(has(&us, &format!("us.xai.{base}")));
        }
    }

    #[test]
    fn grok_4_7_in_jakarta_and_melbourne_but_not_4_6() {
        for region in ["ap-southeast-3", "ap-southeast-4"] {
            let ids = region_ids(region);
            assert!(has(&ids, "global.xai.grok-4.7"), "{region}");
            assert!(!has(&ids, "global.xai.grok-4.6"), "{region}");
        }
    }

    #[test]
    fn grok_4_6_pricing_geo_rate() {
        let calc = PricingCalculator::new("us.xai.grok-4.6");
        assert_close(calc.calculate_input_cost(1_000_000), 2.2);
        assert_close(calc.calculate_output_cost(1_000_000), 6.6);
        assert_close(calc.calculate_cache_read_cost(1_000_000), 0.55);
    }

    #[test]
    fn grok_4_7_pricing_global_rate() {
        for id in ["us.xai.grok-4.7", "global.xai.grok-4.7"] {
            let calc = PricingCalculator::new(id);
            assert_close(calc.calculate_input_cost(1_000_000), 2.0);
            assert_close(calc.calculate_output_cost(1_000_000), 6.0);
            assert_close(calc.calculate_cache_read_cost(1_000_000), 0.5);
        }
    }
}

/// describe('clampMaxTokensToModelLimit')
mod clamp_max_tokens {
    use super::*;

    #[test]
    fn lowers_request_over_ceiling() {
        assert_eq!(
            clamp_max_tokens_to_model_limit(
                "us.anthropic.claude-haiku-4-5-20251001-v1:0",
                Some(128000)
            ),
            Some(64000)
        );
        assert_eq!(
            clamp_max_tokens_to_model_limit("us.amazon.nova-lite-v1:0", Some(128000)),
            Some(5120)
        );
    }

    #[test]
    fn leaves_request_at_or_below_ceiling() {
        let clamp = clamp_max_tokens_to_model_limit;
        assert_eq!(
            clamp("us.anthropic.claude-opus-5", Some(128000)),
            Some(128000)
        );
        assert_eq!(clamp("us.anthropic.claude-opus-5", Some(4096)), Some(4096));
        assert_eq!(clamp("us.openai.gpt-6-astra", Some(8192)), Some(8192));
    }

    #[test]
    fn passes_through_unknown_models() {
        assert_eq!(
            clamp_max_tokens_to_model_limit(
                "arn:aws:bedrock:us-east-1:123456789012:imported-model/abcdefg",
                Some(200000)
            ),
            Some(200000)
        );
    }

    #[test]
    fn passes_through_unset_max_tokens() {
        assert_eq!(
            clamp_max_tokens_to_model_limit("us.anthropic.claude-opus-5", None),
            None
        );
    }

    #[test]
    fn every_text_model_declares_a_ceiling() {
        let missing: Vec<&str> = all_models()
            .iter()
            .filter(|m| m.max_tokens_limit.unwrap_or(0) == 0)
            .map(|m| m.model_id.as_str())
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }
}

/// describe('model output ceilings')
#[test]
fn model_output_ceilings() {
    let cases = [
        ("us.anthropic.claude-haiku-4-5-20251001-v1:0", 64000),
        ("us.anthropic.claude-sonnet-4-5-20250929-v1:0", 64000),
        ("us.anthropic.claude-opus-4-1-20250805-v1:0", 32000),
        ("us.anthropic.claude-opus-5", 128000),
        ("us.anthropic.claude-opus-5-5", 128000),
        ("us.anthropic.claude-sonnet-5", 128000),
        ("us.anthropic.claude-sonnet-5-5", 128000),
        ("us.amazon.nova-premier-v1:0", 32000),
        ("us.amazon.nova-pro-v1:0", 5120),
        ("us.amazon.nova-2-lite-v1:0", 64000),
        ("us.deepseek.r1-v1:0", 32768),
        ("us.openai.gpt-6-astra", 128000),
        ("us.openai.gpt-6.1-sol", 131072),
        ("us.openai.gpt-6-sol", 128000),
        ("us.openai.gpt-6-luna", 128000),
        ("us.openai.gpt-5.6-sol", 128000),
        ("openai.gpt-oss-120b-1:0", 16384),
        ("openai.gpt-oss-20b-1:0", 16384),
        ("moonshotai.kimi-k2.5", 16384),
        ("us.moonshotai.kimi-k3", 16384),
        ("us.xai.grok-4.6", 32768),
        ("us.xai.grok-4.7", 32768),
    ];
    for (id, expected) in cases {
        assert_eq!(
            get_model_config(id).map(|c| c.max_tokens_limit),
            Some(expected),
            "{id}"
        );
    }
}

// ---- Rust-port-specific checks (not in the TS file) ----

/// `getModelsForRegion` / `getImageGenerationModelsForRegion` order matches what the TS code
/// produced with `localeCompare` (fixtures emitted by the generator).
#[test]
fn region_order_matches_ts_fixture() {
    let data = &*crate::registry::DATA;
    assert!(!data.region_order.is_empty());
    for (region, expected) in &data.region_order {
        assert_eq!(&region_ids(region), expected, "text models in {region}");
    }
    for (region, expected) in &data.image_region_order {
        let got: Vec<String> = get_image_generation_models_for_region(region)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(&got, expected, "image models in {region}");
    }
}

/// Serialized shapes use the TS camelCase field names and omit unset optionals.
#[test]
fn serializes_in_ts_json_shape() {
    let llm = serde_json::to_value(find("us.anthropic.claude-opus-5-5").unwrap()).unwrap();
    assert_eq!(llm["modelId"], "us.anthropic.claude-opus-5-5");
    assert_eq!(llm["modelName"], "Claude Opus 5.5 (US)");
    assert_eq!(llm["toolUse"], true);
    assert_eq!(llm["maxTokensLimit"], 128000);
    assert_eq!(
        llm["supportedThinkingTypes"],
        serde_json::json!(["adaptive"])
    );
    assert!(llm["regions"].is_array());
    assert!(llm.get("isInferenceProfile").is_none());

    let config =
        serde_json::to_value(get_model_config("us.anthropic.claude-opus-5-5").unwrap()).unwrap();
    assert_eq!(config["baseId"], "claude-opus-5-5");
    assert_eq!(config["cache"]["cacheableFields"][0], "messages");
    assert!(config["pricing"]["cacheRead"].is_number());
    let profile_types: Vec<&str> = config["inferenceProfiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["type"].as_str().unwrap())
        .collect();
    assert!(profile_types.contains(&"regional-us"));

    // Round-trip of the whole embedded registry is lossless against the source JSON
    // (numbers compared as f64: JS has no int/float split, so `0` and `0.0` are the same value).
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../data/models.json")).unwrap();
    assert_eq!(
        normalize(serde_json::to_value(model_registry()).unwrap()),
        normalize(source["textModels"].clone())
    );
    assert_eq!(
        normalize(serde_json::to_value(image_generation_models()).unwrap()),
        normalize(source["imageModels"].clone())
    );
}

#[test]
fn helper_lookups() {
    assert_eq!(
        get_base_model_id("global.openai.gpt-6-sol"),
        "openai.gpt-6-sol"
    );
    assert_eq!(
        get_base_model_id("apac.amazon.nova-pro-v1:0"),
        "amazon.nova-pro-v1:0"
    );
    assert_eq!(get_base_model_id("openai.gpt-6-sol"), "openai.gpt-6-sol");

    assert_eq!(get_model_max_tokens("us.anthropic.claude-opus-5"), 128000);
    assert_eq!(get_model_max_tokens("totally-unknown"), 8192);

    assert_eq!(
        get_supported_thinking_types("us.xai.grok-4.7"),
        vec![ThinkingType::Enabled]
    );
    assert!(get_supported_thinking_types("unknown").is_empty());
    assert!(get_thinking_supported_model_ids().contains(&"us.openai.gpt-6-sol".to_string()));
    assert!(supports_streaming_with_tool_use(
        "us.anthropic.claude-opus-5"
    ));
    assert!(supports_streaming_with_tool_use("unknown"));

    let images = get_image_generation_models_for_region("us-west-2");
    assert!(images.first().unwrap().id.starts_with("amazon"));
    assert!(images.last().unwrap().id.starts_with("stability"));
}

fn normalize(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.into_iter().map(normalize).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        other => other,
    }
}
