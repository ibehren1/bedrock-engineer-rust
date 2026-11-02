import React from 'react'
import { useTranslation } from 'react-i18next'

/** Shown instead of remote web content while the proxy rule blocks it (see `webContent.ts`). */
export const WebContentBlocked: React.FC<{ reason?: string | null; compact?: boolean }> = ({
  reason,
  compact
}) => {
  const { t } = useTranslation()
  return (
    <div
      className={`rounded-control border border-subtle bg-surface text-ink-muted ${
        compact ? 'inline-block px-2 py-1 text-xs' : 'm-4 p-4 text-sm'
      }`}
    >
      <p className="font-medium text-ink">{t('webContentBlocked.title')}</p>
      {!compact && <p className="mt-1">{reason || t('webContentBlocked.fallback')}</p>}
    </div>
  )
}
