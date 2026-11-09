import { describe, test, expect } from '@jest/globals'
import {
  allModels,
  clampMaxTokensToModelLimit,
  getModelConfig,
  getModelsForRegion
} from '../models'
import { PricingCalculator } from '../pricing'

// OpenAI GPT-6 models are served through the standard Bedrock
// Converse API. They do NOT support on-demand invocation of the bare model ID,
// so each is exposed only via cross-region inference profiles (global.* and
// us.*). These cases guard registration, per-region availability, config
// resolution, and pricing.
// GPT-6.1 Sol and GPT-6.1 Terra are covered separately below.
const OPENAI_GPT_MODELS = [
  { base: 'gpt-6-astra', name: 'GPT-6 Astra' },
  { base: 'gpt-6-sol', name: 'GPT-6 Sol' },
  { base: 'gpt-6-luna', name: 'GPT-6 Luna' }
]

describe('OpenAI GPT model registry integration', () => {
  test.each(OPENAI_GPT_MODELS)(
    '$base is exposed as global.* and us.* inference profiles (no bare model ID)',
    ({ base, name }) => {
      const ids = allModels.map((m) => m.modelId)
      // Cross-region profiles exist...
      expect(ids).toContain(`global.openai.${base}`)
      expect(ids).toContain(`us.openai.${base}`)
      // ...and the bare on-demand ID is NOT registered (Converse rejects it).
      expect(ids).not.toContain(`openai.${base}`)

      const usModel = allModels.find((m) => m.modelId === `us.openai.${base}`)
      expect(usModel?.modelName).toBe(`${name} (US)`)
      expect(usModel?.toolUse).toBe(true)
      expect(usModel?.supportsThinking).toBe(true)
    }
  )

  // GPT-5.4 and 5.5 were never reachable via Converse; GPT-5.6 was retired on 2026-11-09.
  test('no GPT-5.x model is registered', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids.filter((id) => /\.gpt-5[.-]/.test(id))).toEqual([])
  })

  test('all OpenAI GPT models are available in the three US regions', () => {
    for (const region of ['us-east-1', 'us-east-2', 'us-west-2'] as const) {
      const ids = getModelsForRegion(region).map((m) => m.modelId)
      for (const { base } of OPENAI_GPT_MODELS) {
        expect(ids).toContain(`us.openai.${base}`)
        expect(ids).toContain(`global.openai.${base}`)
      }
    }
  })

  test('getModelConfig resolves prefixed ids to the right config', () => {
    for (const { base, name } of OPENAI_GPT_MODELS) {
      expect(getModelConfig(`us.openai.${base}`)?.name).toBe(name)
      expect(getModelConfig(`global.openai.${base}`)?.name).toBe(name)
    }
  })

  test('every OpenAI GPT model has pricing wired', () => {
    for (const { base } of OPENAI_GPT_MODELS) {
      const pricing = getModelConfig(`us.openai.${base}`)?.pricing
      expect(pricing).toBeDefined()
      expect(pricing!.input).toBeGreaterThan(0)
      expect(pricing!.output).toBeGreaterThan(0)
    }
  })

  test('pricing calculator uses per-1K rates (GPT-6 Astra: $11/$55 per 1M)', () => {
    const calc = new PricingCalculator('us.openai.gpt-6-astra')
    // Short Context Window (272K) Geo CRIS rate: 1M input = $11.00, 1M output = $55.00
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(11.0, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(55.0, 5)
  })

  test('pricing calculator uses per-1K rates (GPT-6 Sol: $2/$10 per 1M)', () => {
    const calc = new PricingCalculator('us.openai.gpt-6-sol')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(2.0, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(10.0, 5)
  })

  test('pricing calculator uses per-1K rates (GPT-6 Luna: $0.10/$0.50 per 1M)', () => {
    const calc = new PricingCalculator('us.openai.gpt-6-luna')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(0.1, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(0.5, 5)
  })

  // The GPT-6 tiers share a prefix with GPT-6.1's, so config resolution must
  // keep each tier on its own rates.
  test('GPT-6 tiers resolve to their own rates', () => {
    expect(getModelConfig('us.openai.gpt-6-sol')?.name).toBe('GPT-6 Sol')
    expect(getModelConfig('us.openai.gpt-6-luna')?.name).toBe('GPT-6 Luna')
    expect(getModelConfig('us.openai.gpt-6-sol')?.pricing?.input).toBe(0.002)
    expect(getModelConfig('us.openai.gpt-6-luna')?.pricing?.input).toBe(0.0001)
  })

  test('GPT-6 Astra allows 128K max output tokens', () => {
    expect(getModelConfig('us.openai.gpt-6-astra')?.maxTokensLimit).toBe(128000)
  })
})

// GPT-6.1 Sol is exposed through the US geo and global cross-region profiles;
// there is no in-region invocation of the bare model ID on bedrock-runtime.
// Its base ID also sits next to GPT-6 Sol's, which costs half as much, so
// config resolution must keep the two apart.
describe('OpenAI GPT-6.1 Sol registry integration', () => {
  test('is exposed as the us.* and global.* profiles (no bare model ID)', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('us.openai.gpt-6.1-sol')
    expect(ids).toContain('global.openai.gpt-6.1-sol')
    expect(ids).not.toContain('openai.gpt-6.1-sol')
  })

  test('display name carries the routing suffix and capabilities are set', () => {
    const model = allModels.find((m) => m.modelId === 'us.openai.gpt-6.1-sol')
    expect(model?.modelName).toBe('GPT-6.1 Sol (US)')
    expect(allModels.find((m) => m.modelId === 'global.openai.gpt-6.1-sol')?.modelName).toBe(
      'GPT-6.1 Sol (Global)'
    )
    expect(model?.toolUse).toBe(true)
    expect(model?.supportsThinking).toBe(true)
    expect(model?.maxTokensLimit).toBe(131072)
  })

  test('is offered in the three US regions and nowhere else', () => {
    for (const region of ['us-east-1', 'us-east-2', 'us-west-2'] as const) {
      const ids = getModelsForRegion(region).map((m) => m.modelId)
      expect(ids).toContain('us.openai.gpt-6.1-sol')
      expect(ids).toContain('global.openai.gpt-6.1-sol')
    }
    for (const region of ['eu-west-1', 'ap-northeast-1'] as const) {
      expect(getModelsForRegion(region).map((m) => m.modelId)).not.toContain(
        'us.openai.gpt-6.1-sol'
      )
    }
  })

  test('does not resolve to GPT-6 Sol, which costs half as much', () => {
    expect(getModelConfig('us.openai.gpt-6.1-sol')?.name).toBe('GPT-6.1 Sol')
    expect(getModelConfig('global.openai.gpt-6.1-sol')?.name).toBe('GPT-6.1 Sol')
    expect(getModelConfig('us.openai.gpt-6-sol')?.name).toBe('GPT-6 Sol')
  })

  test('no explicit prompt cache is declared (implicit caching only)', () => {
    expect(getModelConfig('us.openai.gpt-6.1-sol')?.cache).toBeUndefined()
  })

  test('pricing uses the US CRIS short-context rate ($2.20/$11.00/$0.11/$2.75 per 1M)', () => {
    const calc = new PricingCalculator('us.openai.gpt-6.1-sol')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(2.2, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(11.0, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.11, 5)
    expect(calc.calculateCacheWriteCost(1_000_000)).toBeCloseTo(2.75, 5)
  })
})

// GPT-6.1 Terra, like GPT-6.1 Sol, has only the US geo and global profiles, and
// config resolution must keep it apart from GPT-6.1 Sol.
describe('OpenAI GPT-6.1 Terra registry integration', () => {
  test('is exposed as the us.* and global.* profiles (no bare model ID)', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('us.openai.gpt-6.1-terra')
    expect(ids).toContain('global.openai.gpt-6.1-terra')
    expect(ids).not.toContain('openai.gpt-6.1-terra')
  })

  test('display name carries the routing suffix and capabilities are set', () => {
    const model = allModels.find((m) => m.modelId === 'us.openai.gpt-6.1-terra')
    expect(model?.modelName).toBe('GPT-6.1 Terra (US)')
    expect(allModels.find((m) => m.modelId === 'global.openai.gpt-6.1-terra')?.modelName).toBe(
      'GPT-6.1 Terra (Global)'
    )
    expect(model?.toolUse).toBe(true)
    expect(model?.supportsThinking).toBe(true)
    expect(model?.maxTokensLimit).toBe(128000)
  })

  test('is offered in the three US regions and nowhere else', () => {
    for (const region of ['us-east-1', 'us-east-2', 'us-west-2'] as const) {
      const ids = getModelsForRegion(region).map((m) => m.modelId)
      expect(ids).toContain('us.openai.gpt-6.1-terra')
      expect(ids).toContain('global.openai.gpt-6.1-terra')
    }
    for (const region of ['eu-west-1', 'ap-northeast-1'] as const) {
      expect(getModelsForRegion(region).map((m) => m.modelId)).not.toContain(
        'us.openai.gpt-6.1-terra'
      )
    }
  })

  test('does not resolve to GPT-6.1 Sol', () => {
    expect(getModelConfig('us.openai.gpt-6.1-terra')?.name).toBe('GPT-6.1 Terra')
    expect(getModelConfig('global.openai.gpt-6.1-terra')?.name).toBe('GPT-6.1 Terra')
    expect(getModelConfig('us.openai.gpt-6.1-sol')?.name).toBe('GPT-6.1 Sol')
  })

  test('no explicit prompt cache is declared (implicit caching only)', () => {
    expect(getModelConfig('us.openai.gpt-6.1-terra')?.cache).toBeUndefined()
  })

  test('pricing uses the US CRIS short-context rate ($1.10/$5.50/$0.055/$1.375 per 1M)', () => {
    const calc = new PricingCalculator('us.openai.gpt-6.1-terra')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(1.1, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(5.5, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.055, 5)
    expect(calc.calculateCacheWriteCost(1_000_000)).toBeCloseTo(1.375, 5)
  })
})

// Claude Opus 5.5 uses an unsuffixed base ID (`anthropic.claude-opus-5-5`) that
// contains Opus 5's base ID, so config resolution has to prefer the longest
// match. Bedrock offers no in-region invocation, only global + US/EU/JP geos.
describe('Claude Opus 5.5 registry integration', () => {
  test('is exposed as global/US/EU/JP profiles and no bare model ID', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('global.anthropic.claude-opus-5-5')
    expect(ids).toContain('us.anthropic.claude-opus-5-5')
    expect(ids).toContain('eu.anthropic.claude-opus-5-5')
    expect(ids).toContain('jp.anthropic.claude-opus-5-5')
    expect(ids).not.toContain('anthropic.claude-opus-5-5')
  })

  test('display names carry the routing suffix and capabilities are set', () => {
    const usModel = allModels.find((m) => m.modelId === 'us.anthropic.claude-opus-5-5')
    expect(usModel?.modelName).toBe('Claude Opus 5.5 (US)')
    expect(usModel?.toolUse).toBe(true)
    expect(usModel?.supportsThinking).toBe(true)
    expect(usModel?.supportedThinkingTypes).toEqual(['adaptive'])
    expect(usModel?.maxTokensLimit).toBe(128000)
  })

  test('pricing uses per-1K rates ($4/$20 per 1M, cache read a tenth of input)', () => {
    const calc = new PricingCalculator('us.anthropic.claude-opus-5-5')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(4.0, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(20.0, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.4, 5)
  })

  test('prompt caching is declared on messages, system and tools', () => {
    const cache = getModelConfig('us.anthropic.claude-opus-5-5')?.cache
    expect(cache?.supported).toBe(true)
    expect(cache?.cacheableFields).toEqual(['messages', 'system', 'tools'])
  })

  // A base ID that is a prefix of another must not swallow the longer model.
  test('config resolution does not confuse Opus 5.5 with Opus 5', () => {
    expect(getModelConfig('us.anthropic.claude-opus-5-5')?.name).toBe('Claude Opus 5.5')
    expect(getModelConfig('global.anthropic.claude-opus-5-5')?.name).toBe('Claude Opus 5.5')
    expect(getModelConfig('us.anthropic.claude-opus-5')?.name).toBe('Claude Opus 5')
    // Same shape in the Fable line: Fable 5 vs Fable 5.1 vs Fable 5.2.
    expect(getModelConfig('us.anthropic.claude-fable-5-2')?.name).toBe('Claude Fable 5.2')
    expect(getModelConfig('us.anthropic.claude-fable-5-1')?.name).toBe('Claude Fable 5.1')
    expect(getModelConfig('global.anthropic.claude-fable-5')?.name).toBe('Claude Fable 5')
  })
})

// Claude Fable 5.2 is offered only through the global endpoint and the US geo
// profile (no bare model ID, no EU/JP geo).
describe('Claude Fable 5.2 registry integration', () => {
  test('is exposed as global/US profiles and no bare model ID', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('global.anthropic.claude-fable-5-2')
    expect(ids).toContain('us.anthropic.claude-fable-5-2')
    expect(ids).not.toContain('eu.anthropic.claude-fable-5-2')
    expect(ids).not.toContain('jp.anthropic.claude-fable-5-2')
    expect(ids).not.toContain('anthropic.claude-fable-5-2')
  })

  test('display names carry the routing suffix and capabilities are set', () => {
    const globalModel = allModels.find((m) => m.modelId === 'global.anthropic.claude-fable-5-2')
    expect(globalModel?.modelName).toBe('Claude Fable 5.2 (Global)')
    expect(globalModel?.toolUse).toBe(true)
    expect(globalModel?.supportsThinking).toBe(true)
    expect(globalModel?.supportedThinkingTypes).toEqual(['adaptive'])
    expect(globalModel?.maxTokensLimit).toBe(128000)
    expect(globalModel?.regions).toContain('eu-west-1')
    expect(globalModel?.regions).toContain('ap-northeast-1')
  })

  test('the global profile reaches Europe and APAC, the US geo only North America', () => {
    const euIds = getModelsForRegion('eu-west-1').map((m) => m.modelId)
    expect(euIds).toContain('global.anthropic.claude-fable-5-2')
    expect(euIds).not.toContain('us.anthropic.claude-fable-5-2')
    const caIds = getModelsForRegion('ca-central-1').map((m) => m.modelId)
    expect(caIds).toContain('us.anthropic.claude-fable-5-2')
  })

  test('pricing uses per-1K rates ($10/$50 per 1M, cache read a tenth of input)', () => {
    const calc = new PricingCalculator('global.anthropic.claude-fable-5-2')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(10.0, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(50.0, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(1.0, 5)
    expect(calc.calculateCacheWriteCost(1_000_000)).toBeCloseTo(12.5, 5)
  })

  test('prompt caching is declared on messages, system and tools', () => {
    const cache = getModelConfig('us.anthropic.claude-fable-5-2')?.cache
    expect(cache?.supported).toBe(true)
    expect(cache?.cacheableFields).toEqual(['messages', 'system', 'tools'])
  })
})

// The Claude 4.x family was retired on 2026-11-09; its prices live in
// RETIRED_MODEL_PRICING and saved settings are migrated by the Rust store.
describe('retired Claude 4.x models', () => {
  test('no Claude 4.x model is registered', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids.filter((id) => /claude-(sonnet|opus|haiku)-4/.test(id))).toEqual([])
  })
})

