import {
  flushTouchedFiles,
  getTouchedFiles,
  recordTouchedFiles,
  subscribeTouchedFiles,
  TOUCHED_FILES_FLUSH_MS,
  trackTurnFileChanges
} from './touchedFiles'

type ChangeEvent = { root?: string; paths: string[] }

const g = globalThis as any
const addTouchedFiles = jest.fn(async () => {})
const getSession = jest.fn()
let emitChange: (event: ChangeEvent) => void = () => {}

beforeAll(() => {
  g.window = {
    chatHistory: {
      addTouchedFiles,
      getSession,
      getTouchedFiles: (id: string) => (id === 'saved' ? ['old.md'] : [])
    },
    workspaceFiles: {
      onChanged: (cb: (event: ChangeEvent) => void) => {
        emitChange = cb
        return () => (emitChange = () => {})
      },
      watch: async () => {},
      unwatch: async () => {}
    }
  }
})

afterAll(() => {
  delete g.window
})

beforeEach(() => {
  jest.useFakeTimers()
  addTouchedFiles.mockClear()
})

afterEach(async () => {
  await flushTouchedFiles()
  jest.useRealTimers()
})

describe('touched files', () => {
  it('shows new paths at once and saves them in one batch', async () => {
    const heard: string[] = []
    const unsubscribe = subscribeTouchedFiles((id) => heard.push(id))
    recordTouchedFiles('s1', ['a.md'])
    recordTouchedFiles('s1', ['a.md', 'b.md'])
    recordTouchedFiles('s2', ['c.md'])
    unsubscribe()

    expect(heard).toEqual(['s1', 's1', 's2'])
    expect(getTouchedFiles('s1')).toEqual(['a.md', 'b.md'])
    expect(addTouchedFiles).not.toHaveBeenCalled()

    jest.advanceTimersByTime(TOUCHED_FILES_FLUSH_MS)
    expect(addTouchedFiles.mock.calls).toEqual([
      ['s1', ['a.md', 'b.md']],
      ['s2', ['c.md']]
    ])
    // Already saved paths aren't sent again.
    recordTouchedFiles('s1', ['b.md'])
    jest.advanceTimersByTime(TOUCHED_FILES_FLUSH_MS)
    expect(addTouchedFiles).toHaveBeenCalledTimes(2)
  })

  it('reads saved paths from the cache without copying the session', () => {
    expect(getTouchedFiles('saved')).toEqual(['old.md'])
    recordTouchedFiles('saved', ['new.md'])
    expect(getTouchedFiles('saved')).toEqual(['old.md', 'new.md'])
    expect(getSession).not.toHaveBeenCalled()
  })

  it("saves a turn's file changes when the turn's tracking stops", async () => {
    const stop = trackTurnFileChanges('turn', '/p')
    emitChange({ root: '/p', paths: ['/p/out/a.md'] })
    emitChange({ root: '/p', paths: ['/p/out/b.md'] })
    expect(getTouchedFiles('turn')).toEqual(['out/a.md', 'out/b.md'])
    expect(addTouchedFiles).not.toHaveBeenCalled()

    stop()
    expect(addTouchedFiles.mock.calls).toEqual([['turn', ['out/a.md', 'out/b.md']]])
    jest.advanceTimersByTime(TOUCHED_FILES_FLUSH_MS)
    expect(addTouchedFiles).toHaveBeenCalledTimes(1)
  })
})
