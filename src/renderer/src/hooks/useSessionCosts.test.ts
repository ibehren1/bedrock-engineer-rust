import { describe, expect, it } from '@jest/globals'
import { runWindows } from './useSessionCosts'

describe('runWindows', () => {
  it('starts each run where the previous run of the same session ended', () => {
    const runs = [
      { sessionId: 'a', executedAt: 300 },
      { sessionId: 'b', executedAt: 200 },
      { sessionId: 'a', executedAt: 100 }
    ]
    expect(runWindows(runs)).toEqual([
      { sessionId: 'a', fromTs: 100, toTs: 300 },
      { sessionId: 'b', fromTs: undefined, toTs: 200 },
      { sessionId: 'a', fromTs: undefined, toTs: 100 }
    ])
  })
})
