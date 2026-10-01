import React from 'react'
import { useTranslation } from 'react-i18next'

interface AgentMetadataProps {
  author?: string
  tags?: string[]
}

export const AgentMetadata: React.FC<AgentMetadataProps> = ({ author, tags }) => {
  const { t } = useTranslation()

  if (!author && (!tags || tags.length === 0)) {
    return null
  }

  return (
    <div className="bg-surface-2 p-2.5 rounded-container">
      {author && (
        <div className="mb-4">
          <span className="text-sm text-ink-muted mb-2 block">{t('authorLabel')}</span>
          <div
            className="flex items-center cursor-pointer"
            onClick={() => open(`https://github.com/${author}`)}
          >
            <img
              src={`https://github.com/${author}.png`}
              alt={`${author} avatar`}
              className="w-8 h-8 rounded-full object-cover mr-2 flex-shrink-0 border border-subtle"
              onError={(e) => {
                e.currentTarget.style.display = 'none'
                const sibling = e.currentTarget.nextElementSibling
                if (sibling && sibling instanceof HTMLElement) {
                  sibling.style.display = 'flex'
                }
              }}
            />
            <div
              className="w-8 h-8 rounded-full bg-raised flex items-center justify-center mr-2"
              style={{ display: 'none' }}
            >
              <span className="text-sm font-medium text-ink-muted">
                {author.charAt(0).toUpperCase()}
              </span>
            </div>
            <span className="text-ink">{author}</span>
          </div>
        </div>
      )}

      {tags && tags.length > 0 && (
        <div>
          <span className="text-sm text-ink-muted block mb-2">Tags</span>
          <div className="flex flex-wrap gap-2">
            {tags.map((tag) => (
              <span
                key={tag}
                className="bg-raised text-ink text-xs font-medium px-2.5 py-0.5 rounded-control"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