// Claude Haiku 5.5 is offered only through inference profiles.
describe('Claude Haiku 5.5 registry integration', () => {
  test('is exposed as global/US/EU/JP profiles and no bare model ID', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('global.anthropic.claude-haiku-5-5')
    expect(ids).toContain('us.anthropic.claude-haiku-5-5')
    expect(ids).toContain('eu.anthropic.claude-haiku-5-5')
    expect(ids).toContain('jp.anthropic.claude-haiku-5-5')
    expect(ids).not.toContain('anthropic.claude-haiku-5-5')
  })

  test('display names carry the routing suffix and capabilities are set', () => {
    const usModel = allModels.find((m) => m.modelId === 'us.anthropic.claude-haiku-5-5')
    expect(usModel?.modelName).toBe('Claude Haiku 5.5 (US)')
    expect(usModel?.toolUse).toBe(true)
    expect(usModel?.supportsThinking).toBe(true)
    expect(usModel?.supportedThinkingTypes).toEqual(['adaptive'])
    expect(usModel?.maxTokensLimit).toBe(128000)
  })

  test('pricing uses the global rate ($0.10/$0.50 per 1M, cache read a tenth of input)', () => {
    const calc = new PricingCalculator('global.anthropic.claude-haiku-5-5')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(0.1, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(0.5, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.01, 5)
    expect(calc.calculateCacheWriteCost(1_000_000)).toBeCloseTo(0.125, 5)
  })

  test('prompt caching is declared on messages, system and tools', () => {
    const cache = getModelConfig('us.anthropic.claude-haiku-5-5')?.cache
    expect(cache?.supported).toBe(true)
    expect(cache?.cacheableFields).toEqual(['messages', 'system', 'tools'])
  })
})

