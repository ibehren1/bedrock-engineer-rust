// The usage ledger behind the Accounting page: the shapes of `window.api.usage`
// (Rust: src-tauri/crates/usage/src/lib.rs) and the `usageContext` the renderer attaches to
// Bedrock calls (src-tauri/crates/bedrock/src/usage.rs).

/** What a Bedrock call is for, recorded with its usage. */
export type UsageFeature =
  | 'chat'
  | 'website'
  | 'diagram'
  | 'stepfunctions'
  | 'title'
  | 'agent-editor'
  | 'structured'
  | 'background'
  | 'subagent'
  | 'kb-ingest'
  | 'kb-search'
  | `tool:${string}`
  | 'other'

export type UsageContext = {
  feature: UsageFeature
  agentId?: string
  sessionId?: string
}

export type UsageGroupBy = 'day' | 'month' | 'model' | 'feature' | 'service' | 'agent'

/** An inclusive range of local days (`YYYY-MM-DD`) with optional filters. */
export type UsageQuery = {
  from: string
  to: string
  modelId?: string
  feature?: string
  /** `''` matches calls made without an agent. */
  agentId?: string
}

export type UsageSummaryRow = {
  /** The day, month, model id, feature or service. */
  key: string
  /** The model's display name when grouping by model; otherwise the key. */
  label: string
  calls: number
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  /** Estimated dollars, over priced calls only. */
  costUsd: number
  /** Calls whose model has no known price. */
  unpricedCalls: number
  /** Calls whose token counts were estimated (Cohere embeddings). */
  estimatedCalls: number
  /** Calls imported from chat history (made before recording began; chat only). */
  importedCalls: number
  /** Units of the non-token services. */
  images: number
  videoSeconds: number
  characters: number
  guardrailTextUnits: number
  /** What prompt caching saved (negative when cache writes cost more than reads saved). */
  cacheSavingsUsd: number
}

/** A chat or background-agent session, optionally a time window of it (Unix ms, `fromTs` exclusive). */
export type SessionWindow = { sessionId: string; fromTs?: number; toTs?: number }

export type SessionCost = { sessionId: string; calls: number; costUsd: number }

export type UsageService =
  | 'converse'
  | 'embed'
  | 'image'
  | 'video'
  | 'agent'
  | 'guardrail'
  | 'translate'
  | 'retrieve'
  | 'flow'

export type UsageUnitKind = 'images' | 'video_seconds' | 'characters' | 'guardrail_text_units'

/** Cost of one (group, sub-group) pair, e.g. one model on one day. */
export type UsageBreakdownRow = {
  key: string
  subKey: string
  subLabel: string
  calls: number
  costUsd: number
}

/** Store key `usageBudget`. */
export type UsageBudget = {
  /** Dollars per month; `null` for no budget. */
  monthlyUsd: number | null
  /** Alert thresholds as fractions of the budget (default 0.8 and 1). */
  alertAt: number[]
}

export type UsageBudgetStatus = {
  /** `YYYY-MM`, local time. */
  month: string
  spentUsd: number
  budgetUsd: number | null
  fraction: number | null
  /** Spend so far extended linearly to the end of the month. */
  projectedUsd: number
  overBudget: boolean
}

export type UsageInfo = {
  /** Unix ms from which calls are recorded live; earlier rows were imported from chat history. */
  liveSince: number
  historyImported: boolean
}

export type UsageEventRow = {
  id: number
  /** Unix ms. */
  ts: number
  localDay: string
  modelId: string
  modelName: string
  rawModelId: string
  region: string
  service: UsageService
  feature: string
  agentId?: string | null
  sessionId?: string | null
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  costUsd: number | null
  estimated: boolean
  source: 'live' | 'backfill'
  units?: number | null
  unitKind?: UsageUnitKind | null
}

export type UsageApi = {
  summary: (query: UsageQuery, groupBy: UsageGroupBy) => Promise<UsageSummaryRow[]>
  breakdown: (
    query: UsageQuery,
    by: UsageGroupBy,
    then: UsageGroupBy
  ) => Promise<UsageBreakdownRow[]>
  info: () => Promise<UsageInfo>
  /** Cost of each session or session window, in order (any full-UI window may call this). */
  sessionCosts: (sessions: SessionWindow[]) => Promise<SessionCost[]>
  /** This month against the budget (also sends any alert now due). */
  budgetStatus: () => Promise<UsageBudgetStatus>
  /** Save the matching calls as CSV via a save dialog; the path, or `null` when canceled. */
  exportCsv: (query: UsageQuery, fileName?: string) => Promise<string | null>
  /** Months (`YYYY-MM`) with recorded usage, newest first. */
  months: () => Promise<string[]>
  events: (query: UsageQuery, limit?: number, offset?: number) => Promise<UsageEventRow[]>
  /** Fires after newly recorded usage is written. */
  onChanged: (callback: () => void) => () => void
}
