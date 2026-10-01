import React from 'react'

interface Props {
  result: {
    title: string
    url: string
    content: string
    score: number
  }
}

export const SearchResult: React.FC<Props> = ({ result }) => {
  const { title, url, content, score } = result

  const handleUrlClick = (url: string) => {
    open(url)
  }

  return (
    <div className="border border-subtle rounded-container bg-surface-2 p-2.5">
      {/* Title & Score */}
      <div className="flex items-start justify-between gap-4 mb-2">
        <h3 className="text-base font-semibold text-ink line-clamp-2 text-sm">{title}</h3>
        <span className="bg-accent-tint text-accent text-xs font-medium px-2.5 py-0.5 rounded-control whitespace-nowrap">
          {(score * 100).toFixed(1)}%
        </span>
      </div>

      {/* URL */}
      <div
        className="text-xs text-accent hover:underline cursor-pointer mb-2 break-all"
        onClick={() => handleUrlClick(url)}
      >
        {url}
      </div>

      {/* Content */}
      <p className="text-xs text-ink line-clamp-3">{content}</p>
    </div>
  )
}
