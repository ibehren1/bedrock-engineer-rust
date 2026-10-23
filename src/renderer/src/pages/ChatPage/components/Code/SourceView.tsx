import React from 'react'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { tomorrow } from 'react-syntax-highlighter/dist/cjs/styles/prism'

type SourceViewProps = {
  code: string
  language: string
  /** Wrap long lines instead of scrolling sideways. */
  wrapLongLines?: boolean
  fontSize?: string
  className?: string
}

// Each line carries its number as data, drawn by `.source-lines` (index.css) as a ::before.
// Generated content is never part of a selection, so copying the source never picks up the
// numbers. `showLineNumbers` is still on because the library only passes real numbers to
// `lineProps` with it (otherwise `false`); the inline number spans it adds are hidden, and the
// `display: flex` it gives wrapped lines is overridden so long lines still wrap as text.
// (The typings only allow HTMLProps, which has no data-* attributes; React passes them through.)
const lineProps = (lineNumber: number) =>
  ({
    'data-line': lineNumber,
    style: { display: 'inline' }
  }) as unknown as React.HTMLProps<HTMLElement>

/**
 * Syntax-highlighted source with line numbers, as shown by the code blocks' Source views and
 * the Files tab. `tomorrow` is a light-on-dark palette, so it sits on its own dark surface
 * (`--code-surface`) in every appearance.
 */
export const SourceView: React.FC<SourceViewProps> = ({
  code,
  language,
  wrapLongLines = false,
  fontSize,
  className = ''
}) => (
  <div className={`source-lines bg-[var(--code-surface)] ${className}`}>
    <SyntaxHighlighter
      language={language}
      style={tomorrow}
      wrapLines
      showLineNumbers
      lineNumberStyle={{ display: 'none' }}
      wrapLongLines={wrapLongLines}
      lineProps={lineProps}
      className="!m-0 !bg-transparent"
      customStyle={{
        background: 'transparent',
        color: 'var(--code-ink)',
        padding: '0.75rem 1rem',
        minHeight: '100%',
        ...(fontSize ? { fontSize } : {})
      }}
    >
      {code}
    </SyntaxHighlighter>
  </div>
)

export default SourceView
