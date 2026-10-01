import React, { useEffect, useRef, useState } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useTranslation } from 'react-i18next'
import { VscCode, VscEye } from 'react-icons/vsc'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { tomorrow } from 'react-syntax-highlighter/dist/cjs/styles/prism'
import { ResizableContainer } from './ResizableContainer'
import { HTML_PREVIEW_SANDBOX, HTML_PREVIEW_SCHEME, previewReply } from './htmlPreview'

type HtmlBlockProps = {
  code: string
  className?: string
}

/**
 * The model's page in an opaque-origin sandbox (see ./htmlPreview), rendered by the
 * `htmlpreview://` shell.
 */
const SandboxedHtmlPreview: React.FC<{ html: string }> = ({ html }) => {
  const frameRef = useRef<HTMLIFrameElement>(null)
  const htmlRef = useRef(html)
  htmlRef.current = html

  useEffect(() => {
    const onMessage = (event: MessageEvent): void => {
      const frameWindow = frameRef.current?.contentWindow
      const reply = previewReply(event, frameWindow, htmlRef.current)
      // The shell's origin is opaque ("null"), so the target origin can't be narrowed.
      if (reply) frameWindow?.postMessage(reply, '*')
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [])

  return (
    // Reload the shell whenever the page changes; it renders one document per load.
    <iframe
      key={html}
      ref={frameRef}
      className="w-full h-full border-0"
      style={{ zIndex: 1 }}
      title="HTML Preview"
      sandbox={HTML_PREVIEW_SANDBOX}
      src={convertFileSrc('preview', HTML_PREVIEW_SCHEME)}
    />
  )
}

export const HtmlBlock: React.FC<HtmlBlockProps> = ({ code, className = '' }) => {
  const { t } = useTranslation()
  const [isPreviewMode, setIsPreviewMode] = useState(true)

  const toggleMode = () => {
    setIsPreviewMode(!isPreviewMode)
  }

  return (
    <div className={`my-4 border border-strong rounded-container overflow-hidden ${className}`}>
      {/* Header with toggle buttons */}
      <div className="flex items-center justify-between bg-raised px-2.5 py-1 border-b border-strong">
        <span className="text-sm font-medium text-ink">HTML</span>
        <div className="flex items-center space-x-2">
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
      <ResizableContainer initialHeight={800} minHeight={200} maxHeight={1800}>
        {isPreviewMode ? (
          <div className="h-full bg-surface relative">
            <SandboxedHtmlPreview html={code} />
          </div>
        ) : (
          <div className="h-full bg-surface-2 overflow-auto">
            <SyntaxHighlighter
              language="html"
              style={tomorrow}
              showLineNumbers
              wrapLines
              className="!m-0 !bg-transparent h-full"
              customStyle={{
                background: 'transparent',
                padding: '1rem',
                height: '100%'
              }}
            >
              {code}
            </SyntaxHighlighter>
          </div>
        )}
      </ResizableContainer>
    </div>
  )
}

export default HtmlBlock
