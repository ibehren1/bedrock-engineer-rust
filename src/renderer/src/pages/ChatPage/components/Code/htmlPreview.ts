/**
 * Sandboxing for the chat's ```html previews (HtmlBlock).
 *
 * The iframe is sandboxed without `allow-same-origin`, so the model's page runs in an opaque
 * origin: it can't reach the app's DOM, storage or `window.parent.__TAURI_INTERNALS__` (which
 * would let it invoke commands and run tools). Under Tauri a `srcdoc` iframe would also inherit
 * the app's Content-Security-Policy and block the page's inline and CDN scripts, so the iframe
 * loads the `htmlpreview://` shell (src-tauri/app/src/html_preview.rs), which has its own policy,
 * and the HTML is handed to it by `postMessage`:
 *
 *   shell → parent  { type: 'html-preview:ready' }
 *   parent → shell  { type: 'html-preview:render', html }
 */

/** The iframe's `sandbox` attribute. Never add `allow-same-origin` here. */
export const HTML_PREVIEW_SANDBOX = 'allow-scripts'

export const HTML_PREVIEW_READY = 'html-preview:ready'
export const HTML_PREVIEW_RENDER = 'html-preview:render'

/** The Tauri URI scheme serving the preview shell. */
export const HTML_PREVIEW_SCHEME = 'htmlpreview'

export type HtmlPreviewRender = { type: typeof HTML_PREVIEW_RENDER; html: string }

/**
 * The message to send back for a `message` event: the page to render when `event` is our own
 * preview frame announcing it is ready, otherwise `null` (other frames, other messages).
 */
export function previewReply(
  event: { source: unknown; data: unknown },
  frameWindow: unknown,
  html: string
): HtmlPreviewRender | null {
  if (!frameWindow || event.source !== frameWindow) return null
  const data = event.data as { type?: unknown } | null
  if (!data || typeof data !== 'object' || data.type !== HTML_PREVIEW_READY) return null
  return { type: HTML_PREVIEW_RENDER, html }
}
