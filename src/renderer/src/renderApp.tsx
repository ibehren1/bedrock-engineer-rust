// Mounts the React app. Imported by main.tsx once the platform bridge is ready.
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import '../index.css'
import './i18n/config'

// Apply the saved appearance and font choices before first paint to avoid a
// flash of the wrong theme or typeface.
// (useTheme and useFont keep these in sync afterwards.)
{
  // Values are validated rather than trusted: an unrecognised appearance would
  // otherwise be written to data-theme with no token block behind it, rendering
  // the whole app untokenised. Keep these lists in step with useTheme.ts and
  // useFont.ts. (Duplicated deliberately — this runs before React, so it cannot
  // import a hook.)
  const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
    allowed.includes(value as T) ? (value as T) : fallback

  const stored = pick(
    window.store?.get('appTheme' as any),
    ['light', 'newspaper', 'dim', 'charcoal', 'dark', 'system'] as const,
    'dim'
  )
  const prefersDark =
    !!window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
  document.documentElement.dataset.theme =
    stored === 'system' ? (prefersDark ? 'dark' : 'light') : stored

  document.documentElement.dataset.fontSans = pick(
    window.store?.get('appFontSans' as any),
    ['inter', 'geist', 'system'] as const,
    'inter'
  )
  document.documentElement.dataset.fontMono = pick(
    window.store?.get('appFontMono' as any),
    ['jetbrains', 'geist-mono', 'system'] as const,
    'jetbrains'
  )
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
