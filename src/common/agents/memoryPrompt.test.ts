import { buildMemoryPrompt, withMemoryPrompt } from './memoryPrompt'
import type { MemoryItem } from '../../types/bridge/memory'

const item = (over: Partial<MemoryItem>): MemoryItem => ({
  id: 'm1',
  content: 'text',
  kind: 'memory',
  enabled: true,
  source: 'chat',
  createdAt: 1,
  updatedAt: 1,
  ...over
})

const HEADER =
  'The user asked you to remember the following. Follow these unless the user overrides them in this chat. Use updateMemory/removeMemory with the id to change or forget an item.'

describe('buildMemoryPrompt', () => {
  it('returns an empty string when nothing is enabled', () => {
    expect(buildMemoryPrompt([])).toBe('')
    expect(buildMemoryPrompt([item({ enabled: false })])).toBe('')
    expect(buildMemoryPrompt([item({ content: '   ' })])).toBe('')
  })

  it('lists rules and facts in separate sections, keeping order', () => {
    const prompt = buildMemoryPrompt([
      item({ id: 'a', kind: 'rule', content: 'Use TypeScript' }),
      item({ id: 'b', kind: 'memory', content: 'Prod region is eu-west-1' }),
      item({ id: 'c', kind: 'rule', content: 'Prefer pnpm' }),
      item({ id: 'd', kind: 'rule', content: 'Disabled', enabled: false })
    ])
    expect(prompt).toBe(
      [
        '<memory>',
        HEADER,
        '<rules>',
        '- [a] Use TypeScript',
        '- [c] Prefer pnpm',
        '</rules>',
        '<facts>',
        '- [b] Prod region is eu-west-1',
        '</facts>',
        '</memory>'
      ].join('\n')
    )
  })

  it('omits an empty sub-section', () => {
    const prompt = buildMemoryPrompt([item({ id: 'a', kind: 'rule', content: 'Rule' })])
    expect(prompt).toContain('<rules>')
    expect(prompt).not.toContain('<facts>')
  })

  it('indents continuation lines of multi-line items', () => {
    expect(buildMemoryPrompt([item({ id: 'x', content: 'one\ntwo' })])).toContain(
      '- [x] one\n  two'
    )
  })
})

describe('withMemoryPrompt', () => {
  const items = [item({ id: 'a', content: 'Fact' })]

  it('appends the block when enabled', () => {
    const prompt = withMemoryPrompt('Base', items, true)
    expect(prompt.startsWith('Base\n\n<memory>\n')).toBe(true)
    expect(prompt.endsWith('</memory>\n')).toBe(true)
  })

  it('leaves the prompt alone when the toggle is off or nothing is enabled', () => {
    expect(withMemoryPrompt('Base', items, false)).toBe('Base')
    expect(withMemoryPrompt('Base', [], true)).toBe('Base')
    expect(withMemoryPrompt('', items, false)).toBe('')
  })

  it('uses the block alone when there is no base prompt', () => {
    expect(withMemoryPrompt('', items, true).startsWith('<memory>')).toBe(true)
  })
})
