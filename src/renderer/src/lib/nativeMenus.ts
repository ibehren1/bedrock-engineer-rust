/**
 * The main window's native context menu and the zoom/reload keys the app menu can miss.
 *
 * - Context menu: like Electron's `context-menu` handler, right-clicking shows a native menu with
 *   only Copy and Paste (`context_menu_popup`, `src-tauri/app/src/menu.rs`) instead of the
 *   webview's default one. Elements that show their own menu (call `preventDefault`, e.g. the
 *   code editor) keep it.
 * - Shortcuts: Electron's `before-input-event` handled Cmd/Ctrl + `=`/`+`/`-`/`0`/`r`. The app
 *   menu's accelerators cover these, except `+` (muda has no Plus key, so Zoom In is bound to
 *   `=`) and, on Windows, any key pressed while the webview has focus (WebView2 keeps keyboard
 *   input from the host window's accelerator table). Those go to `app_menu_action`.
 */
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type Platform = 'mac' | 'windows' | 'linux'
export type ShortcutAction = 'zoomIn' | 'zoomOut' | 'resetZoom' | 'reload'

type KeyEventLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

export function detectPlatform(userAgent: string): Platform {
  if (/Mac/i.test(userAgent)) return 'mac'
  if (/Windows/i.test(userAgent)) return 'windows'
  return 'linux'
}

/** The shortcut the renderer must handle itself for `event`, or `null` (menu or page handles it). */
export function shortcutAction(event: KeyEventLike, platform: Platform): ShortcutAction | null {
  const mod = platform === 'mac' ? event.metaKey : event.ctrlKey
  if (!mod || event.altKey) return null
  // Not bound by the menu on any platform. On Linux GTK may match Ctrl+= for it, so leave it.
  if (event.key === '+') return platform === 'linux' ? null : 'zoomIn'
  if (platform !== 'windows') return null
  if (event.key === '=') return 'zoomIn'
  if (event.key === '-') return 'zoomOut'
  if (event.key === '0') return 'resetZoom'
  // Electron matched lowercase `r` only, so Ctrl+Shift+R stays Force Reload.
  if (event.key === 'r' && !event.shiftKey) return 'reload'
  return null
}

let installed = false

/** Install both handlers in the main window's top-level document. */
export function installNativeMenus(): void {
  if (installed || typeof window === 'undefined') return
  if (window.self !== window.top) return
  try {
    if (getCurrentWindow().label !== 'main') return
  } catch {
    return
  }
  installed = true
  const platform = detectPlatform(navigator.userAgent)

  window.addEventListener('contextmenu', (event) => {
    if (event.defaultPrevented) return
    event.preventDefault()
    invoke('context_menu_popup').catch((e) => console.error('Failed to show context menu', e))
  })

  window.addEventListener(
    'keydown',
    (event) => {
      const action = shortcutAction(event, platform)
      if (!action) return
      event.preventDefault()
      invoke('app_menu_action', { action }).catch((e) =>
        console.error(`Failed to run ${action}`, e)
      )
    },
    true
  )
}
