import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiExternalLink, FiFile, FiFolder } from 'react-icons/fi'

type NoPreviewProps = {
  message?: string
  onOpen: () => void
  onReveal: () => void
  revealLabel: string
}

/** Files the panel can't show: say so and offer the OS app or the folder. */
export const NoPreview: React.FC<NoPreviewProps> = ({ message, onOpen, onReveal, revealLabel }) => {
  const { t } = useTranslation()
  return (
    <div className="h-full grid place-items-center p-6 text-center">
      <div className="flex flex-col items-center gap-3">
        <FiFile className="size-8 text-ink-faint" />
        <div className="text-sm text-ink-faint">{message ?? t('filesPanel.noPreview')}</div>
        <div className="flex gap-2">
          <button
            onClick={onOpen}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-control border border-subtle text-ink hover:bg-raised"
          >
            <FiExternalLink className="size-3.5" />
            {t('filesPanel.openInDefaultApp')}
          </button>
          <button
            onClick={onReveal}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-control text-ink-muted hover:bg-raised hover:text-ink"
          >
            <FiFolder className="size-3.5" />
            {revealLabel}
          </button>
        </div>
      </div>
    </div>
  )
}

export default NoPreview
