import { detectPlatform, shortcutAction, type Platform } from './nativeMenus'

const key = (
  k: string,
  mods: Partial<Record<'ctrl' | 'meta' | 'alt' | 'shift', boolean>> = {}
) => ({
  key: k,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift
})

describe('detectPlatform', () => {
  test('recognises the three webviews', () => {
    expect(detectPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit')).toBe(
      'mac'
    )
    expect(detectPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/130')).toBe('windows')
    expect(detectPlatform('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit')).toBe('linux')
  })
})

describe('shortcutAction', () => {
  test('Windows handles every Electron before-input-event key', () => {
    const p: Platform = 'windows'
    expect(shortcutAction(key('=', { ctrl: true }), p)).toBe('zoomIn')
    expect(shortcutAction(key('+', { ctrl: true, shift: true }), p)).toBe('zoomIn')
    expect(shortcutAction(key('-', { ctrl: true }), p)).toBe('zoomOut')
    expect(shortcutAction(key('0', { ctrl: true }), p)).toBe('resetZoom')
    expect(shortcutAction(key('r', { ctrl: true }), p)).toBe('reload')
  })

  test('macOS leaves menu-bound keys to the menu and handles Cmd+Plus', () => {
    const p: Platform = 'mac'
    expect(shortcutAction(key('+', { meta: true, shift: true }), p)).toBe('zoomIn')
    expect(shortcutAction(key('=', { meta: true }), p)).toBeNull()
    expect(shortcutAction(key('-', { meta: true }), p)).toBeNull()
    expect(shortcutAction(key('r', { meta: true }), p)).toBeNull()
    // Ctrl is not the macOS command modifier.
    expect(shortcutAction(key('+', { ctrl: true }), p)).toBeNull()
  })

  test('Linux leaves everything to the GTK accelerators', () => {
    for (const k of ['+', '=', '-', '0', 'r']) {
      expect(shortcutAction(key(k, { ctrl: true }), 'linux')).toBeNull()
    }
  })

  test('ignores unmodified keys, Alt chords, Force Reload and other keys', () => {
    expect(shortcutAction(key('='), 'windows')).toBeNull()
    expect(shortcutAction(key('=', { ctrl: true, alt: true }), 'windows')).toBeNull()
    expect(shortcutAction(key('R', { ctrl: true, shift: true }), 'windows')).toBeNull()
    expect(shortcutAction(key('r', { ctrl: true, shift: true }), 'windows')).toBeNull()
    expect(shortcutAction(key('c', { ctrl: true }), 'windows')).toBeNull()
  })
})
