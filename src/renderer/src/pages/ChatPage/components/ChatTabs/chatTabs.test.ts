import { effectiveTab, TabMemory, visibleTabs, type ChatTabConditions } from './chatTabs'

const none: ChatTabConditions = {
  hasProject: false,
  hasSandbox: false,
  hasTerminal: false,
  hasTodo: false,
  hasMessages: false
}
const all: ChatTabConditions = {
  hasProject: true,
  hasSandbox: true,
  hasTerminal: true,
  hasTodo: true,
  hasMessages: true
}

describe('visibleTabs', () => {
  it('always lists Chat and System prompt', () => {
    expect(visibleTabs(none)).toEqual(['chat', 'systemPrompt'])
  })

  it('lists every tab, in order, when each has something to show', () => {
    expect(visibleTabs(all)).toEqual([
      'chat',
      'files',
      'sandbox',
      'terminal',
      'todo',
      'cost',
      'systemPrompt'
    ])
  })

  it('ties each optional tab to its own condition', () => {
    expect(visibleTabs({ ...none, hasProject: true })).toContain('files')
    expect(visibleTabs({ ...none, hasSandbox: true })).toContain('sandbox')
    expect(visibleTabs({ ...none, hasTerminal: true })).toContain('terminal')
    expect(visibleTabs({ ...none, hasTodo: true })).toContain('todo')
    expect(visibleTabs({ ...none, hasMessages: true })).toContain('cost')
  })
})

describe('effectiveTab', () => {
  it('keeps a listed tab', () => {
    expect(effectiveTab('sandbox', all)).toBe('sandbox')
  })

  it('falls back to Chat when the tab is no longer listed', () => {
    expect(effectiveTab('sandbox', { ...all, hasSandbox: false })).toBe('chat')
    expect(effectiveTab('files', { ...all, hasProject: false })).toBe('chat')
    expect(effectiveTab('terminal', { ...all, hasTerminal: false })).toBe('chat')
  })
})

describe('TabMemory', () => {
  it('remembers the tab per chat and opens unseen chats on Chat', () => {
    const memory = new TabMemory()
    memory.set('a', 'files')
    memory.set('b', 'cost')
    expect(memory.get('a')).toBe('files')
    expect(memory.get('b')).toBe('cost')
    expect(memory.get('c')).toBe('chat')
  })

  it('ignores a missing session id', () => {
    const memory = new TabMemory()
    memory.set(undefined, 'files')
    expect(memory.get(undefined)).toBe('chat')
  })

  it('going back to Chat forgets the chat', () => {
    const memory = new TabMemory()
    memory.set('a', 'files')
    memory.set('a', 'chat')
    expect(memory.get('a')).toBe('chat')
  })
})
