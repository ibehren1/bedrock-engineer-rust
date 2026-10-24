import { clampNumber, parseInRange } from './NumberInput'

describe('number input parsing', () => {
  it('accepts only whole numbers in range while typing', () => {
    expect(parseInRange('', 0, 10)).toBeUndefined()
    expect(parseInRange('3', 0, 10)).toBe(3)
    expect(parseInRange('11', 0, 10)).toBeUndefined()
    expect(parseInRange('0', 1, 1000)).toBeUndefined()
    expect(parseInRange('abc', 0, 10)).toBeUndefined()
  })

  it('clamps on commit and keeps the old value when empty', () => {
    expect(clampNumber('', 1, 1000, 50)).toBe(50)
    expect(clampNumber('5000', 1, 1000, 50)).toBe(1000)
    expect(clampNumber('0', 1, 1000, 50)).toBe(1)
    expect(clampNumber('7.6', 0, 10, 2)).toBe(8)
  })
})
