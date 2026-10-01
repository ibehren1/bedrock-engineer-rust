const invokeMock = jest.fn()

jest.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => invokeMock(...args),
  Channel: class {
    onmessage: (m: any) => void = () => {}
  }
}))
const mockListeners = new Map<string, (e: { payload: any }) => void>()
jest.mock('@tauri-apps/api/event', () => ({
  listen: jest.fn(async (name: string, cb: (e: { payload: any }) => void) => {
    mockListeners.set(name, cb)
    return () => {}
  })
}))
jest.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isFocused: async () => true })
}))

import {
  call,
  channelToCommand,
  converseStreamOptions,
  installTauriBridge,
  NotPortedError,
  pubsubEventName,
  tauriConverseStream,
  toError,
  withAbort
} from './tauriBridge'

const tick = () => new Promise((r) => setTimeout(r, 0))

beforeEach(() => invokeMock.mockReset())

describe('naming', () => {
  it('maps Electron channels to snake_case commands', () => {
    expect(channelToCommand('background-agent:chat')).toBe('background_agent_chat')
    expect(channelToCommand('window:openTaskHistory')).toBe('window_open_task_history')
    expect(channelToCommand('bedrock:getModelMaxTokens')).toBe('bedrock_get_model_max_tokens')
    expect(channelToCommand('get-local-image')).toBe('get_local_image')
  })

  it('sanitizes pub/sub channels into valid event names', () => {
    expect(pubsubEventName('sandbox-activity:abc_1')).toBe('pubsub:sandbox-activity:abc_1')
    expect(pubsubEventName('a.b c')).toBe('pubsub:a_b_c')
  })
})

describe('errors', () => {
  it('parses JSON error strings into named Errors', () => {
    const e = toError('{"name":"ValidationException","message":"bad input"}')
    expect(e).toBeInstanceOf(Error)
    expect(e.name).toBe('ValidationException')
    expect(e.message).toBe('bad input')
  })

  it('wraps plain strings', () => {
    expect(toError('boom').message).toBe('boom')
  })

  it('reports unregistered commands as not yet ported', async () => {
    invokeMock.mockRejectedValue('Command foo_bar not found')
    await expect(call('api.foo.bar', 'foo_bar')).rejects.toBeInstanceOf(NotPortedError)
    await expect(call('api.foo.bar', 'foo_bar')).rejects.toThrow('not yet ported: api.foo.bar')
  })
})

