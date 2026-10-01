import React from 'react'

export const TextCodeBlock: React.FC<{ text: string }> = ({ text }) => {
  return (
    <pre className="bg-sunken text-ink p-2.5 rounded-control overflow-x-auto whitespace-pre-wrap max-h-[50vh] max-w-[90vw]">
      <code>{text}</code>
    </pre>
  )
}
