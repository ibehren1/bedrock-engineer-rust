import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiDownload } from 'react-icons/fi'
import { useAppUpdate } from '@renderer/contexts/AppUpdateContext'

/**
 * "Version X is ready: Restart now / Later", floating at the top of the window once an update
 * is in. It floats rather than taking a row, because the pages size themselves to the window.
 */
export const UpdateBanner: React.FC = () => {
  const { t } = useTranslation()
  const { status, restart, dismissed, dismiss } = useAppUpdate()
  if (status.state !== 'ready' || dismissed) return null

  return (
    <div
      role="status"
      className="fixed top-2 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 pl-3 pr-1.5 py-1.5 text-xs bg-surface text-ink border border-subtle rounded-container shadow-lg"
    >
      <FiDownload className="size-3.5 shrink-0 text-accent" />
      <span className="mr-2">{t('appUpdate.bannerReady', { version: status.version })}</span>
      <button
        onClick={() => void restart()}
        className="px-2.5 py-0.5 rounded-control bg-accent text-accent-fg hover:bg-accent-strong"
      >
        {t('appUpdate.restartNow')}
      </button>
      <button
        onClick={dismiss}
        className="px-2.5 py-0.5 rounded-control text-ink-muted hover:bg-raised hover:text-ink"
      >
        {t('appUpdate.later')}
      </button>
    </div>
  )
}