describe('tauriConverseStream', () => {
  it('yields channel events in order and finishes at the reported count', async () => {
    const events = [
      { messageStart: { role: 'assistant' } },
      { contentBlockDelta: { delta: { text: 'hi' }, contentBlockIndex: 0 } },
      { messageStop: { stopReason: 'end_turn' } }
    ]
    invokeMock.mockImplementation(async (command: string, args: any) => {
      expect(command).toBe('converse_stream')
      expect(args.request).toEqual({ modelId: 'm' })
      expect(typeof args.streamId).toBe('string')
      // Resolve before the last event arrives, as the IPC response can overtake channel messages.
      args.onEvent.onmessage(events[0])
      args.onEvent.onmessage(events[1])
      setTimeout(() => args.onEvent.onmessage(events[2]), 5)
      return events.length
    })

    const got: any[] = []
    for await (const e of tauriConverseStream({ modelId: 'm' })) got.push(e)
    expect(got).toEqual(events)
  })

  it('rejects with the command error', async () => {
    invokeMock.mockRejectedValue('{"name":"ThrottlingException","message":"slow down"}')
    const gen = tauriConverseStream({ modelId: 'm' })
    await expect(gen.next()).rejects.toMatchObject({ name: 'ThrottlingException' })
  })

  it('yields the events sent before a mid-stream failure, then throws it', async () => {
    const events = [
      { messageStart: { role: 'assistant' } },
      { contentBlockDelta: { delta: { text: 'a' } } }
    ]
    invokeMock.mockImplementation(async (_command: string, args: any) => {
      args.onEvent.onmessage(events[0])
      // The rejection overtakes the second event.
      setTimeout(() => args.onEvent.onmessage(events[1]), 5)
      throw '{"name":"ModelStreamErrorException","message":"broke","eventsSent":2}'
    })
    const got: any[] = []
    await expect(
      (async () => {
        for await (const e of tauriConverseStream({ modelId: 'm' })) got.push(e)
      })()
    ).rejects.toMatchObject({ name: 'ModelStreamErrorException', message: 'broke' })
    expect(got).toEqual(events)
  })

  describe('when channel events never arrive', () => {
    const grace = converseStreamOptions.tailGraceMs
    beforeEach(() => (converseStreamOptions.tailGraceMs = 10))
    afterEach(() => (converseStreamOptions.tailGraceMs = grace))

    it('throws a clear error after the grace period instead of hanging', async () => {
      const errorSpy = jest.spyOn(console, 'error').mockImplementation(() => {})
      invokeMock.mockImplementation(async (_command: string, args: any) => {
        args.onEvent.onmessage({ messageStart: { role: 'assistant' } })
        return 3
      })
      const got: any[] = []
      await expect(
        (async () => {
          for await (const e of tauriConverseStream({ modelId: 'm' })) got.push(e)
        })()
      ).rejects.toThrow('ended with 1 of 3 events received')
      expect(got).toHaveLength(1)
      expect(errorSpy).toHaveBeenCalled()
      errorSpy.mockRestore()
    })

    it('throws the command error after the grace period', async () => {
      const errorSpy = jest.spyOn(console, 'error').mockImplementation(() => {})
      invokeMock.mockRejectedValue('{"name":"ThrottlingException","message":"x","eventsSent":4}')
      const gen = tauriConverseStream({ modelId: 'm' })
      await expect(gen.next()).rejects.toMatchObject({ name: 'ThrottlingException' })
      errorSpy.mockRestore()
    })
  })

  it('uses a distinct stream id per call', async () => {
    const ids: string[] = []
    invokeMock.mockImplementation(async (_command: string, args: any) => {
      ids.push(args.streamId)
      return 0
    })
    for await (const _ of tauriConverseStream({})) void _
    for await (const _ of tauriConverseStream({})) void _
    expect(ids).toHaveLength(2)
    expect(ids[0]).not.toBe(ids[1])
    expect(ids[0]).toMatch(/^stream-[0-9a-f-]{36}$/)
  })

  it('cancels the backend stream and throws AbortError on abort', async () => {
    let streamId: string | undefined
    invokeMock.mockImplementation((command: string, args: any) => {
      if (command === 'converse_stream') {
        streamId = args.streamId
        args.onEvent.onmessage({ messageStart: { role: 'assistant' } })
        return new Promise(() => {}) // never finishes on its own
      }
      return Promise.resolve()
    })
    const controller = new AbortController()
    const gen = tauriConverseStream({ modelId: 'm' }, controller.signal)
    expect((await gen.next()).value).toEqual({ messageStart: { role: 'assistant' } })
    const pending = gen.next()
    controller.abort()
    await expect(pending).rejects.toMatchObject({ name: 'AbortError' })
    expect(invokeMock).toHaveBeenCalledWith('converse_cancel', { streamId })
  })

  it('withAbort rejects with AbortError', async () => {
    const controller = new AbortController()
    const p = withAbort(new Promise(() => {}), controller.signal)
    controller.abort()
    await expect(p).rejects.toMatchObject({ name: 'AbortError' })
  })
})

describe('installTauriBridge', () => {
  const g = globalThis as any

  beforeAll(() => {
    g.__APP_NAME__ = 'Test App'
    g.window = g
    g.__TAURI_INTERNALS__ = {}
    g.document = { title: '', hasFocus: () => true }
  })

  afterAll(() => {
    delete g.window
    delete g.__TAURI_INTERNALS__
    delete g.document
    delete g.__APP_NAME__
  })

  it('hydrates a sync store cache and writes through', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'store_all') return { language: 'ja', aws: { region: 'us-east-1' } }
      if (command === 'store_set' || command === 'store_delete') return null
      throw `Command ${command} not found`
    })
    await installTauriBridge()

    expect(g.window.store.get('language')).toBe('ja')
    // Returned values are copies, like electron-store.
    const aws = g.window.store.get('aws')
    aws.region = 'mutated'
    expect(g.window.store.get('aws').region).toBe('us-east-1')

    g.window.store.set('language', 'en')
    expect(g.window.store.get('language')).toBe('en')
    g.window.store.set('language', undefined)
    await tick()
    await tick()
    expect(invokeMock).toHaveBeenCalledWith('store_set', { key: 'language', value: 'en' })
    expect(invokeMock).toHaveBeenCalledWith('store_delete', { key: 'language' })

    // Unported startup-time sync reads degrade to empty values instead of throwing.
    expect(g.window.chatHistory.getAllSessionMetadata()).toEqual([])
    expect(g.window.chatHistory.getActiveSessionId()).toBeUndefined()
    expect(g.window.api.tools.getToolSpecs()).toEqual([])
    await expect(g.window.file.readSharedAgents()).resolves.toMatchObject({ agents: [] })
    await expect(g.window.api.bedrock.translateText({})).rejects.toBeInstanceOf(NotPortedError)
    expect(document.title).toBe('Test App')
  })
})

