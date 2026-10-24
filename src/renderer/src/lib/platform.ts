/** Whether the app runs on macOS (shortcuts use ⌘ there and Ctrl elsewhere). */
export const isMac = (): boolean =>
  typeof navigator !== 'undefined' && navigator.platform.toLowerCase().includes('mac')

/** The shortcut modifier's label: `⌘` on macOS, `Ctrl` elsewhere. */
export const modKeyLabel = (mac: boolean = isMac()): string => (mac ? '⌘' : 'Ctrl')

/** Whether the platform's shortcut modifier is held: ⌘ on macOS, Ctrl elsewhere. */
export const isModKey = (
  e: Pick<KeyboardEvent, 'metaKey' | 'ctrlKey'>,
  mac: boolean = isMac()
): boolean => (mac ? e.metaKey : e.ctrlKey)

/** Sidebar / command palette page shortcuts: only digits 1-9 exist. */
export const pageShortcut = (position: number): number | undefined =>
  position >= 1 && position <= 9 ? position : undefined
