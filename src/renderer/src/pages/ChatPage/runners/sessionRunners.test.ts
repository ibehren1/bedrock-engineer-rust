import { describe, expect, test, jest } from '@jest/globals'
import type { IdentifiableMessage } from '@/types/chat/message'
import {
  abortRun,
  getRunState,
  getRunningSessionIds,
  isSessionRunning,
  patchRunState,
  releaseRunner,
  seedRunMessages,
  setAbortController,
  setRunMessages,
  subscribeToRunState,
  subscribeToRunningSessions,
  updateRunField
} from './sessionRunners'

const msg = (text: string): IdentifiableMessage => ({
  id: text,
  role: 'assistant',
  content: [{ text }]
})

// The registry is module state shared by every test in this file, so each test uses its own keys.
describe('sessionRunners', () => {
  test('sessions keep independent state', () => {
    setRunMessages('a1', [msg('one')])
    patchRunState('a1', { loading: true })
    setRunMessages('a2', [msg('two'), msg('three')])

    expect(getRunState('a1').messages.map((m) => m.id)).toEqual(['one'])
    expect(getRunState('a1').loading).toBe(true)
    expect(getRunState('a2').messages).toHaveLength(2)
    expect(getRunState('a2').loading).toBe(false)
  })

  test('an unknown session reads as empty with a stable identity', () => {
    // A new object each call would make useSyncExternalStore re-render forever.
    expect(getRunState('never-used')).toBe(getRunState('also-never-used'))
    expect(getRunState(undefined).messages).toEqual([])
  })

  test('the snapshot identity only changes when something changes', () => {
    setRunMessages('b1', [msg('one')])
    const before = getRunState('b1')
    expect(getRunState('b1')).toBe(before)

    patchRunState('b1', { reasoning: true })
    expect(getRunState('b1')).not.toBe(before)
  })

  test('subscribers are notified for their own session only', () => {
    const onC1 = jest.fn()
    const onC2 = jest.fn()
    const unsubscribe = subscribeToRunState('c1', onC1)
    subscribeToRunState('c2', onC2)

    setRunMessages('c1', [msg('one')])
    expect(onC1).toHaveBeenCalledTimes(1)
    expect(onC2).not.toHaveBeenCalled()

    unsubscribe()
    setRunMessages('c1', [msg('two')])
    expect(onC1).toHaveBeenCalledTimes(1)
  })

  test('seeding from the store is skipped while a turn is in flight', () => {
    // Opening a chat re-reads it from disk; a running turn's messages are newer than that.
    setRunMessages('d1', [msg('streamed')])
    patchRunState('d1', { loading: true })
    seedRunMessages('d1', [msg('stale')])
    expect(getRunState('d1').messages.map((m) => m.id)).toEqual(['streamed'])

    patchRunState('d1', { loading: false })
    seedRunMessages('d1', [msg('from-store')])
    expect(getRunState('d1').messages.map((m) => m.id)).toEqual(['from-store'])
  })

  test('seeding clears leftover run flags', () => {
    patchRunState('d2', { reasoning: true, latestReasoningText: 'hmm', timeoutCountdown: 12 })
    seedRunMessages('d2', [msg('one')])
    expect(getRunState('d2')).toMatchObject({
      reasoning: false,
      latestReasoningText: '',
      timeoutCountdown: 0
    })
  })

  test('updateRunField derives from the previous value', () => {
    updateRunField('e1', 'executingTools', (prev) => new Set([...prev, 'readFiles' as never]))
    updateRunField('e1', 'executingTools', (prev) => new Set([...prev, 'writeToFile' as never]))
    expect([...getRunState('e1').executingTools]).toEqual(['readFiles', 'writeToFile'])
  })

  test('abortRun aborts the request and marks the session idle', () => {
    const controller = new AbortController()
    setAbortController('f1', controller)
    patchRunState('f1', { loading: true, executingTools: new Set(['readFiles' as never]) })

    abortRun('f1')

    expect(controller.signal.aborted).toBe(true)
    expect(isSessionRunning('f1')).toBe(false)
    expect(getRunState('f1').executingTools.size).toBe(0)
  })

  test('abortRun leaves other sessions running', () => {
    const running = new AbortController()
    setAbortController('f2', running)
    patchRunState('f2', { loading: true })
    setAbortController('f3', new AbortController())
    patchRunState('f3', { loading: true })

    abortRun('f3')

    expect(running.signal.aborted).toBe(false)
    expect(isSessionRunning('f2')).toBe(true)
  })

  test('the running set tracks loading sessions and notifies subscribers', () => {
    const onChange = jest.fn()
    const unsubscribe = subscribeToRunningSessions(onChange)
    const before = getRunningSessionIds()

    patchRunState('g1', { loading: true })
    expect(getRunningSessionIds()).toContain('g1')
    expect(onChange).toHaveBeenCalled()

    // Identity is stable while the set is unchanged, so the indicator doesn't churn.
    const snapshot = getRunningSessionIds()
    patchRunState('g1', { reasoning: true })
    expect(getRunningSessionIds()).toBe(snapshot)

    patchRunState('g1', { loading: false })
    expect(getRunningSessionIds()).not.toContain('g1')
    unsubscribe()
    expect(before).not.toContain('g1')
  })

  test('releaseRunner drops idle unwatched sessions but keeps running or watched ones', () => {
    setRunMessages('h1', [msg('one')])
    releaseRunner('h1')
    expect(getRunState('h1').messages).toEqual([])

    setRunMessages('h2', [msg('one')])
    patchRunState('h2', { loading: true })
    releaseRunner('h2')
    expect(getRunState('h2').messages).toHaveLength(1)

    setRunMessages('h3', [msg('one')])
    subscribeToRunState('h3', () => {})
    releaseRunner('h3')
    expect(getRunState('h3').messages).toHaveLength(1)
  })
})
