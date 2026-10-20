import React from 'react'
import MD from '@renderer/components/Markdown/MD'

export const MarkdownViewer: React.FC<{ content: string }> = ({ content }) => (
  <div className="px-4 py-3 text-ink">
    <MD>{content}</MD>
  </div>
)

export default MarkdownViewer
