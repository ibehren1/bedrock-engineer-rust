import type { UsageQuery, UsageSummaryRow } from '@/types/usage'

/** `YYYY-MM` of a local date. */
export function monthOf(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
}

/** The month before `month` (`YYYY-MM`). */
export function previousMonth(month: string): string {
  const [y, m] = month.split('-').map(Number)
  return monthOf(new Date(y, m - 2, 1))
}

/** Every local day of `month`, inclusive. */
export function monthRange(month: string): UsageQuery {
  const [y, m] = month.split('-').map(Number)
  const last = new Date(y, m, 0).getDate()
  return { from: `${month}-01`, to: `${month}-${String(last).padStart(2, '0')}` }
}

/** Months to offer in the picker: the ones with data plus the current month, newest first. */
export function pickerMonths(withData: string[], current: string): string[] {
  return [...new Set([current, ...withData])].sort().reverse()
}

/** Dollars with enough digits that small costs don't all read $0.00. */
export function formatUsd(value: number): string {
  const digits = value !== 0 && Math.abs(value) < 0.01 ? 4 : 2
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: digits,
    maximumFractionDigits: digits
  }).format(value)
}

export function formatTokens(value: number): string {
  return new Intl.NumberFormat('en-US').format(value)
}

export type UsageTotals = Pick<
  UsageSummaryRow,
  | 'calls'
  | 'inputTokens'
  | 'outputTokens'
  | 'cacheReadTokens'
  | 'cacheWriteTokens'
  | 'costUsd'
  | 'unpricedCalls'
  | 'estimatedCalls'
  | 'importedCalls'
  | 'images'
  | 'videoSeconds'
  | 'characters'
  | 'guardrailTextUnits'
  | 'cacheSavingsUsd'
>

export function totals(rows: UsageSummaryRow[]): UsageTotals {
  const sum = (pick: (r: UsageSummaryRow) => number) => rows.reduce((n, r) => n + pick(r), 0)
  return {
    calls: sum((r) => r.calls),
    inputTokens: sum((r) => r.inputTokens),
    outputTokens: sum((r) => r.outputTokens),
    cacheReadTokens: sum((r) => r.cacheReadTokens),
    cacheWriteTokens: sum((r) => r.cacheWriteTokens),
    costUsd: sum((r) => r.costUsd),
    unpricedCalls: sum((r) => r.unpricedCalls),
    estimatedCalls: sum((r) => r.estimatedCalls),
    importedCalls: sum((r) => r.importedCalls),
    images: sum((r) => r.images),
    videoSeconds: sum((r) => r.videoSeconds),
    characters: sum((r) => r.characters),
    guardrailTextUnits: sum((r) => r.guardrailTextUnits),
    cacheSavingsUsd: sum((r) => r.cacheSavingsUsd)
  }
}

/** The non-token units in `row` as `[i18n key, amount]` pairs (only the non-zero ones). */
export function unitAmounts(
  row: Pick<UsageSummaryRow, 'images' | 'videoSeconds' | 'characters' | 'guardrailTextUnits'>
): [string, number][] {
  const pairs: [string, number][] = [
    ['images', row.images],
    ['video_seconds', row.videoSeconds],
    ['characters', row.characters],
    ['guardrail_text_units', row.guardrailTextUnits]
  ]
  return pairs.filter(([, n]) => n > 0)
}

/** Highest cost first (ties by name), for the model and feature tables. */
export function byCost(rows: UsageSummaryRow[]): UsageSummaryRow[] {
  return [...rows].sort((a, b) => b.costUsd - a.costUsd || a.label.localeCompare(b.label))
}

/** Share of `part` in `whole` as a whole-number percent string (`—` when there is no total). */
export function share(part: number, whole: number): string {
  if (whole <= 0) return '—'
  const pct = (part / whole) * 100
  return pct > 0 && pct < 1 ? '<1%' : `${Math.round(pct)}%`
}
