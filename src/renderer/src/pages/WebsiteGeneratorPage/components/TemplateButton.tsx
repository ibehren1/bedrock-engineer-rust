import React from 'react'
import { SupportedTemplate } from '../templates'
import { sleep } from '@renderer/lib/util'

interface TemplateButtonProps extends SupportedTemplate {
  isSelected: boolean
  onSelect: (id: SupportedTemplate['id']) => void
  onRefresh: () => void
}

export const TemplateButton: React.FC<TemplateButtonProps> = ({
  id,
  name,
  logo,
  isSelected,
  onSelect,
  onRefresh
}) => {
  return (
    <button
      type="button"
      className={`
        text-ink
        ${isSelected ? 'bg-success-soft' : 'bg-surface'}
        hover:bg-success-soft-strong
        border
        ${isSelected ? 'border-success' : 'border-subtle'}
        focus:ring-4
        focus:outline-none
        focus:ring-accent
        font-medium
        rounded-full
        text-xs
        px-3
        py-1.5
        inline-flex
        items-center
        flex
        gap-2
        bg-surface
        text-ink
        border-subtle
        hover:bg-raised
      `}
      onClick={async () => {
        onSelect(id)
        await sleep(100)
        onRefresh()
      }}
    >
      <div className="w-[18px]">{logo}</div>
      <span>{name}</span>
    </button>
  )
}
