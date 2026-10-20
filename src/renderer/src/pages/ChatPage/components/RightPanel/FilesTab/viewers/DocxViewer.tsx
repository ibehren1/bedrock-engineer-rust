/**
 * Word (.docx) preview with docx-preview. Loaded with React.lazy so it stays out of the main
 * bundle.
 *
 * CSP: docx-preview injects <style> elements (style-src allows 'unsafe-inline'); images and
 * embedded fonts become data: URLs (`useBase64URL`), which img-src and font-src allow.
 * altChunks (embedded HTML documents) are not rendered: they would become srcdoc iframes.
 */
import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { renderAsync } from 'docx-preview'

const DocxViewer: React.FC<{ url: string }> = ({ url }) => {
  const { t } = useTranslation()
  const containerRef = useRef<HTMLDivElement>(null)
  const [state, setState] = useState<'loading' | 'ready' | 'error'>('loading')
  const [error, setError] = useState('')

  useEffect(() => {
    let cancelled = false
    const container = containerRef.current
    if (!container) return
    setState('loading')
    container.replaceChildren()
    ;(async () => {
      try {
        const response = await fetch(url)
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const data = await response.arrayBuffer()
        if (cancelled) return
        await renderAsync(data, container, undefined, {
          className: 'docx',
          inWrapper: true,
          ignoreLastRenderedPageBreak: true,
          useBase64URL: true,
          renderAltChunks: false,
          renderChanges: false,
          experimental: false
        })
        if (!cancelled) setState('ready')
      } catch (e) {
        if (cancelled) return
        setError(e instanceof Error ? e.message : String(e))
        setState('error')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [url])

  return (
    <div className="h-full overflow-auto bg-surface-2">
      {state === 'loading' && (
        <div className="p-4 text-sm text-ink-faint">{t('filesPanel.loading')}</div>
      )}
      {state === 'error' && (
        <div className="p-4 text-sm text-danger">{t('filesPanel.docxFailed', { error })}</div>
      )}
      {/* docx-preview's wrapper draws the grey backdrop and white pages itself. */}
      <div ref={containerRef} className={state === 'error' ? 'hidden' : ''} />
    </div>
  )
}

export default DocxViewer
