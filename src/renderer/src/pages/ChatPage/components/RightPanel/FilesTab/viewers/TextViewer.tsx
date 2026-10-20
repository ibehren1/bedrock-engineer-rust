import React from 'react'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { tomorrow } from 'react-syntax-highlighter/dist/cjs/styles/prism'

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
    <SyntaxHighlighter
      language={language}
      style={tomorrow}
      showLineNumbers
      wrapLongLines
      className="!m-0 !bg-transparent"
      customStyle={{ background: 'transparent', padding: '0.75rem', fontSize: '0.6875rem' }}
    >
      {content}
    </SyntaxHighlighter>
  )
}

export default TextViewer
