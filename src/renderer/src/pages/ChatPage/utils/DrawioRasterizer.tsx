import { useImperativeHandle, useRef, useState, forwardRef, useCallback } from 'react'
import { DrawIoEmbed, DrawIoEmbedRef, EventExport } from 'react-drawio'

/**
 * Imperative handle for rasterizing DrawIO XML to a PNG data URL.
 * Used by the chat export flow, which renders one of these hidden off-screen.
 */
export type DrawioRasterizerRef = {
  rasterize: (xml: string) => Promise<string | null>
}

const EXPORT_TIMEOUT_MS = 15000

/**
 * A hidden DrawIO embed that rasterizes DrawIO XML to PNG on demand.
 *
 * The draw.io embed protocol is event driven, so a live iframe is required;
 * this cannot be done with a pure function. Each `rasterize(xml)` call remounts
 * the embed with a fresh key so `onLoad` fires reliably, then triggers a PNG
 * export and resolves with the resulting data URL (or null on failure/timeout).
 *
 * Calls are processed one at a time; the chat export awaits each sequentially.
 */
export const DrawioRasterizer = forwardRef<DrawioRasterizerRef>((_props, ref) => {
  const drawioRef = useRef<DrawIoEmbedRef>(null)
  const [state, setState] = useState<{ xml: string; nonce: number } | null>(null)

  const pendingRef = useRef<{
    resolve: (value: string | null) => void
    timer: ReturnType<typeof setTimeout> | null
  } | null>(null)
  const nonceRef = useRef(0)

  const settle = useCallback((value: string | null) => {
    const pending = pendingRef.current
    if (!pending) return
    if (pending.timer) clearTimeout(pending.timer)
    pendingRef.current = null
    pending.resolve(value)
  }, [])

  useImperativeHandle(
    ref,
    () => ({
      rasterize: (xml: string) =>
        new Promise<string | null>((resolve) => {
          if (pendingRef.current) {
            // One at a time; refuse overlapping calls defensively.
            resolve(null)
            return
          }
          const timer = setTimeout(() => settle(null), EXPORT_TIMEOUT_MS)
          pendingRef.current = { resolve, timer }
          nonceRef.current += 1
          setState({ xml, nonce: nonceRef.current })
        })
    }),
    [settle]
  )

  const handleLoad = useCallback(() => {
    if (!pendingRef.current) return
    try {
      drawioRef.current?.exportDiagram({ format: 'png', scale: 3, background: '#ffffff' })
    } catch (error) {
      console.error('Failed to export DrawIO diagram:', error)
      settle(null)
    }
  }, [settle])

  const handleExport = useCallback((data: EventExport) => settle(data?.data ?? null), [settle])

  if (!state) return null

  return (
    <div style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', left: -9999 }}>
      <DrawIoEmbed
        key={state.nonce}
        ref={drawioRef}
        xml={state.xml}
        urlParameters={{ spin: false }}
        onLoad={handleLoad}
        onExport={handleExport}
      />
    </div>
  )
})

DrawioRasterizer.displayName = 'DrawioRasterizer'
