import { isModKey, modKeyLabel, pageShortcut } from './platform'

describe('platform shortcuts', () => {
  it('uses ⌘ on macOS and Ctrl elsewhere', () => {
    expect(modKeyLabel(true)).toBe('⌘')
    expect(modKeyLabel(false)).toBe('Ctrl')
    expect(isModKey({ metaKey: true, ctrlKey: false }, true)).toBe(true)
    expect(isModKey({ metaKey: false, ctrlKey: true }, true)).toBe(false)
    expect(isModKey({ metaKey: false, ctrlKey: true }, false)).toBe(true)
    expect(isModKey({ metaKey: true, ctrlKey: false }, false)).toBe(false)
  })

  it('numbers only the first nine pages', () => {
    expect(pageShortcut(1)).toBe(1)
    expect(pageShortcut(9)).toBe(9)
    expect(pageShortcut(10)).toBeUndefined()
    expect(pageShortcut(0)).toBeUndefined()
  })
})
