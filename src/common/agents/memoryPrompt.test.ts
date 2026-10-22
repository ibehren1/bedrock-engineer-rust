import {
  buildMemoryPrompt,
  MEMORY_FACTS_INTRO,
  MEMORY_PROMPT_HEADER,
  MEMORY_RULES_INTRO,
  withMemoryPrompt
} from './memoryPrompt'
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
        MEMORY_PROMPT_HEADER,
        '<rules>',
        MEMORY_RULES_INTRO,
        '- [a] Use TypeScript',
        '- [c] Prefer pnpm',
        '</rules>',
        '<facts>',
        MEMORY_FACTS_INTRO,
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
    expect(prompt).not.toContain(MEMORY_FACTS_INTRO)
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
