import type { UsageBreakdownRow, UsageSummaryRow } from '@/types/usage'
import { monthRange } from './accountingUtils'

/** Models charted as their own series; the rest fold into "Other". */
export const MAX_MODEL_SERIES = 6

/** Key of the folded series. */
export const OTHER = '__other__'

/** Every local day of `month` (`YYYY-MM-DD`), in order. */
export function monthDays(month: string): string[] {
  const { to } = monthRange(month)
  const last = Number(to.slice(8))
  return Array.from({ length: last }, (_, i) => `${month}-${String(i + 1).padStart(2, '0')}`)
}

/** Every month from `first` to `last` (`YYYY-MM`), inclusive. */
export function monthSpan(first: string, last: string): string[] {
  const out: string[] = []
  let [y, m] = first.split('-').map(Number)
  const [ly, lm] = last.split('-').map(Number)
  while (y < ly || (y === ly && m <= lm)) {
    out.push(`${y}-${String(m).padStart(2, '0')}`)
    m += 1
    if (m > 12) {
      m = 1
      y += 1
    }
  }
  return out
}

/**
 * Which models get their own series (and color slot): the month's costliest, in cost order.
 * Computed from the unfiltered month so a filter never repaints the models left on screen.
 */
export function modelSlots(models: UsageSummaryRow[]): Map<string, number> {
  const ranked = [...models].sort((a, b) => b.costUsd - a.costUsd || a.label.localeCompare(b.label))
  return new Map(ranked.slice(0, MAX_MODEL_SERIES).map((m, i) => [m.key, i]))
}

export type Series = { key: string; label: string; slot: number | null; data: number[] }

/** Per-day cost stacked by model: one series per slotted model, then "Other". */
export function dailyByModel(
  days: string[],
  rows: UsageBreakdownRow[],
  slots: Map<string, number>,
  otherLabel: string
): Series[] {
  const index = new Map(days.map((d, i) => [d, i]))
  const series = new Map<string, Series>()
  const other: Series = { key: OTHER, label: otherLabel, slot: null, data: days.map(() => 0) }
  for (const r of rows) {
    const i = index.get(r.key)
    if (i === undefined) continue
    const slot = slots.get(r.subKey)
    let s = other
    if (slot !== undefined) {
      s = series.get(r.subKey) ?? {
        key: r.subKey,
        label: r.subLabel,
        slot,
        data: days.map(() => 0)
      }
      series.set(r.subKey, s)
    }
    s.data[i] += r.costUsd
  }
  const out = [...series.values()].sort((a, b) => (a.slot ?? 0) - (b.slot ?? 0))
  if (other.data.some((v) => v > 0)) out.push(other)
  return out
}

/** Cost per month over `months`, zero where nothing was recorded. */
export function monthlyCost(months: string[], rows: UsageSummaryRow[]): number[] {
  const byMonth = new Map(rows.map((r) => [r.key, r.costUsd]))
  return months.map((m) => byMonth.get(m) ?? 0)
}