describe('store safety', () => {
  const g = globalThis as any

  beforeAll(() => {
    g.__APP_NAME__ = 'Test App'
    g.window = g
    g.__TAURI_INTERNALS__ = {}
    g.document = { title: '', hasFocus: () => true }
  })

  afterAll(() => {
    delete g.window
    delete g.__TAURI_INTERNALS__
    delete g.document
    delete g.__APP_NAME__
  })

  beforeEach(() => mockListeners.clear())

  /** A fresh copy of the bridge module (its install promise and caches are module state). */
  const freshBridge = (): typeof import('./tauriBridge') => {
    let mod: any
    jest.isolateModules(() => {
      mod = require('./tauriBridge')
    })
    return mod
  }

  const emit = (event: string, payload: any) => mockListeners.get(event)?.({ payload })

  it('refuses to start and to write when the settings could not be loaded', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'store_all') throw 'disk on fire'
      if (command === 'store_set') return null
      throw `Command ${command} not found`
    })
    const bridge = freshBridge()
    await expect(bridge.installTauriBridge()).rejects.toBeInstanceOf(bridge.StoreHydrationError)
    await expect(bridge.installTauriBridge()).rejects.toThrow('disk on fire')

    g.window.store.set('customAgents', [])
    await bridge.storeFlush()
    expect(invokeMock).not.toHaveBeenCalledWith('store_set', expect.anything())
  })

  it('treats a non-object store_all result as a failure', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'store_all') return null
      throw `Command ${command} not found`
    })
    const bridge = freshBridge()
    await expect(bridge.installTauriBridge()).rejects.toBeInstanceOf(bridge.StoreHydrationError)
  })

  it('applies store-changed events except for keys with a local write pending', async () => {
    let releaseWrite: () => void = () => {}
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'store_all') return { language: 'en', projectPath: '/a' }
      if (command === 'store_set') return new Promise<void>((r) => (releaseWrite = r))
      throw `Command ${command} not found`
    })
    const bridge = freshBridge()
    await bridge.installTauriBridge()

    // Another window (or Rust, e.g. open_directory) changed a key.
    emit('store-changed', { key: 'projectPath', value: '/b', deleted: false })
    expect(g.window.store.get('projectPath')).toBe('/b')
    emit('store-changed', { key: 'projectPath', value: null, deleted: true })
    expect(g.window.store.get('projectPath')).toBeUndefined()

    // Our own write is queued: a stale echo must not revert the local value.
    g.window.store.set('language', 'ja')
    emit('store-changed', { key: 'language', value: 'en', deleted: false })
    expect(g.window.store.get('language')).toBe('ja')
    await tick()
    releaseWrite()
    await bridge.storeFlush()
    await tick()
    emit('store-changed', { key: 'language', value: 'fr', deleted: false })
    expect(g.window.store.get('language')).toBe('fr')
  })

  it('snapshots the value at set time', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'store_all') return {}
      if (command === 'store_set') return null
      throw `Command ${command} not found`
    })
    const bridge = freshBridge()
    await bridge.installTauriBridge()
    const agents = [{ id: 'a' }]
    g.window.store.set('customAgents', agents)
    agents.push({ id: 'mutated-later' })
    await bridge.storeFlush()
    expect(invokeMock).toHaveBeenCalledWith('store_set', {
      key: 'customAgents',
      value: [{ id: 'a' }]
    })
  })

  it('flushes queued writes before settings-dependent commands and on close', async () => {
    const order: string[] = []
    let releaseWrite: () => void = () => {}
    invokeMock.mockImplementation(async (command: string) => {
      order.push(command)
      if (command === 'store_all') return {}
      if (command === 'store_set') return new Promise<void>((r) => (releaseWrite = r))
      return null
    })
    const bridge = freshBridge()
    await bridge.installTauriBridge()

    g.window.store.set('aws', { region: 'eu-west-1' })
    const tool = bridge.call('api.tools.execute', 'tools_execute', {})
    const other = bridge.call('api.x', 'logger_log', {})
    await tick()
    await other
    // Unrelated commands go straight through; the tool call waits for the write.
    expect(order).toContain('logger_log')
    expect(order).toContain('store_set')
    expect(order).not.toContain('tools_execute')
    releaseWrite()
    await tool
    expect(order.slice(-1)).toEqual(['tools_execute'])

    // Close / quit: Rust asks the window to flush and waits for the answer.
    g.window.store.set('language', 'ja')
    emit('store-flush-request', { token: 7 })
    await tick()
    expect(invokeMock).not.toHaveBeenCalledWith('store_flush_done', expect.anything())
    releaseWrite()
    await bridge.storeFlush()
    await tick()
    await tick()
    expect(invokeMock).toHaveBeenCalledWith('store_flush_done', { token: 7 })
  })
})
