// Minimal startup error screen, shown instead of the app when the settings could not be loaded
// (see main.tsx). Plain DOM on purpose: nothing that reads `window.store` may be evaluated.

/** Replace the page with the error and a Retry button (reloads the window). */
export function renderStartupError(error: unknown, doc: Document = document): void {
  const message = error instanceof Error ? error.message : String(error)
  const root = doc.getElementById('root') ?? doc.body
  root.replaceChildren()

  const box = doc.createElement('div')
  box.setAttribute('role', 'alert')
  box.style.cssText =
    'max-width:560px;margin:15vh auto;padding:24px;font-family:system-ui,sans-serif;' +
    'line-height:1.5;color:#222;background:#fff;border:1px solid #ddd;border-radius:8px'

  const title = doc.createElement('h1')
  title.textContent = 'Your settings could not be loaded'
  title.style.cssText = 'font-size:18px;margin:0 0 8px'

  const text = doc.createElement('p')
  text.textContent =
    'The app was not started so that nothing overwrites your saved settings. ' +
    'Nothing has been changed on disk.'
  text.style.cssText = 'margin:0 0 12px'

  const detail = doc.createElement('pre')
  detail.textContent = message
  detail.style.cssText =
    'white-space:pre-wrap;word-break:break-word;font-size:12px;background:#f5f5f5;' +
    'padding:8px;border-radius:4px;margin:0 0 16px'

  const retry = doc.createElement('button')
  retry.type = 'button'
  retry.textContent = 'Retry'
  retry.style.cssText = 'padding:6px 16px;font-size:14px;cursor:pointer'
  retry.addEventListener('click', () => doc.defaultView?.location.reload())

  box.append(title, text, detail, retry)
  root.append(box)
}
