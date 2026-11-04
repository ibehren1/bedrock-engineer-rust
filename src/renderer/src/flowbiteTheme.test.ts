import { createElement as h } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
// Per-component entries: the package index pulls in Carousel, whose `debounce` dependency is
// ESM-only and can't be required under Jest.
import { Button } from 'flowbite-react/components/Button'
import { Label } from 'flowbite-react/components/Label'
import { Select } from 'flowbite-react/components/Select'
import { TextInput } from 'flowbite-react/components/TextInput'
import { ThemeProvider } from 'flowbite-react/theme'
import { flowbiteClearTheme, flowbiteTheme } from './flowbiteTheme'

/** Render `element` the way App.tsx does: the app theme, with the keys it sets cleared first. */
const render = (element: ReturnType<typeof h>): string =>
  renderToStaticMarkup(
    h(ThemeProvider, { theme: flowbiteTheme, clearTheme: flowbiteClearTheme }, element)
  )

/** Class lists in `html`, flattened to single classes. */
const classes = (html: string): string[] =>
  [...html.matchAll(/class="([^"]*)"/g)].flatMap((m) => m[1].split(/\s+/)).filter(Boolean)

describe('flowbiteTheme on Flowbite React 0.12', () => {
  it('replaces the stock classes of a themed component instead of merging with them', () => {
    const html = render(h(Button, { color: 'light', size: 'sm' }, 'Save'))
    const list = classes(html)
    // Flowbite's own button base and light color: none of it may survive next to the tokens.
    expect(list.filter((c) => c.startsWith('dark:'))).toEqual([])
    expect(list).not.toContain('focus:ring-4')
    expect(list).not.toContain('rounded-lg')
    expect(list.filter((c) => /-(gray|cyan|blue)-\d+$/.test(c))).toEqual([])
    // The app's own classes are there.
    expect(list).toEqual(expect.arrayContaining(['rounded-control', 'bg-surface', 'text-ink']))
  })

  it('keeps form controls on the token palette', () => {
    const html = [
      render(h(Label, { htmlFor: 'x' }, 'Name')),
      render(h(TextInput, { id: 'x', defaultValue: '' })),
      render(h(Select, { id: 'y' }, h('option', null, 'a')))
    ].join('')
    const list = classes(html)
    expect(list.filter((c) => c.startsWith('dark:'))).toEqual([])
    expect(list.filter((c) => /-(gray|cyan|blue)-\d+$/.test(c))).toEqual([])
  })
})