// Claude Sonnet 5.5 has the same shape: `anthropic.claude-sonnet-5-5` contains
// Sonnet 5's base ID, and it is offered only through inference profiles.
describe('Claude Sonnet 5.5 registry integration', () => {
  test('is exposed as global/US/EU/JP profiles and no bare model ID', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids).toContain('global.anthropic.claude-sonnet-5-5')
    expect(ids).toContain('us.anthropic.claude-sonnet-5-5')
    expect(ids).toContain('eu.anthropic.claude-sonnet-5-5')
    expect(ids).toContain('jp.anthropic.claude-sonnet-5-5')
    expect(ids).not.toContain('anthropic.claude-sonnet-5-5')
  })

  test('display names carry the routing suffix and capabilities are set', () => {
    const usModel = allModels.find((m) => m.modelId === 'us.anthropic.claude-sonnet-5-5')
    expect(usModel?.modelName).toBe('Claude Sonnet 5.5 (US)')
    expect(usModel?.toolUse).toBe(true)
    expect(usModel?.supportsThinking).toBe(true)
    expect(usModel?.supportedThinkingTypes).toEqual(['adaptive'])
    expect(usModel?.maxTokensLimit).toBe(128000)
  })

  test('pricing matches Sonnet 5 ($2/$10 per 1M, cache read a tenth of input)', () => {
    const calc = new PricingCalculator('us.anthropic.claude-sonnet-5-5')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(2.0, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(10.0, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.2, 5)
  })

  test('prompt caching is declared on messages, system and tools', () => {
    const cache = getModelConfig('us.anthropic.claude-sonnet-5-5')?.cache
    expect(cache?.supported).toBe(true)
    expect(cache?.cacheableFields).toEqual(['messages', 'system', 'tools'])
  })

  // A base ID that is a prefix of another must not swallow the longer model.
  test('config resolution does not confuse Sonnet 5.5 with Sonnet 5', () => {
    expect(getModelConfig('us.anthropic.claude-sonnet-5-5')?.name).toBe('Claude Sonnet 5.5')
    expect(getModelConfig('global.anthropic.claude-sonnet-5-5')?.name).toBe('Claude Sonnet 5.5')
    expect(getModelConfig('us.anthropic.claude-sonnet-5')?.name).toBe('Claude Sonnet 5')
  })
})

