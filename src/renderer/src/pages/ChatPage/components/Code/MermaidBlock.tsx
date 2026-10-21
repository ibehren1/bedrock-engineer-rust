import React, { useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  VscArrowSwap,
  VscCode,
  VscCopy,
  VscEye,
  VscZoomIn,
  VscZoomOut,
  VscScreenFull,
  VscCloudDownload
} from 'react-icons/vsc'
import { SourceView } from './SourceView'
import { Mermaid } from './Mermaid'
import { ResizableContainer } from './ResizableContainer'
import toast from 'react-hot-toast'
import { parseSvg, toSvgXml } from '@renderer/lib/svgMarkup'
import { renderMermaid } from '@renderer/lib/mermaidRender'

type MermaidBlockProps = {
  code: string
  className?: string
}

export const MermaidBlock: React.FC<MermaidBlockProps> = ({ code, className = '' }) => {
  const { t } = useTranslation()
  const [isPreviewMode, setIsPreviewMode] = useState(true)
  const [zoomLevel, setZoomLevel] = useState(1)
  // Lopsided flowcharts are turned to fit the column; this switches back to the written direction.
  const [fit, setFit] = useState(true)
  const [reoriented, setReoriented] = useState(false)
  // Set once a fitted render actually turned the diagram, so the switch only shows when it matters.
  const [canFit, setCanFit] = useState(false)
  const onReoriented = useCallback((value: boolean) => {
    setReoriented(value)
    if (value) setCanFit(true)
  }, [])

  const copySource = async () => {
    try {
      await navigator.clipboard.writeText(code)
      toast.success(t('Source copied'))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }

  const toggleMode = () => {
    setIsPreviewMode(!isPreviewMode)
  }

  const zoomIn = () => {
    setZoomLevel((prev) => Math.min(prev + 0.25, 3))
  }

  const zoomOut = () => {
    setZoomLevel((prev) => Math.max(prev - 0.25, 0.5))
  }

  const resetZoom = () => {
    setZoomLevel(1)
  }

  const downloadAsPNG = async () => {
    try {
      // MermaidからSVGを生成（画面と同じ向きで）
      const { svg } = await renderMermaid(code, fit)

      // SVGのサイズを正確に取得（HTML として解析。XML だと <br> で切れる）
      const svgElement = parseSvg(svg)

      let width = 800
      let height = 600

      if (svgElement) {
        // width/height属性があれば使用
        const svgWidth = svgElement.getAttribute('width')
        const svgHeight = svgElement.getAttribute('height')

        if (svgWidth && svgHeight) {
          width = parseFloat(svgWidth.replace('px', ''))
          height = parseFloat(svgHeight.replace('px', ''))
        } else {
          // viewBox属性からサイズを取得
          const viewBox = svgElement.getAttribute('viewBox')
          if (viewBox) {
            const [, , vbWidth, vbHeight] = viewBox.split(' ').map(Number)
            width = vbWidth || 800
            height = vbHeight || 600
          }
        }
      }

      // 高解像度レンダリング（3倍解像度）
      const scale = 3
      const canvas = document.createElement('canvas')
      const ctx = canvas.getContext('2d')
      const img = new Image()

      img.onload = () => {
        // 高解像度でcanvasサイズを設定
        canvas.width = width * scale
        canvas.height = height * scale

        // コンテキストをスケール
        if (ctx) {
          ctx.scale(scale, scale)

          // 白い背景を追加
          ctx.fillStyle = 'white'
          ctx.fillRect(0, 0, width, height)

          // 高画質レンダリング設定
          ctx.imageSmoothingEnabled = true
          ctx.imageSmoothingQuality = 'high'

          // SVGをcanvasに描画
          ctx.drawImage(img, 0, 0, width, height)
        }

        // PNGとしてダウンロード
        canvas.toBlob(
          (blob) => {
            if (blob) {
              const url = URL.createObjectURL(blob)
              const a = document.createElement('a')
              a.href = url
              a.download = `mermaid-diagram-${Date.now()}.png`
              document.body.appendChild(a)
              a.click()
              document.body.removeChild(a)
              URL.revokeObjectURL(url)
            }
          },
          'image/png',
          1.0
        ) // 最高品質で出力
      }

      img.onerror = () => {
        console.error('Failed to load SVG image for PNG conversion')
      }

      // SVGをData URLに直接変換（Blob URLではなく）。画像として読むので整形式の XML にする。
      const svgDataUrl = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(toSvgXml(svg))))}`
      img.src = svgDataUrl
    } catch (error) {
      console.error('Failed to download PNG:', error)
    }
  }

  return (
    <div className={`my-4 border border-strong rounded-container overflow-hidden ${className}`}>
      {/* Header with toggle buttons */}
      <div className="flex items-center justify-between bg-raised px-2.5 py-1 border-b border-strong">
        <span className="text-sm font-medium text-ink">Mermaid</span>
        <div className="flex items-center space-x-2">
          {/* Zoom controls and download button - only show in preview mode */}
          {isPreviewMode && (
            <>
              <button
                onClick={downloadAsPNG}
                className="p-1 rounded-control text-ink hover:bg-sunken"
                title="Download as PNG"
              >
                <VscCloudDownload size={14} />
              </button>
              <div className="w-px h-4 bg-sunken" />
              <button
                onClick={zoomOut}
                disabled={zoomLevel <= 0.5}
                className="p-1 rounded-control text-ink hover:bg-sunken disabled:opacity-50 disabled:cursor-not-allowed"
                title="Zoom Out"
              >
                <VscZoomOut size={14} />
              </button>
              <span className="text-xs text-ink-muted min-w-[3rem] text-center">
                {Math.round(zoomLevel * 100)}%
              </span>
              <button
                onClick={zoomIn}
                disabled={zoomLevel >= 3}
                className="p-1 rounded-control text-ink hover:bg-sunken disabled:opacity-50 disabled:cursor-not-allowed"
                title="Zoom In"
              >
                <VscZoomIn size={14} />
              </button>
              <button
                onClick={resetZoom}
                className="p-1 rounded-control text-ink hover:bg-sunken"
                title="Reset Zoom"
              >
                <VscScreenFull size={14} />
              </button>
            </>
          )}
          {isPreviewMode && canFit && (
            <button
              onClick={() => setFit(!fit)}
              className="flex items-center gap-1 px-2 py-1 rounded-control text-xs text-ink hover:bg-sunken"
              title={
                fit && reoriented
                  ? t('Diagram turned to fit the chat; show it in the direction it was written')
                  : t('Turn the diagram so it fits the chat with less scrolling')
              }
            >
              <VscArrowSwap size={12} />
              <span>{fit ? t('As written') : t('Fit to width')}</span>
            </button>
          )}
          {!isPreviewMode && (
            <button
              onClick={() => void copySource()}
              className="p-1 rounded-control text-ink hover:bg-sunken"
              title={t('Copy source')}
              aria-label={t('Copy source')}
            >
              <VscCopy size={14} />
            </button>
          )}
          <div className="w-px h-4 bg-sunken" />
          <button
            onClick={toggleMode}
            className={`flex items-center space-x-1 px-3 py-1 rounded-control text-xs font-medium transition-colors ${
              !isPreviewMode ? 'bg-accent text-accent-fg' : 'bg-raised text-ink hover:bg-sunken'
            }`}
          >
            <VscCode size={12} />
            <span>{t('Source')}</span>
          </button>
          <button
            onClick={toggleMode}
            className={`flex items-center space-x-1 px-3 py-1 rounded-control text-xs font-medium transition-colors ${
              isPreviewMode ? 'bg-accent text-accent-fg' : 'bg-raised text-ink hover:bg-sunken'
            }`}
          >
            <VscEye size={12} />
            <span>{t('Preview')}</span>
          </button>
        </div>
      </div>

      {/* Resizable Content area */}
      <ResizableContainer initialHeight={800} minHeight={200} maxHeight={800}>
        {/* Preview mode - always rendered but hidden when not in preview mode */}
        <div
          className={`h-full bg-surface overflow-auto border-0 ${
            isPreviewMode ? 'block' : 'hidden'
          }`}
        >
          <div
            style={{
              transform: `scale(${zoomLevel})`,
              transformOrigin: 'top left',
              minWidth: `${100 * zoomLevel}%`,
              minHeight: `${100 * zoomLevel}%`,
              padding: '1rem'
            }}
            className="transition-transform duration-200"
          >
            <Mermaid chart={code} fit={fit} onReoriented={onReoriented} />
          </div>
        </div>

        {/* Source mode - always rendered but hidden when in preview mode */}
        <SourceView
          code={code}
          language="mermaid"
          className={`h-full overflow-auto ${isPreviewMode ? 'hidden' : 'block'}`}
        />
      </ResizableContainer>
    </div>
  )
}

export default MermaidBlock
