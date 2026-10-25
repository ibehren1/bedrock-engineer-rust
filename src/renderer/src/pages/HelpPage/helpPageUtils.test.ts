import { describe, expect, it } from '@jest/globals'
import { markdownHeadings } from '@renderer/components/Markdown/slug'
import { headingSlugForPath, snippet } from './helpPageUtils'

const doc = `# Guide
## 8. Agents
### Tab 1 — Basic Settings
## 26. Accounting
### 26.4 A monthly budget
### Tab 1 — Basic Settings
`

describe('helpPageUtils', () => {
  const headings = markdownHeadings(doc)

  it('maps a heading path to the deepest heading under its parents', () => {
    expect(headingSlugForPath(headings, 'Guide › 26. Accounting › 26.4 A monthly budget')).toBe(
      '264-a-monthly-budget'
    )
    expect(headingSlugForPath(headings, 'Guide › 26. Accounting › Tab 1 — Basic Settings')).toBe(
      'tab-1--basic-settings-1'
    )
    expect(headingSlugForPath(headings, 'Nothing › here')).toBeNull()
  })

  it('makes plain snippets', () => {
    expect(snippet('**Set** a [budget](#x) on the `page`.')).toBe('Set a budget on the page.')
    expect(snippet('a'.repeat(300)).length).toBe(180)
  })
})
