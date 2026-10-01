//! Cost calculation. Port of `src/common/models/pricing.ts`.
//!
//! `PricingCalculator.formatCurrency` (an `Intl.NumberFormat` wrapper) is renderer-only display
//! code and stays in TypeScript.

use crate::registry::{get_model_config, Pricing};

/// Computes dollar costs from token counts using a model's per-1K-token rates.
/// Unknown models cost 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PricingCalculator {
    pricing: Option<Pricing>,
}

impl PricingCalculator {
    pub fn new(model_id: &str) -> Self {
        Self {
            pricing: get_model_config(model_id).and_then(|c| c.pricing),
        }
    }

    fn cost(&self, tokens: u64, rate: impl Fn(&Pricing) -> f64) -> f64 {
        self.pricing
            .as_ref()
            .map_or(0.0, |p| (tokens as f64 * rate(p)) / 1000.0)
    }

    pub fn calculate_input_cost(&self, tokens: u64) -> f64 {
        self.cost(tokens, |p| p.input)
    }

    pub fn calculate_output_cost(&self, tokens: u64) -> f64 {
        self.cost(tokens, |p| p.output)
    }

    pub fn calculate_cache_read_cost(&self, tokens: u64) -> f64 {
        self.cost(tokens, |p| p.cache_read)
    }

    pub fn calculate_cache_write_cost(&self, tokens: u64) -> f64 {
        self.cost(tokens, |p| p.cache_write)
    }

    /// Sum of all four components (pass 0 for cache tokens to match the TS defaults).
    pub fn calculate_total_cost(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> f64 {
        self.calculate_input_cost(input_tokens)
            + self.calculate_output_cost(output_tokens)
            + self.calculate_cache_read_cost(cache_read_tokens)
            + self.calculate_cache_write_cost(cache_write_tokens)
    }

    pub fn get_pricing(&self) -> Option<Pricing> {
        self.pricing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_model_costs_nothing() {
        let calc = PricingCalculator::new("unknown-model");
        assert_eq!(calc.calculate_total_cost(1000, 500, 0, 0), 0.0);
        assert_eq!(calc.get_pricing(), None);
    }

    #[test]
    fn total_is_sum_of_components() {
        let calc = PricingCalculator::new("anthropic.claude-sonnet-4-20250514-v1:0");
        let expected = (1000.0 * 0.003 + 500.0 * 0.015 + 200.0 * 0.0003 + 100.0 * 0.00375) / 1000.0;
        assert!((calc.calculate_total_cost(1000, 500, 200, 100) - expected).abs() < 1e-9);
        assert_eq!(
            calc.get_pricing(),
            Some(Pricing {
                input: 0.003,
                output: 0.015,
                cache_read: 0.0003,
                cache_write: 0.00375
            })
        );
    }
}