// xAI Grok 4.6 and 4.7 are served through the standard Bedrock Converse API, but
// only via cross-region inference profiles (global.* and us.*) — in-region
// invocation of the bare model ID is not supported on bedrock-runtime. Grok 4.3
// is Mantle-only (no Converse), so it must not appear at all.
describe('xAI Grok model registry integration', () => {
  test.each(['grok-4.6', 'grok-4.7'])(
    '%s is exposed as global.* and us.* inference profiles (no bare model ID)',
    (baseId) => {
      const ids = allModels.map((m) => m.modelId)
      expect(ids).toContain(`global.xai.${baseId}`)
      expect(ids).toContain(`us.xai.${baseId}`)
      expect(ids).not.toContain(`xai.${baseId}`)
    }
  )

  test.each([
    ['grok-4.6', 'Grok 4.6'],
    ['grok-4.7', 'Grok 4.7']
  ])('%s display names carry the routing suffix and capabilities are set', (baseId, name) => {
    const usModel = allModels.find((m) => m.modelId === `us.xai.${baseId}`)
    expect(usModel?.modelName).toBe(`${name} (US)`)
    expect(usModel?.toolUse).toBe(true)
    expect(usModel?.supportsThinking).toBe(true)
    expect(usModel?.maxTokensLimit).toBe(32768)

    const globalModel = allModels.find((m) => m.modelId === `global.xai.${baseId}`)
    expect(globalModel?.modelName).toBe(`${name} (Global)`)
  })

  // Config resolution is substring-based, so pin that every Grok 4.7 ID resolves
  // to Grok 4.7's own ceiling and thinking type.
  test('Grok 4.7 resolves to its own config', () => {
    for (const grokId of ['us.xai.grok-4.7', 'global.xai.grok-4.7', 'xai.grok-4.7']) {
      const config = getModelConfig(grokId)
      expect(config?.name).toBe('Grok 4.7')
      expect(config?.provider).toBe('xai')
      expect(config?.maxTokensLimit).toBe(32768)
      expect(config?.supportedThinkingTypes).toEqual(['enabled'])
    }
  })

  test('Grok 4.3 is not registered (not available via Converse)', () => {
    const ids = allModels.map((m) => m.modelId)
    expect(ids.some((id) => id.includes('grok-4.3'))).toBe(false)
  })

  test('getModelConfig resolves prefixed ids to the right config', () => {
    expect(getModelConfig('us.xai.grok-4.6')?.name).toBe('Grok 4.6')
    expect(getModelConfig('global.xai.grok-4.6')?.name).toBe('Grok 4.6')
    expect(getModelConfig('us.xai.grok-4.7')?.name).toBe('Grok 4.7')
    expect(getModelConfig('global.xai.grok-4.7')?.name).toBe('Grok 4.7')
  })

  test('no explicit prompt cache is declared (implicit caching only)', () => {
    expect(getModelConfig('us.xai.grok-4.6')?.cache).toBeUndefined()
    expect(getModelConfig('us.xai.grok-4.7')?.cache).toBeUndefined()
  })

  test.each(['grok-4.6', 'grok-4.7'])(
    'only the global %s profile reaches regions outside the US',
    (baseId) => {
      const euIds = getModelsForRegion('eu-west-1').map((m) => m.modelId)
      expect(euIds).toContain(`global.xai.${baseId}`)
      expect(euIds).not.toContain(`us.xai.${baseId}`)

      const usIds = getModelsForRegion('us-east-1').map((m) => m.modelId)
      expect(usIds).toContain(`global.xai.${baseId}`)
      expect(usIds).toContain(`us.xai.${baseId}`)
    }
  )

  // Grok 4.7 reaches three regions Grok 4.6 does not: Jakarta, Melbourne and
  // (Global only) the APAC/EU tail the 4.6 model card left out.
  test('Grok 4.7 is offered in Jakarta and Melbourne, where Grok 4.6 is not', () => {
    for (const region of ['ap-southeast-3', 'ap-southeast-4']) {
      const ids = getModelsForRegion(region).map((m) => m.modelId)
      expect(ids).toContain('global.xai.grok-4.7')
      expect(ids).not.toContain('global.xai.grok-4.6')
    }
  })

  test('Grok 4.6 pricing uses the Geo/US CRIS rate ($2.20/$6.60/$0.55 per 1M)', () => {
    const calc = new PricingCalculator('us.xai.grok-4.6')
    expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(2.2, 5)
    expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(6.6, 5)
    expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.55, 5)
  })

  // One rate per model, and for Grok 4.7 it is the Global CRIS rate, so US
  // routing is under-estimated by the 10% Geo premium.
  test.each(['us.xai.grok-4.7', 'global.xai.grok-4.7'])(
    '%s pricing uses the Global CRIS rate ($2.00/$6.00/$0.50 per 1M)',
    (modelId) => {
      const calc = new PricingCalculator(modelId)
      expect(calc.calculateInputCost(1_000_000)).toBeCloseTo(2.0, 5)
      expect(calc.calculateOutputCost(1_000_000)).toBeCloseTo(6.0, 5)
      expect(calc.calculateCacheReadCost(1_000_000)).toBeCloseTo(0.5, 5)
    }
  )
})

