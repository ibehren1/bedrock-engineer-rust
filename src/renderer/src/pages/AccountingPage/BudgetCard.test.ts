import { describe, expect, it } from '@jest/globals'
import { formatThresholds, parseThresholds } from './BudgetCard'

describe('budget thresholds', () => {
  it('parses percentages into sorted fractions', () => {
    expect(parseThresholds('80, 100')).toEqual([0.8, 1])
    expect(parseThresholds('100 50% 80')).toEqual([0.5, 0.8, 1])
    expect(parseThresholds('80, 80')).toEqual([0.8])
  })

  it('rejects empty or invalid input', () => {
    expect(parseThresholds('')).toBeNull()
    expect(parseThresholds('80, abc')).toBeNull()
    expect(parseThresholds('0')).toBeNull()
  })

  it('formats fractions back to percentages', () => {
    expect(formatThresholds([0.8, 1])).toBe('80, 100')
  })
})
