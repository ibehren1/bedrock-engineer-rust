import { describe, expect, it } from '@jest/globals'
import { readFileSync } from 'fs'
import { join } from 'path'
import { githubSlug, markdownHeadings, rehypeHeadingIds, Slugger } from './slug'

describe('githubSlug', () => {
  it('matches GitHub anchors', () => {
    expect(githubSlug('26. Accounting: what Bedrock is costing you')).toBe(
      '26-accounting-what-bedrock-is-costing-you'
    )
    expect(githubSlug('4.2 Step 1 — Turn on the models')).toBe('42-step-1--turn-on-the-models')
    expect(githubSlug('12. Handing work to another agent with `@`')).toBe(
      '12-handing-work-to-another-agent-with-'
    )
  })

  it('numbers repeated headings', () => {
    const s = new Slugger()
    expect([s.slug('Tab'), s.slug('Tab'), s.slug('Tab')]).toEqual(['tab', 'tab-1', 'tab-2'])
  })
})

describe('rehypeHeadingIds', () => {
  const heading = (tagName: string, ...children: object[]) => ({
    type: 'element',
    tagName,
    properties: {},
    children
  })
  const text = (value: string) => ({ type: 'text', value })

  it('gives headings the same ids every time it runs', () => {
    const tree = () => ({
      type: 'root',
      children: [
        heading('h2', text('26. Accounting: what')),
        heading('p', text('body')),
        heading('h3', text('Tab')),
        heading('h3', text('Tab with '), heading('code', text('@')))
      ]
    })
    const ids = (t: any) =>
      t.children.filter((c: any) => c.tagName !== 'p').map((c: any) => c.properties.id)
    const first = tree()
    rehypeHeadingIds()(first as any)
    const second = tree()
    rehypeHeadingIds()(second as any)
    expect(ids(first)).toEqual(['26-accounting-what', 'tab', 'tab-with-'])
    expect(ids(second)).toEqual(ids(first))
  })
})

describe('the user guide', () => {
  const guide = readFileSync(join(__dirname, '../../../../../docs/USER_GUIDE.md'), 'utf8')
  const slugs = new Set(markdownHeadings(guide).map((h) => h.slug))

  it('has a heading for every in-page link', () => {
    const anchors = [...guide.matchAll(/\]\(#([^)]+)\)/g)].map((m) => m[1])
    expect(anchors.length).toBeGreaterThan(50)
    const broken = [...new Set(anchors)].filter((a) => !slugs.has(a))
    expect(broken).toEqual([])
  })
})
