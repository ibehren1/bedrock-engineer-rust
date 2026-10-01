import type { BedrockSupportRegion, LLM, ThinkingType } from '../../types/llm'

/**
 * Type definition for cacheable fields
 */
export type CacheableField = 'messages' | 'system' | 'tools'

/**
 * Type definition for model providers
 */
export type ModelProvider =
  | 'anthropic'
  | 'amazon'
  | 'deepseek'
  | 'stability'
  | 'openai'
  | 'moonshotai'
  | 'xai'

/**
 * Type definition for model categories
 */
export type ModelCategory = 'text' | 'image'

/**
 * Type definition for inference profiles
 */
export type InferenceProfileType =
  | 'base' // Single region, no prefix
  | 'global' // global. - All commercial regions
  | 'regional-us' // us. - US region
  | 'regional-eu' // eu. - EU region
  | 'regional-apac' // apac. - APAC region
  | 'jp' // jp. - Japan domestic only

/**
 * Inference profile definition
 * Structure representing Amazon Bedrock inference profiles
 */
export interface InferenceProfile {
  /**
   * Type of inference profile
   * - base: Direct execution in a single region (no prefix)
   * - global: Global routing to all commercial AWS regions
   * - regional-us/eu/apac: Cross-region inference within specific geography
   * - jp: Cross-region inference limited to Japan (Tokyo/Osaka)
   */
  type: InferenceProfileType

  /**
   * Prefix added to model ID
   * - 'global': Global inference profile
   * - 'us': US region cross-region inference
   * - 'eu': EU region cross-region inference
   * - 'apac': APAC region cross-region inference
   * - 'jp': Japan domestic inference
   * - undefined: Base model (no prefix)
   */
  prefix?: string

  /**
   * List of AWS regions where this profile can process requests
   * For cross-region inference, load is automatically balanced across these regions
   */
  regions: BedrockSupportRegion[]

  /**
   * Suffix added to display name in UI
   * Examples: "(Global)", "(JP)", "(US)", "(EU)", "(APAC)"
   * undefined for base models
   */
  displaySuffix?: string
}

/**
 * Unified model configuration interface
 */
export interface ModelConfig {
  baseId: string
  name: string
  provider: ModelProvider
  category: ModelCategory

  // Features
  toolUse: boolean
  maxTokensLimit: number
  supportsThinking?: boolean
  supportedThinkingTypes?: ThinkingType[] // Which thinking API types the model accepts
  supportsStreamingToolUse?: boolean // Support for Tool Use with streaming

  // Inference profiles (new design)
  inferenceProfiles: InferenceProfile[]

  // Pricing (dollar price per 1000 tokens)
  pricing?: {
    input: number
    output: number
    cacheRead: number
    cacheWrite: number
  }

  // Cache configuration
  cache?: {
    supported: boolean
    cacheableFields: CacheableField[]
  }
}

/**
 * Unified model registry
 */
