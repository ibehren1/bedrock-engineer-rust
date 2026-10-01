//! Model catalog, pricing, and prompt-cache metadata.
//!
//! Port of `src/common/models/` (`models.ts`, `pricing.ts`, `promptCache.ts`). The TS registry
//! stays the source of truth: its data is exported to `data/models.json` by
//! `scripts/port/gen-models-json.mjs` and embedded here with `include_str!`. Regenerate after any
//! change to `models.ts`:
//!
//! ```text
//! node scripts/port/gen-models-json.mjs          # rewrite data/models.json
//! node scripts/port/gen-models-json.mjs --check  # fail if it is stale
//! ```
//!
//! All public types serialize with the same camelCase JSON shape as the TS `ModelConfig` / `LLM`
//! types, so they can be handed to the renderer unchanged.
//!
//! Covered tests: `src/common/models/__tests__/registry.test.ts` (see `registry_tests.rs`).

mod collation;
pub mod pricing;
pub mod prompt_cache;
mod registry;

pub use collation::locale_compare;
pub use pricing::PricingCalculator;
pub use prompt_cache::PromptCacheManager;
pub use registry::*;

#[cfg(test)]
mod registry_tests;
