import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { AgentIconView, IconifyIcon } from './AgentIconView'
import { getIconData, loadIconCollection } from './iconCollections'

// The component is exercised through static markup: it has no DOM dependencies,
// and the interesting part is which SVG a given icon value produces.
const render = (props: Parameters<typeof AgentIconView>[0]) =>
  renderToStaticMarkup(React.createElement(AgentIconView, props))

describe('AgentIconView', () => {
  it('renders a curated icon with the requested colour and classes', () => {
    const markup = render({ icon: 'robot', iconColor: '#ff0000', className: 'w-5 h-5' })
    expect(markup).toContain('<svg')
    expect(markup).toContain('class="w-5 h-5"')
    expect(markup).toContain('color:#ff0000')
  })

  it('falls back to the placeholder for an unknown value', () => {
    const markup = render({ icon: 'definitely-not-an-icon' })
    expect(markup).toContain('<svg')
  })

  it('renders the requested fallback until the collection is loaded', () => {
    const markup = render({
      icon: 'ph:rocket-launch',
      fallback: React.createElement('span', { className: 'placeholder' })
    })
    expect(markup).toBe('<span class="placeholder"></span>')
  })

  it('renders the collection icon once its data is available', async () => {
    await loadIconCollection('tabler')
    const markup = render({ icon: 'tabler:rocket', className: 'w-4 h-4' })
    expect(markup).toContain('viewBox="0 0 24 24"')
    expect(markup).toContain('class="w-4 h-4"')
    expect(markup).toContain('<path')
  })
})

describe('IconifyIcon', () => {
  it('inlines the icon body inside a sized, self-colouring svg', async () => {
    await loadIconCollection('tabler')
    const data = getIconData('tabler:rocket')
    if (!data) throw new Error('expected tabler:rocket to be loaded')

    const markup = renderToStaticMarkup(React.createElement(IconifyIcon, { data }))
    expect(markup).toContain('width="1em"')
    expect(markup).toContain('height="1em"')
    expect(markup).toContain('aria-hidden="true"')
    expect(markup).toContain('currentColor')
  })
})
