import { highlightJson } from './highlightJson'

describe('highlightJson', () => {
  test('escapes markup in keys and values', () => {
    const html = highlightJson({ '<b>k</b>': '<img src=x onerror="alert(1)">', a: 'x & y' })
    expect(html).not.toMatch(/<img|<b>/)
    expect(html).toContain('&lt;img src=x onerror=')
    expect(html).toContain('&lt;b&gt;k&lt;/b&gt;')
    expect(html).toContain('x &amp; y')
  })

  test('colors keys, strings, numbers, booleans and null', () => {
    const html = highlightJson({ s: 'v', n: 1, b: true, z: null })
    expect(html).toContain('<span class="text-accent">"s"</span>:')
    expect(html).toContain(': <span class="text-success">"v"</span>')
    expect(html).toContain(': <span class="text-accent">1</span>,')
    expect(html).toContain(': <span class="text-warning">true</span>,')
    expect(html).toContain(': <span class="text-purple-600 dark:text-purple-400">null</span>')
  })

  test('undefined renders as text', () => {
    expect(highlightJson(undefined)).toBe('undefined')
  })
})