// Max Output Tokens is a single global setting, so every request is clamped to
// the selected model's own ceiling before it is sent.
describe('clampMaxTokensToModelLimit', () => {
  test('lowers a request that exceeds the model ceiling', () => {
    // DeepSeek R1 stops at 32768 even though the setting allows 128000.
    expect(clampMaxTokensToModelLimit('us.deepseek.r1-v1:0', 128000)).toBe(32768)
    // First-generation Nova models stop far lower.
    expect(clampMaxTokensToModelLimit('us.amazon.nova-lite-v1:0', 128000)).toBe(5120)
  })

  test('leaves a request at or below the ceiling untouched', () => {
    expect(clampMaxTokensToModelLimit('us.anthropic.claude-opus-5', 128000)).toBe(128000)
    expect(clampMaxTokensToModelLimit('us.anthropic.claude-opus-5', 4096)).toBe(4096)
    expect(clampMaxTokensToModelLimit('us.openai.gpt-6-astra', 8192)).toBe(8192)
  })

  test('passes through models the registry does not know', () => {
    // Custom inference profile ARNs and imported models have no known ceiling,
    // so clamping them to a default would silently truncate their output.
    expect(
      clampMaxTokensToModelLimit(
        'arn:aws:bedrock:us-east-1:123456789012:imported-model/abcdefg',
        200000
      )
    ).toBe(200000)
  })

  test('passes through an unset maxTokens rather than inventing one', () => {
    expect(clampMaxTokensToModelLimit('us.anthropic.claude-opus-5', undefined)).toBeUndefined()
  })

  test('every registered text model declares a max output ceiling', () => {
    // A model with no ceiling silently opts out of clamping, so guard against
    // an entry being added without one.
    const missing = allModels.filter((m) => !m.maxTokensLimit).map((m) => m.modelId)
    expect(missing).toEqual([])
  })
})

