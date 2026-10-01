import React from 'react'
import { useTranslation } from 'react-i18next'
import { TbSearchOff } from 'react-icons/tb'

export const EmptyState: React.FC = () => {
  const { t } = useTranslation()

  return (
    <div className="flex flex-col items-center justify-center h-[calc(100vh-16rem)]">
      <div className="mb-3 p-3 bg-surface rounded-full shadow-sm">
        <TbSearchOff className="w-16 h-16 text-accent" />
      </div>
      <p className="text-heading text-ink mb-2">{t('noAgentsFound')}</p>
      <p className="text-sm text-ink-muted text-center max-w-md">{t('tryDifferentSearch')}</p>
    </div>
  )
}
