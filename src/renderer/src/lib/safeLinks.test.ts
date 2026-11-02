import { stripUnsafeLinks } from './safeLinks'

type FakeAnchor = {
  href: string | null
  getAttribute: () => string | null
  removeAttribute: () => void
}

const anchor = (href: string): FakeAnchor => {
  const a: FakeAnchor = {
    href,
    getAttribute: () => a.href,
    removeAttribute: () => {
      a.href = null
    }
  }
  return a
}

describe('stripUnsafeLinks', () => {
  it('keeps web, mail and in-document links and removes the rest', () => {
    const links = [
      anchor('https://example.com'),
      anchor('http://example.com'),
      anchor('mailto:a@b.c'),
      anchor('#_Toc1'),
      anchor('javascript:alert(1)'),
      anchor('  JavaScript:alert(1)'),
      anchor('data:text/html,<script>x</script>'),
      anchor('file:///etc/passwd')
    ]
    const root = { querySelectorAll: () => links } as unknown as ParentNode
    expect(stripUnsafeLinks(root)).toBe(4)
    expect(links.map((l) => l.href)).toEqual([
      'https://example.com',
      'http://example.com',
      'mailto:a@b.c',
      '#_Toc1',
      null,
      null,
      null,
      null
    ])
  })
})
