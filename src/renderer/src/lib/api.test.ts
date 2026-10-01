const invokeMock = jest.fn()

jest.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => invokeMock(...args),
  Channel: class {
    onmessage: (m: any) => void = () => {}
  }
}))
jest.mock('@tauri-apps/api/event', () => ({ listen: jest.fn(async () => () => {}) }))
jest.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isFocused: async () => true })
}))
// A model that can't stream with tool use, so streamChatCompletion falls back to converse.
jest.mock('@common/models/models', () => ({ supportsStreamingWithToolUse: () => false }))

const g = globalThis as any
g.window = g
g.__TAURI_INTERNALS__ = {}

// eslint-disable-next-line @typescript-eslint/no-require-imports
const api = require('./api') as typeof import('./api')

afterAll(() => {
  delete g.window
  delete g.__TAURI_INTERNALS__
})

beforeEach(() => invokeMock.mockReset())

const converseResult = {
  output: {
    message: {
      role: 'assistant',
      content: [
        { text: 'Reading.' },
        { toolUse: { toolUseId: 't1', name: 'readFiles', input: { paths: ['a.ts'] } } },
        { toolUse: { toolUseId: 't2', name: 'listFiles', input: {} } }
      ]
    }
  },
  stopReason: 'tool_use',
  usage: { inputTokens: 1, outputTokens: 2, totalTokens: 3 }
}

describe('converseOutputToStreamEvents', () => {
  it('sends tool input as a delta and gives each block its own index', () => {
    const events = [...api.converseOutputToStreamEvents(converseResult)]
    expect(events).toEqual([
      { messageStart: { role: 'assistant' } },
      { contentBlockStart: { start: undefined, contentBlockIndex: 0 } },
      { contentBlockDelta: { delta: { text: 'Reading.' }, contentBlockIndex: 0 } },
      { contentBlockStop: { contentBlockIndex: 0 } },
      {
        contentBlockStart: {
          start: { toolUse: { toolUseId: 't1', name: 'readFiles' } },
          contentBlockIndex: 1
        }
      },
      {
        contentBlockDelta: {
          delta: { toolUse: { input: '{"paths":["a.ts"]}' } },
          contentBlockIndex: 1
        }
      },
      { contentBlockStop: { contentBlockIndex: 1 } },
      {
        contentBlockStart: {
          start: { toolUse: { toolUseId: 't2', name: 'listFiles' } },
          contentBlockIndex: 2
        }
      },
      { contentBlockDelta: { delta: { toolUse: { input: '{}' } }, contentBlockIndex: 2 } },
      { contentBlockStop: { contentBlockIndex: 2 } },
      { messageStop: { stopReason: 'tool_use' } },
      { metadata: { usage: converseResult.usage, metrics: { latencyMs: 0 } } }
    ])
  })
})

describe('streamChatCompletion without streaming tool use (Tauri)', () => {
  const props = {
    modelId: 'm',
    system: undefined,
    messages: [],
    toolConfig: { tools: [{ toolSpec: { name: 'readFiles', inputSchema: { json: {} } } }] }
  } as any

  it('calls converse with a request id and synthesizes the stream', async () => {
    invokeMock.mockImplementation(async (command: string, args: any) => {
      expect(command).toBe('converse')
      expect(args.request).toBe(props)
      expect(typeof args.requestId).toBe('string')
      return converseResult
    })
    const got: any[] = []
    for await (const e of api.streamChatCompletion(props)) got.push(e)
    expect(got).toEqual([...api.converseOutputToStreamEvents(converseResult)])
  })

  it('cancels the backend request on abort', async () => {
    let requestId: string | undefined
    invokeMock.mockImplementation((command: string, args: any) => {
      if (command === 'converse') {
        requestId = args.requestId
        return new Promise(() => {})
      }
      return Promise.resolve()
    })
    const controller = new AbortController()
    const pending = api.converse(props, controller.signal)
    controller.abort()
    await expect(pending).rejects.toMatchObject({ name: 'AbortError' })
    expect(invokeMock).toHaveBeenCalledWith('converse_cancel', { streamId: requestId })
  })
})