// Output ceilings verified against each model's AWS model card and confirmed
// against the Converse API, which rejects an over-large maxTokens with
// "exceeds the model limit of N".
describe('model output ceilings', () => {
  test.each([
    ['us.anthropic.claude-haiku-5-5', 128000],
    ['us.anthropic.claude-opus-5', 128000],
    ['us.anthropic.claude-opus-5-5', 128000],
    ['us.anthropic.claude-sonnet-5', 128000],
    ['us.anthropic.claude-sonnet-5-5', 128000],
    ['global.anthropic.claude-fable-5-2', 128000],
    ['us.amazon.nova-lite-v1:0', 5120],
    ['us.amazon.nova-pro-v1:0', 5120],
    ['us.amazon.nova-2-lite-v1:0', 64000],
    ['us.deepseek.r1-v1:0', 32768],
    ['us.openai.gpt-6-astra', 128000],
    ['us.openai.gpt-6.1-sol', 131072],
    ['us.openai.gpt-6.1-terra', 128000],
    ['us.openai.gpt-6-sol', 128000],
    ['us.openai.gpt-6-luna', 128000],
    ['openai.gpt-oss-120b-1:0', 16384],
    ['openai.gpt-oss-20b-1:0', 16384],
    ['moonshotai.kimi-k2.5', 16384],
    ['us.moonshotai.kimi-k3', 16384],
    ['us.xai.grok-4.6', 32768],
    ['us.xai.grok-4.7', 32768]
  ])('%s caps output at %i tokens', (modelId, expected) => {
    expect(getModelConfig(modelId as string)?.maxTokensLimit).toBe(expected)
  })
})
