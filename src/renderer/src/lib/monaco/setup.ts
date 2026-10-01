/**
 * Point @monaco-editor/react at the bundled monaco-editor instead of its default, which injects
 * `<script src="https://cdn.jsdelivr.net/npm/monaco-editor@…/min/vs/loader.js">` — remote script
 * the app's Content-Security-Policy (script-src 'self') refuses, and which also broke the
 * editors offline.
 *
 * Import this module (for its side effect) from every component that renders a monaco editor.
 * The editor is loaded as its own chunk: `loader.init()` resolves to the configured `monaco`,
 * and resolving with a promise adopts its value.
 */
import { loader } from '@monaco-editor/react'

loader.config({
  // eslint-disable-next-line no-restricted-syntax -- lazy chunk: monaco is several MB
  monaco: import('./monacoEditor').then(
    (m) => m.monaco
  ) as unknown as typeof import('monaco-editor')
})
