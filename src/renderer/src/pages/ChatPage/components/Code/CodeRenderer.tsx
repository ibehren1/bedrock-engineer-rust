import React, { useMemo } from 'react'
import Markdown, { Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import rehypeKatex from 'rehype-katex'
import { HtmlBlock } from './HtmlBlock'
import { MermaidBlock } from './MermaidBlock'
import { DrawIOBlock } from './DrawIOBlock'
import LocalImage from '@renderer/components/LocalImage'
import { FilePathAnchor, FilePathCode } from '@renderer/components/FilePathLink'
import style from '@renderer/components/Markdown/styles.module.css'
import { extractDrawioXml, isDrawioXml } from '@renderer/lib/drawio/xmlParser'
import 'katex/dist/katex.min.css'

// Types
type CodeRendererProps = {
  text?: string
  className?: string
}

type AnchorProps = {
  href?: string
  title?: string
  children?: React.ReactNode
}

type ImgProps = {
  src?: string
  alt?: string
  className?: string
}

type CodeProps = {
  inline?: boolean
  className?: string
  children?: React.ReactNode
}

// Main Component
export const CodeRenderer: React.FC<CodeRendererProps> = ({ text = '', className = '' }) => {
  const components: Partial<Components> = useMemo(() => {
    return {
      // A link to a project file opens it in the Files panel; others open externally.
      a: ({ href, title, children }: AnchorProps) => (
        <FilePathAnchor href={href} title={title}>
          {children}
        </FilePathAnchor>
      ),
      img: ({ src, alt, ...props }: ImgProps) => {
        // Check if it's a remote URL (has a valid protocol)
        const isRemoteUrl = (() => {
          if (!src) return false
          try {
            const url = new URL(src)
            return ['http:', 'https:', 'data:', 'blob:'].includes(url.protocol)
          } catch {
            // If URL parsing fails, it's likely a local path
            return false
          }
        })()

        if (isRemoteUrl) {
          return <img src={src} alt={alt} {...props} />
        }

        // Treat everything else as local path
        return <LocalImage src={src || ''} alt={alt || ''} {...props} />
      },
      // Wide tables scroll inside their own box instead of widening the whole transcript.
      table: ({ children }: { children?: React.ReactNode }) => (
        <div className={style.tableScroll}>
          <table>{children}</table>
        </div>
      ),
      code: ({ inline, className, children, ...props }: CodeProps) => {
        const match = /language-(\w+)/.exec(className || '')
        const language = match ? match[1] : ''
        const codeContent = String(children).replace(/\n$/, '')

        // Handle block code (not inline)
        if (!inline) {
          // Special handling for HTML blocks
          if (language === 'html') {
            return <HtmlBlock code={codeContent} />
          }

          // Special handling for Mermaid blocks
          if (language === 'mermaid') {
            return <MermaidBlock code={codeContent} />
          }

          // Special handling for DrawIO XML blocks
          if (language === 'xml' || language === 'drawio') {
            const drawioXml = extractDrawioXml(codeContent)
            if (drawioXml) {
              return <DrawIOBlock xml={drawioXml} />
            }
          }

          // Auto-detect DrawIO XML even without explicit language tag
          if (!language && isDrawioXml(codeContent)) {
            const drawioXml = extractDrawioXml(codeContent)
            if (drawioXml) {
              return <DrawIOBlock xml={drawioXml} />
            }
          }
        }

        // Inline code (react-markdown 9 has no `inline` flag: no language and one line). One
        // naming a project file opens it in the Files panel.
        if (!className && !codeContent.includes('\n')) {
          return <FilePathCode text={codeContent}>{children}</FilePathCode>
        }
        return (
          <code className={className} {...props}>
            {children}
          </code>
        )
      }
    }
  }, [])

  return (
    <div className={`w-full text-ink ${className}`}>
      <Markdown
        className={style.reactMarkDown}
        remarkPlugins={[[remarkGfm, { singleTilde: false }], [remarkMath]]}
        rehypePlugins={[rehypeKatex]}
        components={components}
      >
        {text}
      </Markdown>
    </div>
  )
}

export default CodeRenderer
