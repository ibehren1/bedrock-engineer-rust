import { describe, expect, it } from '@jest/globals'
import type { UsageBreakdownRow, UsageSummaryRow } from '@/types/usage'
import { dailyByModel, modelSlots, monthDays, monthSpan, monthlyCost, OTHER } from './chartData'

const model = (key: string, costUsd: number): UsageSummaryRow => ({
  key,
  label: key.toUpperCase(),
  calls: 1,
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheWriteTokens: 0,
  costUsd,
  unpricedCalls: 0,
  estimatedCalls: 0,
  importedCalls: 0,
  images: 0,
  videoSeconds: 0,
  characters: 0,
  guardrailTextUnits: 0,
  cacheSavingsUsd: 0
})

const cell = (key: string, subKey: string, costUsd: number): UsageBreakdownRow => ({
  key,
  subKey,
  subLabel: subKey.toUpperCase(),
  calls: 1,
  costUsd
})

describe('chartData', () => {
  it('lists days and months', () => {
    expect(monthDays('2026-02')).toHaveLength(28)
    expect(monthDays('2026-10')[30]).toBe('2026-10-31')
    expect(monthSpan('2025-11', '2026-02')).toEqual(['2025-11', '2025-12', '2026-01', '2026-02'])
    expect(monthSpan('2026-03', '2026-03')).toEqual(['2026-03'])
  })

  it('slots the costliest models and folds the rest into Other', () => {
    const models = ['a', 'b', 'c', 'd', 'e', 'f', 'g'].map((k, i) => model(k, 10 - i))
    const slots = modelSlots(models)
    expect(slots.get('a')).toBe(0)
    expect(slots.get('f')).toBe(5)
    expect(slots.has('g')).toBe(false)

    const days = ['2026-10-01', '2026-10-02']
    const series = dailyByModel(
      days,
      [cell('2026-10-01', 'b', 2), cell('2026-10-01', 'g', 1), cell('2026-10-02', 'g', 3)],
      slots,
      'Other'
    )
    expect(series.map((s) => [s.key, s.slot, s.data])).toEqual([
      ['b', 1, [2, 0]],
      [OTHER, null, [1, 3]]
    ])
  })

  it('keeps colors when a filter removes models', () => {
    const slots = modelSlots([model('a', 5), model('b', 4)])
    const series = dailyByModel(['2026-10-01'], [cell('2026-10-01', 'b', 1)], slots, 'Other')
    expect(series[0].slot).toBe(1)
  })

  it('fills months with no usage', () => {
    expect(monthlyCost(['2026-08', '2026-09', '2026-10'], [model('2026-10', 4)])).toEqual([0, 0, 4])
  })
})
