//! Prices a usage event with the model catalog at the time it is recorded.

use bedrock::usage::{UnitKind, UsageEvent, UsageService};
use models::{
    get_base_model_id, get_embedding_pricing, get_model_config, get_retired_pricing,
    get_unit_pricing, PricedUnit, Pricing, PricingRules,
};
use serde_json::{json, Value};

/// The model and cost an event is stored with.
#[derive(Debug, Clone, PartialEq)]
pub struct Priced {
    /// The base model id (cross-region prefix removed; the router's invoked model if known).
    pub model_id: String,
    /// The catalog's display name, or the id when the catalog doesn't know the model.
    pub model_name: String,
    /// `None` when the model has no price (unknown model, application inference profile ARN).
    pub cost_usd: Option<f64>,
    /// The per-1000-token rates used, so old rows keep their price when the catalog changes.
    pub snapshot: Option<Value>,
}

fn unit_matches(unit: PricedUnit, kind: UnitKind) -> bool {
    matches!(
        (unit, kind),
        (PricedUnit::Image, UnitKind::Images)
            | (PricedUnit::VideoSecond, UnitKind::VideoSeconds)
            | (PricedUnit::Character, UnitKind::Characters)
            | (PricedUnit::GuardrailTextUnit, UnitKind::GuardrailTextUnits)
    )
}

/// The cross-region profile prefix of a model id (`global`, `us`, ...), or `base`.
fn profile_prefix(model_id: &str) -> &str {
    match model_id.split_once('.') {
        Some((p @ ("global" | "us" | "eu" | "apac" | "jp"), _)) => p,
        _ => "base",
    }
}

/// The rates this request actually billed at: the profile's multiplier, and the long-context
/// multiples when the prompt is over the threshold.
fn apply_rules(
    p: Pricing,
    rules: Option<&PricingRules>,
    raw_model_id: &str,
    event: &UsageEvent,
) -> Pricing {
    let Some(rules) = rules else {
        return p;
    };
    let m = rules
        .profile_multiplier
        .get(profile_prefix(raw_model_id))
        .copied()
        .unwrap_or(1.0);
    let prompt = event.input_tokens + event.cache_read_tokens + event.cache_write_tokens;
    let long = rules
        .long_context
        .as_ref()
        .filter(|l| prompt > l.threshold_tokens);
    let (i, o, cr, cw) = long.map_or((1.0, 1.0, 1.0, 1.0), |l| {
        let x = |v: Option<f64>| v.unwrap_or(1.0);
        (x(l.input), x(l.output), x(l.cache_read), x(l.cache_write))
    });
    Pricing {
        input: p.input * m * i,
        output: p.output * m * o,
        cache_read: p.cache_read * m * cr,
        cache_write: p.cache_write * m * cw,
    }
}

/// Dollars for the event's tokens at `p`'s per-1000-token rates.
fn token_cost(p: &Pricing, event: &UsageEvent) -> f64 {
    (event.input_tokens as f64 * p.input
        + event.output_tokens as f64 * p.output
        + event.cache_read_tokens as f64 * p.cache_read
        + event.cache_write_tokens as f64 * p.cache_write)
        / 1000.0
}