const MODEL_REGISTRY: ModelConfig[] = [
  // Claude Haiku 4.5
  {
    baseId: 'claude-haiku-4-5-20251001-v1:0',
    name: 'Claude Haiku 4.5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 64000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-1',
          'us-west-2',
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3',
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-northeast-3',
          'ap-south-1',
          'ap-south-2',
          'ap-southeast-1',
          'ap-southeast-2',
          'ap-southeast-3',
          'ap-southeast-4',
          'ca-central-1',
          'sa-east-1'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      }
    ],
    pricing: {
      input: 0.001,
      output: 0.005,
      cacheRead: 0.0001,
      cacheWrite: 0.00125
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 4.1
  {
    baseId: 'claude-opus-4-1-20250805-v1:0',
    name: 'Claude Opus 4.1',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 32000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.015,
      output: 0.075,
      cacheRead: 0.0015,
      cacheWrite: 0.01875
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Sonnet 4
  {
    baseId: 'claude-sonnet-4-20250514-v1:0',
    name: 'Claude Sonnet 4',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 64000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.003,
      output: 0.015,
      cacheRead: 0.0003,
      cacheWrite: 0.00375
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 4
  {
    baseId: 'claude-opus-4-20250514-v1:0',
    name: 'Claude Opus 4',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 32000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.015,
      output: 0.075,
      cacheRead: 0.0015,
      cacheWrite: 0.01875
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Sonnet 4.5
  {
    baseId: 'claude-sonnet-4-5-20250929-v1:0',
    name: 'Claude Sonnet 4.5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 64000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      }
    ],
    pricing: {
      input: 0.003,
      output: 0.015,
      cacheRead: 0.0003,
      cacheWrite: 0.00375
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Sonnet 4.6
  {
    baseId: 'claude-sonnet-4-6',
    name: 'Claude Sonnet 4.6',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 64000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      }
    ],
    pricing: {
      input: 0.003,
      output: 0.015,
      cacheRead: 0.0003,
      cacheWrite: 0.00375
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Sonnet 5
  {
    baseId: 'claude-sonnet-5',
    name: 'Claude Sonnet 5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      }
    ],
    pricing: {
      input: 0.002,
      output: 0.01,
      cacheRead: 0.0002,
      cacheWrite: 0.0025
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Sonnet 5.5
  // Anthropic's newest Sonnet (launched 2026-09-28): stronger at coding, agentic
  // work and instruction following than Sonnet 5, at the same price. 1M context
  // window, 128K max output, adaptive thinking. Model ID carries no version
  // suffix: invoked as e.g. `global.anthropic.claude-sonnet-5-5`.
  // Offered through the same inference profiles as Sonnet 5 — global plus the
  // US/EU/JP geos — with no in-region invocation of the bare model ID.
  // Pricing is unchanged from Sonnet 5: $2 in / $10 out per 1M tokens, stored per
  // 1K, cache read 10% of input and a 5-minute cache write 1.25x it.
  {
    baseId: 'claude-sonnet-5-5',
    name: 'Claude Sonnet 5.5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      }
    ],
    pricing: {
      input: 0.002,
      output: 0.01,
      cacheRead: 0.0002,
      cacheWrite: 0.0025
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 4.6
  {
    baseId: 'claude-opus-4-6-v1',
    name: 'Claude Opus 4.6',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.005,
      output: 0.025,
      cacheRead: 0.0005,
      cacheWrite: 0.00625
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 4.7
  {
    baseId: 'claude-opus-4-7',
    name: 'Claude Opus 4.7',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.005,
      output: 0.025,
      cacheRead: 0.0005,
      cacheWrite: 0.00625
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 4.8
  {
    baseId: 'claude-opus-4-8',
    name: 'Claude Opus 4.8',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.005,
      output: 0.025,
      cacheRead: 0.0005,
      cacheWrite: 0.00625
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 5
  // Anthropic's most advanced Opus model (launched 2026-07-24). 1M context window,
  // 128K max output, adaptive thinking on by default. Model ID has no version suffix:
  // invoked as e.g. `global.anthropic.claude-opus-5`.
  // NOTE: Opus 5 pricing is not yet published on the Bedrock pricing page; the values
  // below mirror Claude Opus 4.8 as a placeholder and should be updated once available.
  {
    baseId: 'claude-opus-5',
    name: 'Claude Opus 5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-west-2', 'us-east-1', 'us-east-2', 'eu-west-1', 'ap-northeast-1'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      }
    ],
    pricing: {
      input: 0.005,
      output: 0.025,
      cacheRead: 0.0005,
      cacheWrite: 0.00625
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Opus 5.5
  // Anthropic's most capable Opus model (launched 2026-09-22): better at coding,
  // knowledge work and long-running tasks, and cheaper to run than Opus 5. 1M
  // context window, 128K max output, adaptive thinking always on and not
  // disableable. Model ID carries no version suffix: invoked as e.g.
  // `global.anthropic.claude-opus-5-5`.
  // Bedrock offers no in-region invocation of the bare model ID, so there is no
  // `base` profile — only the global endpoint and the US/EU/JP geo profiles. The
  // model card also lists an `au.` geo (Sydney/Melbourne), which this registry
  // has no profile type for; Australian users can reach the model through the
  // global endpoint.
  // Pricing: $4 in / $20 out per 1M tokens, stored per 1K. Cache read is the
  // standard Anthropic 10% of the input rate; a 5-minute cache write is 1.25x it.
  {
    baseId: 'claude-opus-5-5',
    name: 'Claude Opus 5.5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-1',
          'us-west-2',
          'ca-central-1',
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3',
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-northeast-3',
          'ap-south-1',
          'ap-south-2',
          'ap-southeast-1',
          'ap-southeast-2',
          'ap-southeast-3',
          'ap-southeast-4',
          'sa-east-1'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2', 'ca-central-1'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3'
        ],
        displaySuffix: '(EU)'
      },
      {
        type: 'jp',
        prefix: 'jp',
        regions: ['ap-northeast-1', 'ap-northeast-3'],
        displaySuffix: '(JP)'
      }
    ],
    pricing: {
      input: 0.004,
      output: 0.02,
      cacheRead: 0.0004,
      cacheWrite: 0.005
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Fable 5
  {
    baseId: 'claude-fable-5',
    name: 'Claude Fable 5',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-west-2',
          'us-east-1',
          'us-east-2',
          'eu-west-1',
          'eu-central-1',
          'ap-northeast-1',
          'ap-northeast-3',
          'ap-southeast-1',
          'ap-southeast-2'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: [
          'eu-north-1',
          'eu-west-3',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-central-1'
        ],
        displaySuffix: '(EU)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.01,
      output: 0.05,
      cacheRead: 0.001,
      cacheWrite: 0.0125
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Claude Fable 5.1
  // Anthropic's most capable model for demanding reasoning and long-horizon agentic
  // work. Same always-on adaptive thinking, 1M context and 128K max output as Fable 5.
  // Bedrock currently offers the global endpoint plus a regional endpoint in
  // us-east-1 only (no EU/JP regional profiles yet).
  {
    baseId: 'claude-fable-5-1',
    name: 'Claude Fable 5.1',
    provider: 'anthropic',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['adaptive'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-west-2',
          'us-east-1',
          'us-east-2',
          'eu-west-1',
          'eu-central-1',
          'ap-northeast-1',
          'ap-northeast-3',
          'ap-southeast-1',
          'ap-southeast-2'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.01,
      output: 0.05,
      cacheRead: 0.001,
      cacheWrite: 0.0125
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system', 'tools']
    }
  },

  // Amazon Nova Premier
  {
    baseId: 'nova-premier-v1:0',
    name: 'Amazon Nova Premier',
    provider: 'amazon',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 32000,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ]
  },

  // Amazon Nova Pro
  {
    baseId: 'nova-pro-v1:0',
    name: 'Amazon Nova Pro',
    provider: 'amazon',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 5120,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: ['eu-central-1', 'eu-north-1', 'eu-south-1', 'eu-west-1', 'eu-west-3'],
        displaySuffix: '(EU)'
      },
      {
        type: 'regional-apac',
        prefix: 'apac',
        regions: [
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-south-1',
          'ap-southeast-1',
          'ap-southeast-2'
        ],
        displaySuffix: '(APAC)'
      }
    ],
    pricing: {
      input: 0.0008,
      output: 0.0032,
      cacheRead: 0.0002,
      cacheWrite: 0
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system']
    }
  },

  // Amazon Nova Lite
  {
    baseId: 'nova-lite-v1:0',
    name: 'Amazon Nova Lite',
    provider: 'amazon',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 5120,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: ['eu-central-1', 'eu-north-1', 'eu-south-1', 'eu-south-2', 'eu-west-3'],
        displaySuffix: '(EU)'
      },
      {
        type: 'regional-apac',
        prefix: 'apac',
        regions: [
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-south-1',
          'ap-southeast-1',
          'ap-southeast-2'
        ],
        displaySuffix: '(APAC)'
      }
    ],
    pricing: {
      input: 0.00006,
      output: 0.00024,
      cacheRead: 0.000015,
      cacheWrite: 0
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system']
    }
  },

  // Amazon Nova 2 Lite
  // Unlike the first-generation Nova models, which stop at 5K output tokens,
  // Nova 2 Lite emits up to 64K (AWS model card; the API accepts up to 65535).
  {
    baseId: 'nova-2-lite-v1:0',
    name: 'Amazon Nova 2 Lite',
    provider: 'amazon',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 64000,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-west-2'],
        displaySuffix: '(Global)'
      }
    ]
  },

  // Amazon Nova Micro
  {
    baseId: 'nova-micro-v1:0',
    name: 'Amazon Nova Micro',
    provider: 'amazon',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 5120,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      },
      {
        type: 'regional-eu',
        prefix: 'eu',
        regions: ['eu-central-1', 'eu-north-1', 'eu-south-1', 'eu-south-2', 'eu-west-3'],
        displaySuffix: '(EU)'
      },
      {
        type: 'regional-apac',
        prefix: 'apac',
        regions: [
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-south-1',
          'ap-southeast-1',
          'ap-southeast-2'
        ],
        displaySuffix: '(APAC)'
      }
    ],
    pricing: {
      input: 0.000035,
      output: 0.00014,
      cacheRead: 0.00000875,
      cacheWrite: 0
    },
    cache: {
      supported: true,
      cacheableFields: ['messages', 'system']
    }
  },

  // DeepSeek R1
  {
    baseId: 'r1-v1:0',
    name: 'DeepSeek R1',
    provider: 'deepseek',
    category: 'text',
    toolUse: false,
    maxTokensLimit: 32768,
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ]
  },

  // OpenAI GPT-6 Astra
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-6-astra` / `global.openai.gpt-6-astra`); on-demand
  // invocation of the bare model ID is not supported. OpenAI's most capable
  // model: complex reasoning, coding, computer use, research, document
  // creation. 1.05M context window, 128K max output.
  // Pricing: $11.00/$55.00 per 1M in/out (stored per 1K), Short Context Window
  // (272K) at the Geo CRIS rate. The (Global) profile is ~9% cheaper and input
  // beyond 272K bills at the Long Context tier (~2x), so displayed cost is a
  // floor rather than an exact figure.
  {
    baseId: 'gpt-6-astra',
    name: 'GPT-6 Astra',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.011,
      output: 0.055,
      cacheRead: 0.0011,
      cacheWrite: 0.01375
    }
  },

  // OpenAI GPT-6.1 Sol
  // Launched 2026-09-29: Sol-tier pricing with quality approaching GPT-6 Astra
  // on demanding evaluations, aimed at codebase investigation, document
  // understanding and multi-step computer-use workflows.
  // On bedrock-runtime, Converse is reachable only through the US geo
  // cross-region inference profile (`us.openai.gpt-6.1-sol`). Neither in-region
  // invocation of the bare model ID nor a global profile is offered for this
  // launch, so `regional-us` is the only profile declared.
  // 1M-token context window, 131,072 max output tokens, text and image input.
  // Explicit prompt caching is not supported, so no `cache` block is declared —
  // a cachePoint block would be rejected. Implicit caching is still billed, so
  // cache pricing is recorded.
  // Pricing (US CRIS short context, per 1M in/out/cache-read/cache-write):
  // $2.20/$11.00/$0.11/$2.75, stored per 1K. Input above 272K tokens bills at
  // the long-context tier (2x input, 1.5x output) for the whole request, so the
  // displayed cost is a floor rather than an exact figure on very long prompts.
  {
    baseId: 'gpt-6.1-sol',
    name: 'GPT-6.1 Sol',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 131072,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0022,
      output: 0.011,
      cacheRead: 0.00011,
      cacheWrite: 0.00275
    }
  },

  // OpenAI GPT-6 Sol
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-6-sol` / `global.openai.gpt-6-sol`); on-demand
  // invocation of the bare model ID is not supported. Mid-tier GPT-6: the
  // everyday reasoning/coding model below Astra.
  // Pricing: $2.00/$10.00 per 1M in/out, $0.20 cache read, $2.50 cache write
  // (stored per 1K).
  {
    baseId: 'gpt-6-sol',
    name: 'GPT-6 Sol',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.002,
      output: 0.01,
      cacheRead: 0.0002,
      cacheWrite: 0.0025
    }
  },

  // OpenAI GPT-6 Luna
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-6-luna` / `global.openai.gpt-6-luna`); on-demand
  // invocation of the bare model ID is not supported. Fast/affordable tier of
  // GPT-6: high-volume classification, summarization, routing.
  // Pricing: $0.10/$0.50 per 1M in/out, $0.01 cache read, $0.125 cache write
  // (stored per 1K).
  {
    baseId: 'gpt-6-luna',
    name: 'GPT-6 Luna',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0001,
      output: 0.0005,
      cacheRead: 0.00001,
      cacheWrite: 0.000125
    }
  },

  // OpenAI GPT-5.6 Sol
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-5.6-sol` / `global.openai.gpt-5.6-sol`); on-demand
  // invocation of the bare model ID is not supported. Most capable of the
  // GPT-5.6 family; frontier reasoning + agentic performance.
  // Pricing: $5.50/$33.00 per 1M in/out (stored per 1K).
  {
    baseId: 'gpt-5.6-sol',
    name: 'GPT-5.6 Sol',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0055,
      output: 0.033,
      cacheRead: 0.00055,
      cacheWrite: 0.00688
    }
  },

  // OpenAI GPT-5.6 Terra
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-5.6-terra` / `global.openai.gpt-5.6-terra`).
  // Balanced everyday model.
  // Pricing: $2.20/$13.20 per 1M in/out (stored per 1K).
  {
    baseId: 'gpt-5.6-terra',
    name: 'GPT-5.6 Terra',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0022,
      output: 0.0132,
      cacheRead: 0.00022,
      cacheWrite: 0.00275
    }
  },

  // OpenAI GPT-5.6 Luna
  // Invoked through the Bedrock Converse API via a cross-region inference
  // profile (`us.openai.gpt-5.6-luna` / `global.openai.gpt-5.6-luna`).
  // Fast/affordable; high-volume classification, summarization, routing.
  // Pricing: $0.22/$1.32 per 1M in/out (stored per 1K).
  {
    baseId: 'gpt-5.6-luna',
    name: 'GPT-5.6 Luna',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 128000,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.00022,
      output: 0.00132,
      cacheRead: 0.000022,
      cacheWrite: 0.000275
    }
  },

  // OpenAI GPT-OSS 120B
  // 16K output tokens per the AWS model card.
  {
    baseId: 'gpt-oss-120b-1:0',
    name: 'GPT-OSS 120B',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 16384,
    supportsThinking: false,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ],
    // Standard tier: $0.1545/$0.6180 per 1M in/out (stored per 1K).
    pricing: {
      input: 0.0001545,
      output: 0.000618,
      cacheRead: 0,
      cacheWrite: 0
    }
  },

  // OpenAI GPT-OSS 20B
  // 16K output tokens per the AWS model card.
  {
    baseId: 'gpt-oss-20b-1:0',
    name: 'GPT-OSS 20B',
    provider: 'openai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 16384,
    supportsThinking: false,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ],
    // Standard tier: $0.0721/$0.3090 per 1M in/out (stored per 1K).
    pricing: {
      input: 0.0000721,
      output: 0.000309,
      cacheRead: 0,
      cacheWrite: 0
    }
  },

  // Moonshot AI Kimi K3
  // Moonshot's open-weight flagship: native vision input and a 1M-token context
  // window. On bedrock-runtime it is reachable only through cross-region
  // inference profiles (`us.moonshotai.kimi-k3` / `global.moonshotai.kimi-k3`);
  // in-region invocation of the bare model ID is not offered, so no `base`
  // profile is declared.
  // The model reasons internally, but Converse returns an InternalServerException
  // when reasoning content from an earlier turn is sent back in a multi-turn
  // request, so `supportsThinking` stays false: that both keeps the
  // Anthropic-style `thinking` field out of the request and makes the chat layer
  // strip reasoningContent blocks from history before sending.
  // The model also rejects both sampling fields on Converse ("This model
  // doesn't support the temperature field" / "... the topP field"), so
  // converseService sends maxTokens alone for it.
  // Explicit prompt caching is offered only on the Responses and Chat
  // Completions APIs, not Converse, so no `cache` block is declared — a
  // cachePoint block would be rejected. Implicit caching still applies and is
  // billed, so cache-read pricing is recorded.
  // Pricing (US CRIS, per 1M in/out/cache-read): $3.30/$16.50/$0.33, stored per
  // 1K. Global CRIS bills about 10% less ($3.00/$15.00/$0.30) but the registry
  // keeps one rate per model, so the higher US rate is used.
  {
    baseId: 'kimi-k3',
    name: 'Kimi K3',
    provider: 'moonshotai',
    category: 'text',
    toolUse: true,
    // The model card documents no output ceiling, so this matches the documented
    // Kimi K2.5 cap as a conservative floor: selecting a model sets maxTokens to
    // this value, and an over-high guess would fail validation on every request.
    maxTokensLimit: 16384,
    supportsThinking: false,
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-1',
          'us-west-2',
          'ca-central-1',
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3',
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-northeast-3',
          'ap-south-1',
          'ap-south-2',
          'ap-southeast-1',
          'ap-southeast-2',
          'ap-southeast-3',
          'ap-southeast-4',
          'sa-east-1'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2', 'ca-central-1'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0033,
      output: 0.0165,
      cacheRead: 0.00033,
      cacheWrite: 0
    }
  },

  // Moonshot AI Kimi K2.5
  // In-region only (no cross-region/global inference profile), model ID has no
  // version suffix: invoked as `moonshotai.kimi-k2.5`.
  {
    baseId: 'kimi-k2.5',
    name: 'Kimi K2.5',
    provider: 'moonshotai',
    category: 'text',
    toolUse: true,
    maxTokensLimit: 16384,
    supportsThinking: false,
    inferenceProfiles: [
      {
        type: 'base',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-2',
          'eu-north-1',
          'eu-west-2',
          'ap-northeast-1',
          'ap-south-1',
          'ap-southeast-2',
          'ap-southeast-3',
          'ap-southeast-4',
          'sa-east-1'
        ]
      }
    ],
    pricing: {
      input: 0.0006,
      output: 0.003,
      cacheRead: 0,
      cacheWrite: 0
    }
  },

  // xAI Grok 4.6
  // xAI's frontier model for coding, agentic tasks and knowledge work, with a
  // 500K context window. Reasoning is always active; the effort level (low,
  // medium, high, xhigh) is set through `additionalModelRequestFields.reasoning`
  // rather than an Anthropic-style `thinking` field, and the model rejects
  // temperature/topP the way the GPT-5.x models do — see converseService.
  // On bedrock-runtime the model is only reachable through cross-region
  // inference profiles (`us.xai.grok-4.6` / `global.xai.grok-4.6`); in-region
  // invocation of the bare model ID is not supported, so no `base` profile.
  // Bedrock also lists structured output as unsupported for this model on
  // bedrock-runtime, so structured-output requests may fail with it selected.
  // Only implicit prompt caching is offered, so no `cache` block is declared:
  // explicit cachePoint blocks would be rejected. Cache-read pricing is still
  // recorded because implicit hits are billed and reported in usage.
  // Pricing (Geo/US CRIS, per 1M in/out/cache-read): $2.20/$6.60/$0.55, stored
  // per 1K. Global CRIS bills about 10% less ($2.00/$6.00/$0.50) but the
  // registry keeps one rate per model, so the higher US rate is used.
  {
    baseId: 'grok-4.6',
    name: 'Grok 4.6',
    provider: 'xai',
    category: 'text',
    toolUse: true,
    // The model card documents no output cap, so this is a conservative floor:
    // selecting a model sets maxTokens to this value, and an over-high guess
    // would make every request fail validation.
    maxTokensLimit: 32768,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-1',
          'us-west-2',
          'ca-central-1',
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3',
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-northeast-3',
          'ap-south-1',
          'ap-south-2',
          'ap-southeast-1',
          'ap-southeast-2',
          'sa-east-1'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.0022,
      output: 0.0066,
      cacheRead: 0.00055,
      cacheWrite: 0
    }
  },

  // xAI Grok 4.7
  // xAI's frontier model (launched 2026-09-28), built on Grok 4.6 with a focus on
  // long-running agents and more ambitious interactive work. Same 500K context
  // window, text and image input, and the same always-on reasoning with a
  // configurable effort level (low, medium, high, xhigh) set through
  // `additionalModelRequestFields.reasoning`; it rejects temperature/topP the way
  // Grok 4.6 and the GPT-5.x models do — see converseService.
  // Reachable only through cross-region inference profiles on bedrock-runtime
  // (`us.xai.grok-4.7` / `global.xai.grok-4.7`), so no `base` profile. Unlike
  // Grok 4.6, Bedrock lists structured output as supported for this model.
  // Only implicit prompt caching is offered, so no `cache` block is declared:
  // explicit cachePoint blocks would be rejected. Cache-read pricing is still
  // recorded because implicit hits are billed and reported in usage.
  // Pricing (Global CRIS, per 1M in/out/cache-read): $2.00/$6.00/$0.50, stored
  // per 1K. Geo/US CRIS bills about 10% more ($2.20/$6.60/$0.55) but the registry
  // keeps one rate per model, so the Global rate is used.
  {
    baseId: 'grok-4.7',
    name: 'Grok 4.7',
    provider: 'xai',
    category: 'text',
    toolUse: true,
    // The model card documents no output cap, so this is a conservative floor,
    // matching Grok 4.6: selecting a model sets maxTokens to this value, and an
    // over-high guess would make every request fail validation.
    maxTokensLimit: 32768,
    supportsThinking: true,
    supportedThinkingTypes: ['enabled'],
    inferenceProfiles: [
      {
        type: 'global',
        prefix: 'global',
        regions: [
          'us-east-1',
          'us-east-2',
          'us-west-1',
          'us-west-2',
          'ca-central-1',
          'eu-central-1',
          'eu-central-2',
          'eu-north-1',
          'eu-south-1',
          'eu-south-2',
          'eu-west-1',
          'eu-west-2',
          'eu-west-3',
          'ap-northeast-1',
          'ap-northeast-2',
          'ap-northeast-3',
          'ap-south-1',
          'ap-south-2',
          'ap-southeast-1',
          'ap-southeast-2',
          'ap-southeast-3',
          'ap-southeast-4',
          'sa-east-1'
        ],
        displaySuffix: '(Global)'
      },
      {
        type: 'regional-us',
        prefix: 'us',
        regions: ['us-east-1', 'us-east-2', 'us-west-1', 'us-west-2'],
        displaySuffix: '(US)'
      }
    ],
    pricing: {
      input: 0.002,
      output: 0.006,
      cacheRead: 0.0005,
      cacheWrite: 0
    }
  }

  // Custom model (this is example)
  // {
  //   baseId: 'arn:aws:bedrock:us-east-1:1234567890:imported-model/xxxx',
  //   name: 'DeepSeek-R1-Distill-Llama-8B',
  //   provider: 'deepseek',
  //   category: 'text',
  //   toolUse: true,
  //   maxTokensLimit: 4096,
  //   supportsStreamingToolUse: false,
  //   inferenceProfiles: [
  //     {
  //       type: 'base',
  //       regions: ['us-east-1']
  //     }
  //   ]
  // }
]

/**
 * Image generation model registry
 */
const IMAGE_GENERATION_MODELS: ModelConfig[] = [
  // Stability AI models
  {
    baseId: 'stability.sd3-5-large-v1:0',
    name: 'Stability SD3.5 Large',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  {
    baseId: 'stability.sd3-large-v1:0',
    name: 'Stability SD3 Large',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  {
    baseId: 'stability.stable-image-core-v1:0',
    name: 'Stability Stable Image Core v1.0',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  {
    baseId: 'stability.stable-image-core-v1:1',
    name: 'Stability Stable Image Core v1.1',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  {
    baseId: 'stability.stable-image-ultra-v1:0',
    name: 'Stability Stable Image Ultra v1.0',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  {
    baseId: 'stability.stable-image-ultra-v1:1',
    name: 'Stability Stable Image Ultra v1.1',
    provider: 'stability',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-west-2']
      }
    ]
  },
  // Amazon models
  {
    baseId: 'amazon.nova-canvas-v1:0',
    name: 'Amazon Nova Canvas',
    provider: 'amazon',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-east-1', 'ap-northeast-1', 'eu-west-1']
      }
    ]
  },
  {
    baseId: 'amazon.titan-image-generator-v2:0',
    name: 'Amazon Titan Image Generator v2',
    provider: 'amazon',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-east-1', 'us-west-2']
      }
    ]
  },
  {
    baseId: 'amazon.titan-image-generator-v1',
    name: 'Amazon Titan Image Generator v1',
    provider: 'amazon',
    category: 'image',
    toolUse: false,
    maxTokensLimit: 0,
    inferenceProfiles: [
      {
        type: 'base',
        regions: ['us-east-1', 'us-west-2', 'eu-west-1', 'eu-west-2', 'ap-south-1']
      }
    ]
  }
]

/**
 * Check if model ID is in ARN format
 */
function isArnModelId(modelId: string): boolean {
  return modelId.startsWith('arn:aws:bedrock:')
}

/**
 * Remove region prefix from model ID to get base model name
 * Example: 'us.anthropic.claude-3-7-sonnet-20250219-v1:0' → 'anthropic.claude-3-7-sonnet-20250219-v1:0'
 */
export function getBaseModelId(modelId: string): string {
  // Region prefix pattern: specific region codes (e.g., 'us.', 'eu.', 'apac.', 'jp.', 'global.')
  const regionPrefixPattern = /^(us|eu|apac|jp|global)\./
  return modelId.replace(regionPrefixPattern, '')
}

/**
 * Generate full model ID from model configuration
 */
function generateFullModelId(config: ModelConfig, profile: InferenceProfile): string {
  // Return as-is if model ID is in ARN format
  if (isArnModelId(config.baseId)) {
    return config.baseId
  }

  // Add prefix if it exists (for cross-region inference profile)
  if (profile.prefix) {
    return `${profile.prefix}.${config.provider}.${config.baseId}`
  }

  // No prefix for base type
  return `${config.provider}.${config.baseId}`
}

/**
 * Create LLM object from model configuration
 */
function createLLMFromConfig(config: ModelConfig, profile: InferenceProfile): LLM {
  const modelId = generateFullModelId(config, profile)
  const modelName = profile.displaySuffix ? `${config.name} ${profile.displaySuffix}` : config.name

  return {
    modelId,
    modelName,
    toolUse: config.toolUse,
    maxTokensLimit: config.maxTokensLimit,
    supportsThinking: config.supportsThinking,
    supportedThinkingTypes: config.supportedThinkingTypes,
    regions: profile.regions
  }
}

/**
 * Generate all LLM objects from model configurations
 */
function generateModelsFromConfigs(): LLM[] {
  const models: LLM[] = []

  // Process only text models
  const textModels = MODEL_REGISTRY.filter((config) => config.category === 'text')

  textModels.forEach((config) => {
    config.inferenceProfiles.forEach((profile) => {
      models.push(createLLMFromConfig(config, profile))
    })
  })

  return models
}

// Generated model list
export const allModels = generateModelsFromConfigs()

/**
 * Get models by region
 */
export const getModelsForRegion = (region: BedrockSupportRegion): LLM[] => {
  const models = allModels.filter((model) => model.regions?.includes(region))
  return models.sort((a, b) => a.modelName.localeCompare(b.modelName))
}

/**
 * Get list of model IDs that support Thinking
 */
export const getThinkingSupportedModelIds = (): string[] => {
  return allModels.filter((model) => model.supportsThinking === true).map((model) => model.modelId)
}

/**
 * Get supported thinking types for a model ID.
 * Returns the thinking API types the model accepts ('enabled' and/or 'adaptive').
 */
export const getSupportedThinkingTypes = (modelId: string): ThinkingType[] => {
  const config = getModelConfig(modelId)
  return config?.supportedThinkingTypes || []
}

/**
 * Get image generation models by region
 */
export const getImageGenerationModelsForRegion = (region: BedrockSupportRegion) => {
  const models: Array<{ id: string; name: string }> = []

  IMAGE_GENERATION_MODELS.forEach((config) => {
    // Find inference profiles that include the specified region
    const hasRegion = config.inferenceProfiles.some((profile) => profile.regions.includes(region))

    if (hasRegion) {
      models.push({
        id: config.baseId,
        name: config.name
      })
    }
  })

  return models.sort((a, b) => {
    // Provider order: Amazon → Stability
    const providerOrderA = a.id.startsWith('amazon') ? 0 : 1
    const providerOrderB = b.id.startsWith('amazon') ? 0 : 1

    if (providerOrderA !== providerOrderB) {
      return providerOrderA - providerOrderB
    }

    // Within same provider, sort by name
    return a.name.localeCompare(b.name)
  })
}

/**
 * Model utility functions
 */
export const getModelMaxTokens = (modelId: string): number => {
  // Try exact match first
  let model = allModels.find((m) => m.modelId === modelId)

  // Try partial match if exact match not found
  if (!model) {
    model = allModels.find((m) => m.modelId.includes(modelId) || modelId.includes(m.modelId))
  }

  return model?.maxTokensLimit || 8192 // Default value
}

// =========================
// Pricing-related functions
// =========================

/**
 * Get model configuration
 *
 * Matching is by substring so that a bare or prefixed model ID both resolve, but
 * one base ID can be a prefix of another — `claude-opus-5` of `claude-opus-5-5`,
 * `claude-fable-5` of `claude-fable-5-1` — so the longest matching base ID wins
 * rather than whichever entry comes first in the registry.
 */
export const getModelConfig = (modelId: string): ModelConfig | undefined => {
  const baseModelId = getBaseModelId(modelId)
  return MODEL_REGISTRY.filter(
    (c) => baseModelId.includes(c.baseId) || baseModelId.includes(`${c.provider}.${c.baseId}`)
  ).sort((a, b) => b.baseId.length - a.baseId.length)[0]
}

/**
 * Resolve the maxTokens value to send for a model: the lesser of what the
 * caller asked for and the model's own output ceiling.
 *
 * Max Output Tokens is a single global setting, but the ceiling is per model —
 * 128000 is valid for Opus 5 and rejected by Haiku 4.5, which stops at 64000.
 * Rather than making the user re-tune the setting per model, every request is
 * clamped here.
 *
 * Models the registry does not know (custom inference profile ARNs, imported
 * models) have no ceiling to clamp against, so their requested value is passed
 * through untouched.
 */
export const clampMaxTokensToModelLimit = (
  modelId: string,
  requestedMaxTokens: number | undefined
): number | undefined => {
  if (typeof requestedMaxTokens !== 'number') return requestedMaxTokens
  const limit = getModelConfig(modelId)?.maxTokensLimit
  if (!limit) return requestedMaxTokens
  return Math.min(requestedMaxTokens, limit)
}

/**
 * Check if model supports streaming with Tool Use
 */
export const supportsStreamingWithToolUse = (modelId: string): boolean => {
  const config = getModelConfig(modelId)
  // Return false only if supportsStreamingToolUse is explicitly false
  // If undefined, assume true (supported) by default
  return config?.supportsStreamingToolUse !== false
}
