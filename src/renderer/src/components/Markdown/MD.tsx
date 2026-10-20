import Markdown, { Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import style from './styles.module.css'
import { useMemo, type ReactNode } from 'react'
import LocalImage from '../LocalImage'
import { FilePathAnchor, FilePathCode } from '../FilePathLink'
import rehypeKatex from 'rehype-katex'
import remarkMath from 'remark-math'
import 'katex/dist/katex.min.css' // `rehype-katex` does not import the CSS for you

type MDProps = {
  children: string | null | undefined
}

type AnchorProps = {
  href?: string
  title?: string
  children?: React.ReactNode
}

type CodeProps = {
  className?: string
  children?: React.ReactNode
}

type ImgProps = {
  src?: string
  alt?: string
  className?: string
}

const MD = (props: MDProps) => {
  const components: Partial<Components> = useMemo(() => {
    return {
      // Links and inline code naming a project file open it in the chat's Files panel
      // (only inside ChatPage, which provides FilesPanelContext; plain elsewhere).
      // Wide tables scroll inside their own box instead of widening the whole transcript.
      table: ({ children }: { children?: ReactNode }) => (
        <div className={style.tableScroll}>
          <table>{children}</table>
        </div>
      ),
      a: ({ href, title, children }: AnchorProps) => (
        <FilePathAnchor href={href} title={title}>
          {children}
        </FilePathAnchor>
      ),
      code: ({ className, children }: CodeProps) => {
        const text = String(children ?? '')
        // Fenced blocks carry a language class or span lines; only inline code is a candidate.
        if (className || text.includes('\n')) return <code className={className}>{children}</code>
        return <FilePathCode text={text}>{children}</FilePathCode>
      },
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
      }
    }
  }, [])

  return (
    <Markdown
      className={style.reactMarkDown}
      remarkPlugins={[[remarkGfm, { singleTilde: false }], [remarkMath]]}
      rehypePlugins={[rehypeKatex]}
      components={components}
    >
      {props.children}
    </Markdown>
  )
}

export default MD
