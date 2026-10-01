import {
  clearHostApproval,
  getPendingHostApproval,
  hasSessionHostApproval,
  requestHostApproval,
  resolveHostApproval,
  subscribeHostApproval
} from './hostCommandApproval'

const request = (sessionId?: string, command = 'git push') =>
  requestHostApproval({ sessionId, command, cwd: '/tmp' })

describe('hostCommandApproval', () => {
  afterEach(() => {
    // Drain any request the test left parked, then drop standing approvals.
    if (getPendingHostApproval()) resolveHostApproval('deny')
    clearHostApproval('s1')
    clearHostApproval('s2')
  })

  it('parks a request and exposes it to the modal', () => {
    const promise = request('s1', 'rm -rf build')

    expect(getPendingHostApproval()).toMatchObject({
      sessionId: 's1',
      command: 'rm -rf build',
      cwd: '/tmp'
    })

    resolveHostApproval('deny')
    return expect(promise).resolves.toBe('deny')
  })

  it('notifies subscribers when a request arrives and when it is answered', () => {
    const listener = jest.fn()
    const unsubscribe = subscribeHostApproval(listener)

    const promise = request('s1')
    expect(listener).toHaveBeenCalledTimes(1)

    resolveHostApproval('once')
    expect(listener).toHaveBeenCalledTimes(2)

    unsubscribe()
    return promise
  })

  it('allow-once does not grant a standing approval', async () => {
    const promise = request('s1')
    resolveHostApproval('once')
    await expect(promise).resolves.toBe('once')

    expect(hasSessionHostApproval('s1')).toBe(false)

    // The next command prompts again.
    const second = request('s1')
    expect(getPendingHostApproval()).not.toBeNull()
    resolveHostApproval('deny')
    await second
  })

  it('allow-for-chat auto-approves later commands in the same session only', async () => {
    const promise = request('s1')
    resolveHostApproval('chat')
    await expect(promise).resolves.toBe('chat')

    // Same session: resolves without parking anything for the modal.
    await expect(request('s1', 'git status')).resolves.toBe('chat')
    expect(getPendingHostApproval()).toBeNull()

    // A different session still has to ask.
    const other = request('s2')
    expect(getPendingHostApproval()).toMatchObject({ sessionId: 's2' })
    resolveHostApproval('deny')
    await expect(other).resolves.toBe('deny')
  })

  it('clearing a session revokes its standing approval', async () => {
    const promise = request('s1')
    resolveHostApproval('chat')
    await promise

    clearHostApproval('s1')
    expect(hasSessionHostApproval('s1')).toBe(false)

    const second = request('s1')
    expect(getPendingHostApproval()).not.toBeNull()
    resolveHostApproval('deny')
    await second
  })

  it('denies a second concurrent request rather than queueing it', async () => {
    const first = request('s1', 'first')
    const second = request('s1', 'second')

    await expect(second).resolves.toBe('deny')
    // The modal is still showing the first command, not the second.
    expect(getPendingHostApproval()?.command).toBe('first')

    resolveHostApproval('once')
    await expect(first).resolves.toBe('once')
  })

  it('resolving with nothing pending is a no-op', () => {
    expect(() => resolveHostApproval('once')).not.toThrow()
  })

  it('a request without a session id can never gain a standing approval', async () => {
    const promise = request(undefined)
    resolveHostApproval('chat')
    await expect(promise).resolves.toBe('chat')

    // Nothing was recorded, so the next one still prompts.
    const second = request(undefined)
    expect(getPendingHostApproval()).not.toBeNull()
    resolveHostApproval('deny')
    await second
  })
})
