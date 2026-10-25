import { describe, expect, it } from '@jest/globals'
import type { UsageSummaryRow } from '@/types/usage'
import {
  byCost,
  formatUsd,
  monthOf,
  monthRange,
  pickerMonths,
  previousMonth,
  share,
  totals,
  unitAmounts
} from './accountingUtils'

const row = (key: string, costUsd: number, calls = 1): UsageSummaryRow => ({
  key,
  label: key,
  calls,
  inputTokens: 10 * calls,
  outputTokens: 5 * calls,
  cacheReadTokens: 0,
  cacheWriteTokens: 0,
  costUsd,
  unpricedCalls: costUsd === 0 ? calls : 0,
  estimatedCalls: 0,
  importedCalls: 0,
  images: 0,
  videoSeconds: 0,
  characters: 0,
  guardrailTextUnits: 0,
  cacheSavingsUsd: 0
})

describe('accountingUtils', () => {
  it('works with local months', () => {
    expect(monthOf(new Date(2026, 0, 31))).toBe('2026-01')
    expect(previousMonth('2026-01')).toBe('2025-12')
    expect(previousMonth('2026-10')).toBe('2026-09')
    expect(monthRange('2026-02')).toEqual({ from: '2026-02-01', to: '2026-02-28' })
    expect(monthRange('2028-02').to).toBe('2028-02-29')
    expect(monthRange('2026-10').to).toBe('2026-10-31')
  })

  it('always offers the current month, newest first', () => {
    expect(pickerMonths(['2026-08', '2026-10'], '2026-10')).toEqual(['2026-10', '2026-08'])
    expect(pickerMonths([], '2026-10')).toEqual(['2026-10'])
  })

  it('formats small costs with more digits', () => {
    expect(formatUsd(12.345)).toBe('$12.35')
    expect(formatUsd(0.0042)).toBe('$0.0042')
    expect(formatUsd(0)).toBe('$0.00')
  })

  it('totals and sorts rows', () => {
    const rows = [row('a', 1, 2), row('b', 3), row('c', 0)]
    expect(totals(rows)).toMatchObject({ calls: 4, inputTokens: 40, costUsd: 4, unpricedCalls: 1 })
    expect(byCost(rows).map((r) => r.key)).toEqual(['b', 'a', 'c'])
    expect(share(1, 4)).toBe('25%')
    expect(share(0.001, 4)).toBe('<1%')
    expect(share(1, 0)).toBe('—')
  })

  it('lists only the units a row used', () => {
    expect(unitAmounts({ ...row('a', 1), images: 2, characters: 10 })).toEqual([
      ['images', 2],
      ['characters', 10]
    ])
    expect(unitAmounts(row('a', 1))).toEqual([])
  })
})
