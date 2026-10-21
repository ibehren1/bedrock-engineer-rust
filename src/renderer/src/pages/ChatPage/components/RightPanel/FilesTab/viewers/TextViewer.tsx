import React from 'react'
import { SourceView } from '../../../Code/SourceView'

/** Highlighting gets slow on big files; past this they show as plain text. */
const HIGHLIGHT_LIMIT = 300_000

export const TextViewer: React.FC<{ content: string; language: string }> = ({
  content,
  language
}) => {
  if (content.length > HIGHLIGHT_LIMIT || language === 'text') {
    return (
      <pre className="m-0 p-3 font-mono text-xs text-ink whitespace-pre-wrap break-words">
        {content}
      </pre>
    )
  }
  return (
    <SourceView
      code={content}
      language={language}
      wrapLongLines
      fontSize="0.6875rem"
      className="min-h-full"
    />
  )
}

export default TextViewer
