const invokeMock = jest.fn()

jest.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => invokeMock(...args),
  convertFileSrc: (path: string, protocol: string) =>
    `${protocol}://localhost/${encodeURIComponent(path)}`,
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
let mockCloseHandler: ((event: { preventDefault: () => void }) => Promise<void>) | undefined
let mockWindowLabel = 'main'
jest.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    label: mockWindowLabel,
    isFocused: async () => true,
    onCloseRequested: async (handler: typeof mockCloseHandler) => {
      mockCloseHandler = handler
      return () => {}
    }
  })
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

    // Other windows keep their own cache but never write settings (main-window-only in Rust).
    invokeMock.mockClear()
    mockWindowLabel = 'task-history'
    try {
      g.window.store.set('language', 'de')
      expect(g.window.store.get('language')).toBe('de')
      await tick()
      await tick()
      expect(invokeMock).not.toHaveBeenCalledWith('store_set', expect.anything())
    } finally {
      mockWindowLabel = 'main'
    }

    // Unported startup-time sync reads degrade to empty values instead of throwing.
    expect(g.window.chatHistory.getAllSessionMetadata()).toEqual([])
    expect(g.window.chatHistory.getActiveSessionId()).toBeUndefined()
    expect(g.window.api.tools.getToolSpecs()).toEqual([])
    await expect(g.window.file.readSharedAgents()).resolves.toMatchObject({ agents: [] })
    await expect(g.window.api.bedrock.translateText({})).rejects.toBeInstanceOf(NotPortedError)
    expect(document.title).toBe('Test App')
  })

  it('maps window.memory to the memory_* commands and memory-changed', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue([])

    await g.window.memory.list()
    await g.window.memory.add({ content: 'Use pnpm', kind: 'rule', source: 'manual' })
    await g.window.memory.update({ id: 'm1', enabled: false })
    await g.window.memory.delete('m1')
    await g.window.chatHistory.setUseMemoryItems('s1', false)
    expect(invokeMock).toHaveBeenCalledWith('memory_list', undefined)
    expect(invokeMock).toHaveBeenCalledWith('memory_add', {
      content: 'Use pnpm',
      kind: 'rule',
      source: 'manual'
    })
    expect(invokeMock).toHaveBeenCalledWith('memory_update', { id: 'm1', enabled: false })
    expect(invokeMock).toHaveBeenCalledWith('memory_delete', { id: 'm1' })
    expect(invokeMock).toHaveBeenCalledWith('chat_history_set_use_memory_items', {
      sessionId: 's1',
      value: false
    })

    const seen: unknown[] = []
    g.window.memory.onChanged((items: unknown) => seen.push(items))
    await tick()
    mockListeners.get('memory-changed')?.({ payload: [{ id: 'm2' }] })
    expect(seen).toEqual([[{ id: 'm2' }]])
  })

  it('maps window.api.help to the help_* commands', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue('')
    await g.window.api.help.userGuide()
    await g.window.api.help.search('budget', 5)
    expect(invokeMock).toHaveBeenCalledWith('help_user_guide', undefined)
    expect(invokeMock).toHaveBeenCalledWith('help_search', { query: 'budget', topK: 5 })
  })

  it('maps window.api.usage to the usage_* commands and usage-changed', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue([])
    const query = { from: '2026-10-01', to: '2026-10-31' }

    await g.window.api.usage.summary(query, 'model')
    await g.window.api.usage.months()
    await g.window.api.usage.events(query, 50, 100)
    await g.window.api.usage.breakdown(query, 'day', 'model')
    await g.window.api.usage.info()
    await g.window.api.usage.budgetStatus()
    await g.window.api.usage.sessionCosts([{ sessionId: 's1' }])
    expect(invokeMock).toHaveBeenCalledWith('session_usage_costs', {
      sessions: [{ sessionId: 's1' }]
    })
    expect(invokeMock).toHaveBeenCalledWith('usage_budget_status', undefined)
    await g.window.api.usage.exportCsv(query, 'usage.csv')
    expect(invokeMock).toHaveBeenCalledWith('usage_breakdown', { query, by: 'day', then: 'model' })
    expect(invokeMock).toHaveBeenCalledWith('usage_info', undefined)
    expect(invokeMock).toHaveBeenCalledWith('usage_export_csv', { query, fileName: 'usage.csv' })
    expect(invokeMock).toHaveBeenCalledWith('usage_summary', { query, groupBy: 'model' })
    expect(invokeMock).toHaveBeenCalledWith('usage_months', undefined)
    expect(invokeMock).toHaveBeenCalledWith('usage_events', { query, limit: 50, offset: 100 })

    let changes = 0
    g.window.api.usage.onChanged(() => changes++)
    await tick()
    mockListeners.get('usage-changed')?.({ payload: null })
    expect(changes).toBe(1)
  })

  it('maps window.workspaceFiles to the workspace_files_* commands', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue(null)
    const wf = g.window.workspaceFiles

    await wf.list('')
    await wf.readText('docs/a.md')
    await wf.writeText('docs/a.md', 'x', 42)
    await wf.writeText('docs/a.md', 'y')
    await wf.open('docs/a.md')
    await wf.reveal()
    await wf.reveal('docs')
    await wf.exists(['/p/a.md', 'b.md'])
    await wf.popOut('docs/a.md')
    await g.window.chatHistory.addTouchedFiles('s1', ['out/r.md'])
    await g.window.chatHistory.addTouchedFiles('s1', [])
    expect(invokeMock.mock.calls).toEqual([
      ['workspace_files_list', { dir: '' }],
      ['workspace_files_read_text', { path: 'docs/a.md' }],
      ['workspace_files_write_text', { path: 'docs/a.md', content: 'x', expectedModifiedMs: 42 }],
      ['workspace_files_write_text', { path: 'docs/a.md', content: 'y' }],
      ['workspace_files_open', { path: 'docs/a.md' }],
      ['workspace_files_reveal', undefined],
      ['workspace_files_reveal', { path: 'docs' }],
      ['workspace_files_exists', { paths: ['/p/a.md', 'b.md'] }],
      ['window_open_file_viewer', { path: 'docs/a.md' }],
      ['chat_history_add_touched_files', { sessionId: 's1', paths: ['out/r.md'] }]
    ])

    expect(wf.fileUrl('a b/c.pdf')).toBe('workspacefile://localhost/a%20b%2Fc.pdf')
    expect(wf.fileUrl('c.pdf', 7)).toBe('workspacefile://localhost/c.pdf?v=7')

    const seen: unknown[] = []
    wf.onChanged((e: unknown) => seen.push(e))
    await tick()
    mockListeners.get('workspace-files-changed')?.({ payload: { root: '/p', paths: ['a.md'] } })
    expect(seen).toEqual([{ root: '/p', paths: ['a.md'] }])
  })

  it('serves chat sessions from the cache and patches it instead of re-reading', async () => {
    const meta = {
      id: 's2',
      title: 'Chat later',
      createdAt: 1,
      updatedAt: 1,
      messageCount: 3,
      agentId: 'helpAgent',
      modelId: 'm'
    }
    const session = (id: string) => ({
      id,
      title: 'Chat one',
      createdAt: 1,
      updatedAt: 1,
      agentId: 'a1',
      modelId: 'm',
      messages: [{ id: 'm0', role: 'user', content: [{ text: 'hi' }], timestamp: 1 }],
      touchedFiles: ['a.md']
    })
    let resolveLoad: (v: unknown) => void = () => {}
    invokeMock.mockImplementation(async (command: string, args?: any) => {
      if (command === 'store_all') return {}
      if (command === 'chat_history_snapshot')
        return { metadata: [meta], recent: [], folders: [], sessions: { s1: session('s1') } }
      if (command === 'chat_history_get_session')
        return new Promise((r) => (resolveLoad = () => r(session(args.sessionId))))
      if (command.startsWith('chat_history_get_') || command.startsWith('chat_history_'))
        return command.endsWith('_id') ? null : []
      return null
    })
    // The bridge installs once per module; a fresh copy hydrates this snapshot.
    let fresh: typeof import('./tauriBridge') | undefined
    jest.isolateModules(() => {
      fresh = require('./tauriBridge')
    })
    await fresh!.installTauriBridge()
    const ch = g.window.chatHistory

    // Field reads don't copy the session; uncached sessions answer from the list metadata only.
    expect(ch.getSessionField('s1', 'agentId')).toBe('a1')
    expect(ch.getSessionField('s2', 'agentId')).toBe('helpAgent')
    expect(ch.getSessionField('s2', 'touchedFiles')).toBeUndefined()
    const touched = ch.getTouchedFiles('s1')
    touched.push('mutated')
    expect(ch.getTouchedFiles('s1')).toEqual(['a.md'])
    expect(ch.getTouchedFiles('s2')).toEqual([])
    expect(ch.isSessionLoaded('s1')).toBe(true)

    // addMessage appends to the cached copy; only the list metadata is re-read.
    const message = { id: 'm1', role: 'assistant', content: [{ text: 'yo' }], timestamp: 2 }
    await ch.addMessage('s1', message)
    message.content[0].text = 'mutated'
    expect(ch.getSession('s1').messages.map((m: any) => m.content[0].text)).toEqual(['hi', 'yo'])
    expect(invokeMock).toHaveBeenCalledWith('chat_history_add_message', {
      sessionId: 's1',
      message
    })
    expect(invokeMock).toHaveBeenCalledWith('chat_history_get_all_session_metadata', undefined)
    expect(invokeMock).not.toHaveBeenCalledWith('chat_history_get_session', expect.anything())

    await ch.updateMessageContent('s1', 1, { ...message, content: [{ text: 'edited' }] })
    await ch.deleteMessage('s1', 0)
    await ch.deleteMessage('s1', 5) // out of range: no change, like the backend
    await ch.updateSessionTitle('s1', 'Named')
    expect(ch.getSession('s1').messages.map((m: any) => m.content[0].text)).toEqual(['edited'])
    expect(ch.getSessionField('s1', 'title')).toBe('Named')
    expect(invokeMock).not.toHaveBeenCalledWith('chat_history_get_session', expect.anything())

    // loadSession reads asynchronously, once for concurrent callers, and fills the cache.
    expect(ch.isSessionLoaded('s2')).toBe(false)
    const first = ch.loadSession('s2')
    const second = ch.loadSession('s2')
    resolveLoad(undefined)
    await expect(first).resolves.toBe(true)
    await expect(second).resolves.toBe(true)
    expect(
      invokeMock.mock.calls.filter(([c]: [string]) => c === 'chat_history_get_session')
    ).toEqual([['chat_history_get_session', { sessionId: 's2' }]])
    expect(ch.getSessionField('s2', 'touchedFiles')).toEqual(['a.md'])
    await expect(ch.loadSession('s2')).resolves.toBe(true)

    // A chat deleted while it loads isn't cached again.
    const load = ch.loadSession('s3')
    ch.deleteSession('s3')
    resolveLoad(undefined)
    await expect(load).resolves.toBe(false)
    expect(ch.isSessionLoaded('s3')).toBe(false)
  })

  it('reference-counts the workspace file watcher', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue(null)
    const wf = g.window.workspaceFiles

    await wf.watch()
    await wf.watch()
    await wf.unwatch()
    expect(invokeMock.mock.calls).toEqual([['workspace_files_watch', undefined]])
    await wf.unwatch()
    await wf.unwatch() // extra unwatch is a no-op
    expect(invokeMock.mock.calls).toEqual([
      ['workspace_files_watch', undefined],
      ['workspace_files_unwatch', undefined]
    ])
  })

  it('asks before a window closes and flushes the store when it may', async () => {
    await installTauriBridge()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue(null)
    let allow = false
    await g.window.appWindow.onCloseRequested(async () => allow)
    const preventDefault = jest.fn()

    await mockCloseHandler!({ preventDefault })
    expect(preventDefault).toHaveBeenCalledTimes(1)

    // Allowed: a queued store write lands before the handler returns (then the window closes).
    allow = true
    g.window.store.set('rightPanelWidth', 500)
    await mockCloseHandler!({ preventDefault })
    expect(preventDefault).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('store_set', { key: 'rightPanelWidth', value: 500 })
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
