/**
 * PDF preview with pdf.js, one page at a time on a canvas. Loaded with React.lazy so pdf.js
 * stays out of the main bundle.
 *
 * CSP (src-tauri/app/tauri.conf.json): the worker is a bundled file (`?url`, so 'self' for
 * worker-src); pdf.js 6 has no eval-based font path; WebAssembly decoders are turned off
 * (`useWasm: false`) because script-src has no 'wasm-unsafe-eval', and pdf.js falls back to its
 * JavaScript decoders. The bytes are fetched here (connect-src `workspacefile:`) and handed to
 * pdf.js as data, so the worker never fetches anything.
 */
import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiChevronLeft, FiChevronRight, FiMinus, FiPlus } from 'react-icons/fi'
import { getDocument, GlobalWorkerOptions, type PDFDocumentProxy } from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

GlobalWorkerOptions.workerSrc = workerUrl

const MIN_ZOOM = 0.25
const MAX_ZOOM = 4
const ZOOM_STEP = 0.25

const PdfViewer: React.FC<{ url: string }> = ({ url }) => {
  const { t } = useTranslation()
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState(1)
  // null = fit the panel width
  const [zoom, setZoom] = useState<number | null>(null)
  const [effectiveZoom, setEffectiveZoom] = useState(1)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const [containerWidth, setContainerWidth] = useState(0)

  useEffect(() => {
    let cancelled = false
    let loadingTask: ReturnType<typeof getDocument> | undefined
    setDoc(null)
    setError(null)
    setPage(1)
    ;(async () => {
      try {
        const response = await fetch(url)
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const data = new Uint8Array(await response.arrayBuffer())
        if (cancelled) return
        loadingTask = getDocument({ data, useWasm: false })
        const loaded = await loadingTask.promise
        if (!cancelled) setDoc(loaded)
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      }
    })()
    return () => {
      cancelled = true
      void loadingTask?.destroy()
    }
  }, [url])

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const observer = new ResizeObserver(() => setContainerWidth(el.clientWidth))
    observer.observe(el)
    setContainerWidth(el.clientWidth)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!doc || !canvasRef.current) return
    let cancelled = false
    let task: { cancel: () => void } | undefined
    ;(async () => {
      try {
        const pdfPage = await doc.getPage(page)
        if (cancelled) return
        const base = pdfPage.getViewport({ scale: 1 })
        const fit = containerWidth > 0 ? Math.max(0.1, (containerWidth - 32) / base.width) : 1
        const scale = zoom ?? fit
        setEffectiveZoom(scale)
        const ratio = window.devicePixelRatio || 1
        const viewport = pdfPage.getViewport({ scale: scale * ratio })
        const canvas = canvasRef.current
        if (!canvas) return
        canvas.width = Math.floor(viewport.width)
        canvas.height = Math.floor(viewport.height)
        canvas.style.width = `${Math.floor(viewport.width / ratio)}px`
        canvas.style.height = `${Math.floor(viewport.height / ratio)}px`
        const renderTask = pdfPage.render({ canvas, viewport })
        task = renderTask
        await renderTask.promise
      } catch (e) {
        if (!cancelled && (e as Error)?.name !== 'RenderingCancelledException') {
          setError(e instanceof Error ? e.message : String(e))
        }
      }
    })()
    return () => {
      cancelled = true
      task?.cancel()
    }
  }, [doc, page, zoom, containerWidth])

  const total = doc?.numPages ?? 0
  const button =
    'p-1 rounded-control text-ink-muted hover:bg-raised hover:text-ink disabled:opacity-40 disabled:hover:bg-transparent'

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-1 px-3 py-1.5 border-b border-subtle text-xs text-ink-muted shrink-0">
        <button
          className={button}
          onClick={() => setPage((p) => Math.max(1, p - 1))}
          disabled={page <= 1}
          title={t('filesPanel.previousPage')}
          aria-label={t('filesPanel.previousPage')}
        >
          <FiChevronLeft />
        </button>
        <span className="tabular-nums">
          {t('filesPanel.pdfPage', { page: total ? page : 0, total })}
        </span>
        <button
          className={button}
          onClick={() => setPage((p) => Math.min(total, p + 1))}
          disabled={page >= total}
          title={t('filesPanel.nextPage')}
          aria-label={t('filesPanel.nextPage')}
        >
          <FiChevronRight />
        </button>
        <span className="mx-2 h-3 w-px bg-subtle" />
        <button
          className={button}
          onClick={() => setZoom(Math.max(MIN_ZOOM, effectiveZoom - ZOOM_STEP))}
          disabled={effectiveZoom <= MIN_ZOOM}
          title={t('filesPanel.zoomOut')}
          aria-label={t('filesPanel.zoomOut')}
        >
          <FiMinus />
        </button>
        <button
          className="tabular-nums px-1 rounded-control hover:bg-raised"
          onClick={() => setZoom(null)}
        >
          {Math.round(effectiveZoom * 100)}%
        </button>
        <button
          className={button}
          onClick={() => setZoom(Math.min(MAX_ZOOM, effectiveZoom + ZOOM_STEP))}
          disabled={effectiveZoom >= MAX_ZOOM}
          title={t('filesPanel.zoomIn')}
          aria-label={t('filesPanel.zoomIn')}
        >
          <FiPlus />
        </button>
      </div>
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-auto bg-surface-2 p-4">
        {error ? (
          <div className="text-sm text-danger">{t('filesPanel.pdfFailed', { error })}</div>
        ) : (
          <>
            {!doc && <div className="text-sm text-ink-faint">{t('filesPanel.loading')}</div>}
            <canvas
              ref={canvasRef}
              className={`mx-auto bg-white shadow-raised ${doc ? '' : 'hidden'}`}
            />
          </>
        )}
      </div>
    </div>
  )
}

export default PdfViewer
