import React from 'react'
import { useTranslation } from 'react-i18next'

interface TagFilterProps {
  tags: string[]
  selectedTags: string[]
  onSelectTag: (tag: string) => void
}

export const TagFilter: React.FC<TagFilterProps> = ({ tags, selectedTags, onSelectTag }) => {
  const { t } = useTranslation()

  if (tags.length === 0) {
    return null
  }

  return (
    <div className="flex overflow-x-auto gap-2 mb-4 pb-2">
      {tags.map((tag) => (
        <button
          key={tag}
          onClick={() => onSelectTag(tag)}
          className={`px-3 py-1 text-sm font-medium rounded-container transition-colors whitespace-nowrap ${
            selectedTags.includes(tag)
              ? 'bg-accent-tint text-accent'
              : 'bg-raised text-ink hover:bg-sunken'
          }`}
        >
          {tag}
        </button>
      ))}
      {selectedTags.length > 0 && (
        <button
          onClick={() => {
            // Clear all tags by clicking each selected tag again
            selectedTags.forEach((tag) => onSelectTag(tag))
          }}
          className="sticky right-0 px-3 py-1 text-sm font-medium text-danger bg-danger-soft hover:bg-danger-soft-strong rounded-container whitespace-nowrap shadow-[-8px_0_8px_-4px_rgba(0,0,0,0.1)]"
        >
          {t('clearAll')}
        </button>
      )}
    </div>
  )
}