pub fn price(event: &UsageEvent) -> Priced {
    let id = event
        .invoked_model_id
        .as_deref()
        .unwrap_or(&event.raw_model_id);
    let model_id = get_base_model_id(id).to_string();
    match event.service {
        UsageService::Converse | UsageService::Agent => {
            // The registry first; models since removed from it keep their last price.
            let (name, pricing) = match get_model_config(&model_id) {
                Some(c) => (
                    Some(c.name.clone()),
                    c.pricing.map(|p| {
                        apply_rules(p, c.pricing_rules.as_ref(), &event.raw_model_id, event)
                    }),
                ),
                None => match get_retired_pricing(&model_id) {
                    Some(r) => (Some(r.name.clone()), Some(r.pricing)),
                    None => (None, None),
                },
            };
            Priced {
                model_name: name.unwrap_or_else(|| model_id.clone()),
                cost_usd: pricing.map(|p| token_cost(&p, event)),
                snapshot: pricing.map(|p| serde_json::to_value(p).unwrap_or(Value::Null)),
                model_id,
            }
        }
        UsageService::Embed => {
            let pricing = get_embedding_pricing(&model_id);
            Priced {
                model_name: pricing.map_or_else(|| model_id.clone(), |p| p.name.clone()),
                cost_usd: pricing.map(|p| event.input_tokens as f64 * p.input_per1k / 1000.0),
                snapshot: pricing.map(|p| json!({ "input": p.input_per1k })),
                model_id,
            }
        }
        UsageService::Image
        | UsageService::Video
        | UsageService::Guardrail
        | UsageService::Translate => {
            let pricing = get_unit_pricing(&model_id)
                .filter(|p| event.unit_kind.is_some_and(|k| unit_matches(p.unit, k)));
            Priced {
                model_name: get_unit_pricing(&model_id)
                    .map_or_else(|| model_id.clone(), |p| p.name.clone()),
                cost_usd: pricing.map(|p| event.units * p.price),
                snapshot: pricing.map(|p| json!({ "perUnit": p.price })),
                model_id,
            }
        }
        // No usage is reported for these, so there is nothing to price.
        UsageService::Retrieve | UsageService::Flow => Priced {
            model_name: model_id.clone(),
            cost_usd: None,
            snapshot: None,
            model_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::usage::UsageContext;

    fn event(service: UsageService, model: &str) -> UsageEvent {
        UsageEvent::new(service, model, "us-east-1", UsageContext::default())
    }

    #[test]
    fn converse_cost_uses_catalog_rates_and_strips_prefix() {
        let mut e = event(UsageService::Converse, "us.anthropic.claude-sonnet-4-6");
        e.input_tokens = 1000;
        e.output_tokens = 500;
        e.cache_read_tokens = 200;
        e.cache_write_tokens = 100;
        let p = price(&e);
        assert_eq!(p.model_id, "anthropic.claude-sonnet-4-6");
        let expected = (1000.0 * 0.003 + 500.0 * 0.015 + 200.0 * 0.0003 + 100.0 * 0.00375) / 1000.0;
        assert!((p.cost_usd.unwrap() - expected).abs() < 1e-12);
        assert_eq!(p.snapshot.unwrap()["input"], 0.003);
    }

    #[test]
    fn router_prices_the_invoked_model() {
        let mut e = event(
            UsageService::Converse,
            "arn:aws:bedrock:us-east-1:123:default-prompt-router/anthropic.claude:1",
        );
        e.invoked_model_id = Some("anthropic.claude-sonnet-4-6".into());
        e.input_tokens = 1000;
        assert!(price(&e).cost_usd.unwrap() > 0.0);
    }

    #[test]
    fn retired_models_keep_their_price() {
        let mut e = event(
            UsageService::Converse,
            "us.anthropic.claude-sonnet-4-20250514-v1:0",
        );
        e.input_tokens = 1000;
        let p = price(&e);
        assert_eq!(p.model_name, "Claude Sonnet 4");
        assert!((p.cost_usd.unwrap() - 0.003).abs() < 1e-12);
    }

    #[test]
    fn unit_services_price_per_unit() {
        let e = UsageEvent::units(
            UsageService::Image,
            "stability.sd3-5-large-v1:0",
            "us-west-2",
            UsageContext::default(),
            UnitKind::Images,
            2.0,
        );
        let p = price(&e);
        assert!((p.cost_usd.unwrap() - 0.16).abs() < 1e-12);
        assert_eq!(p.snapshot.unwrap()["perUnit"], 0.08);

        let e = UsageEvent::units(
            UsageService::Translate,
            "amazon.translate",
            "us-east-1",
            UsageContext::default(),
            UnitKind::Characters,
            1_000_000.0,
        );
        assert!((price(&e).cost_usd.unwrap() - 15.0).abs() < 1e-9);

        // A unit the entry doesn't charge for is not priced.
        let e = UsageEvent::units(
            UsageService::Image,
            "stability.sd3-5-large-v1:0",
            "",
            UsageContext::default(),
            UnitKind::Characters,
            1.0,
        );
        assert_eq!(price(&e).cost_usd, None);
        let e = UsageEvent::new(UsageService::Retrieve, "KB123", "", UsageContext::default());
        assert_eq!(price(&e).cost_usd, None);
    }

    #[test]
    fn profile_and_long_context_rules() {
        let mut e = event(UsageService::Converse, "global.anthropic.claude-haiku-5-5");
        e.input_tokens = 1000;
        let global = price(&e).cost_usd.unwrap();
        assert!((global - 0.0001).abs() < 1e-12);

        e.raw_model_id = "us.anthropic.claude-haiku-5-5".into();
        let us = price(&e);
        assert!((us.cost_usd.unwrap() - 0.00011).abs() < 1e-12);
        // The snapshot keeps the rates actually used.
        assert!((us.snapshot.unwrap()["input"].as_f64().unwrap() - 0.00011).abs() < 1e-12);

        // Over 100K of prompt (cache reads count): every rate 5x for the whole request.
        e.raw_model_id = "global.anthropic.claude-haiku-5-5".into();
        e.cache_read_tokens = 100_000;
        let long = price(&e).cost_usd.unwrap();
        let expected = (1000.0 * 0.0001 + 100_000.0 * 0.00001) * 5.0 / 1000.0;
        assert!((long - expected).abs() < 1e-12, "{long} vs {expected}");

        // Models without rules are unchanged by the prefix.
        let mut s = event(UsageService::Converse, "us.anthropic.claude-sonnet-4-6");
        s.input_tokens = 1000;
        assert!((price(&s).cost_usd.unwrap() - 0.003).abs() < 1e-12);
        assert_eq!(profile_prefix("arn:aws:bedrock:x"), "base");
        assert_eq!(profile_prefix("apac.amazon.nova-pro-v1:0"), "apac");
    }

    #[test]
    fn unknown_models_are_unpriced() {
        let p = price(&event(UsageService::Converse, "acme.mystery-v1"));
        assert_eq!(p.cost_usd, None);
        assert_eq!(p.model_name, "acme.mystery-v1");
        assert_eq!(
            price(&event(UsageService::Embed, "acme.embed")).cost_usd,
            None
        );
    }

    #[test]
    fn embedding_cost() {
        let mut e = event(UsageService::Embed, "amazon.titan-embed-text-v2:0");
        e.input_tokens = 50_000;
        let p = price(&e);
        assert!((p.cost_usd.unwrap() - 0.001).abs() < 1e-12);
        assert_eq!(p.model_name, "Titan Text Embeddings V2");
    }
}
