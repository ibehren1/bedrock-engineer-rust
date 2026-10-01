// Entry point. The bridge shim must install `window.store`, `window.api`, etc. and hydrate its
// synchronous caches before any app module is evaluated (several read `window.store` at import
// time), so the app is imported only after it resolves.
//
// If the settings can't be loaded, the app is not started at all: rendering it on an empty
// settings cache would let the first settings save overwrite the real ones. A minimal error
// screen with a Retry (reload) is shown instead.
import { installTauriBridge, StoreHydrationError } from './lib/tauriBridge'
import { renderStartupError } from './startupError'
import { installCspViolationLogger } from './lib/cspViolationLogger'
import { installNativeMenus } from './lib/nativeMenus'

installCspViolationLogger()
// Copy/Paste context menu and the zoom/reload keys (main window only).
installNativeMenus()

installTauriBridge().then(
  // eslint-disable-next-line no-restricted-syntax -- must not evaluate before the bridge is ready
  () => import('./renderApp'),
  (e) => {
    if (e instanceof StoreHydrationError) {
      renderStartupError(e)
      return
    }
    console.error('Failed to install the Tauri bridge', e)
    // eslint-disable-next-line no-restricted-syntax -- must not evaluate before the bridge is ready
    return import('./renderApp')
  }
)
