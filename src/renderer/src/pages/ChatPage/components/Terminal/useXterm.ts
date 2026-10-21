import { useEffect, useRef, useState, type RefObject } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebglAddon } from '@xterm/addon-webgl'
import '@xterm/xterm/css/xterm.css'

/** The background both terminals draw on; the host element uses it too, so there is no seam. */
export const TERMINAL_BACKGROUND = '#0b0e14'

/**
 * The app's chosen monospace stack, resolved to a real font list.
 *
 * xterm measures character cells itself and takes a plain font-family string, so a CSS
 * `var(--font-mono)` never resolves for it — it silently falls back to the browser's
 * default monospace. Reading the variable off the document keeps the terminal on the font
 * picked in Settings → Appearance. The Nerd Font comes last, for prompt icons.
 */
const resolveMonoFont = (): string =>
  `${
    getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim() ||
    "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace"
  }, ${NERD_FONT}`

/** Bundled icons-only font (see index.css), last in the list so it only supplies prompt icons. */
const NERD_FONT = "'Symbols Nerd Font Mono'"

/**
 * An xterm emulator in `hostRef`, created while `enabled`, kept fitted to the element and on
 * the app's code font. `onResize` gets the grid size after each refit. The emulator is only
 * the view: disposing it never closes the shell behind it.
 */
export const useXterm = (
  hostRef: RefObject<HTMLDivElement | null>,
  enabled: boolean,
  onResize: (cols: number, rows: number) => void
): Terminal | null => {
  const fitRef = useRef<FitAddon | null>(null)
  const [terminal, setTerminal] = useState<Terminal | null>(null)
  const onResizeRef = useRef(onResize)
  onResizeRef.current = onResize

  // Create the emulator once the host element exists and `enabled` is set.
  //
  // `terminal` must NOT be a dependency here even though the effect sets it: the cleanup
  // disposes the instance, so depending on it makes the effect tear down and rebuild what
  // it just created, forever. That loop also meant the emulator was disposed before it
  // could paint a prompt.
  useEffect(() => {
    if (!enabled || !hostRef.current) return

    const instance = new Terminal({
      convertEol: false,
      cursorBlink: true,
      fontSize: 13,
      fontFamily: resolveMonoFont(),
      // JetBrains Mono Variable is loaded as a variable font; asking for 400/600 rather
      // than the defaults keeps bold text from being synthesised.
      fontWeight: 400,
      fontWeightBold: 600,
      lineHeight: 1.2,
      letterSpacing: 0,
      allowProposedApi: true,
      theme: { background: TERMINAL_BACKGROUND, foreground: '#d3d7de', cursor: '#d3d7de' }
    })
    const fit = new FitAddon()
    instance.loadAddon(fit)
    instance.open(hostRef.current)
    // WebGL draws each cell itself, so a glyph never gets painted over by the next run of text
    // as it can in the default HTML renderer. Without WebGL, or when its context is lost, the
    // HTML renderer takes over again.
    try {
      const webgl = new WebglAddon()
      webgl.onContextLoss(() => webgl.dispose())
      instance.loadAddon(webgl)
    } catch (error) {
      console.warn('[Terminal] WebGL renderer unavailable, using the HTML renderer', error)
    }
    fit.fit()

    fitRef.current = fit
    setTerminal(instance)

    // xterm measures one character to size its grid. If it does that before the variable
    // font has loaded it measures the fallback, and every glyph afterwards sits slightly
    // off its cell. Re-measure once the fonts are ready.
    // The Nerd Font only loads when a glyph needs it, so ask for it up front: otherwise the first
    // prompt's icons are drawn as boxes.
    void Promise.all([
      document.fonts?.ready,
      document.fonts?.load(`13px ${NERD_FONT}`, '\ue0a0')
    ]).then(() => {
      try {
        instance.clearTextureAtlas?.()
        fit.fit()
        instance.refresh(0, instance.rows - 1)
      } catch {
        // The view may already be gone; the ResizeObserver covers the next paint.
      }
    })

    return () => {
      instance.dispose()
      fitRef.current = null
      setTerminal(null)
    }
  }, [enabled])

  // Changing the Code font in Settings → Appearance flips data-font-mono on <html>, which
  // xterm cannot see: it holds the resolved family from when it was created.
  useEffect(() => {
    if (!terminal) return

    const observer = new MutationObserver(() => {
      const font = resolveMonoFont()
      if (terminal.options.fontFamily === font) return
      terminal.options.fontFamily = font
      // Cell metrics change with the font, so the size has to be recomputed.
      try {
        fitRef.current?.fit()
        onResizeRef.current(terminal.cols, terminal.rows)
      } catch {
        // A hidden view has no measurable size; the ResizeObserver will catch up.
      }
    })
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-font-mono']
    })

    return () => observer.disconnect()
  }, [terminal])

  // Fit from the element's own size, so any layout change (a tab shown, the window resized)
  // is picked up once it has settled.
  useEffect(() => {
    if (!terminal || !hostRef.current) return

    let frame: number | undefined
    const observer = new ResizeObserver(() => {
      if (frame) window.clearTimeout(frame)
      frame = window.setTimeout(() => {
        try {
          fitRef.current?.fit()
          onResizeRef.current(terminal.cols, terminal.rows)
        } catch {
          // Fitting a detached or zero-sized element throws; the next resize will retry.
        }
      }, 100)
    })
    observer.observe(hostRef.current)

    return () => {
      if (frame) window.clearTimeout(frame)
      observer.disconnect()
    }
  }, [terminal])

  return terminal
}
