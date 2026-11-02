import React from 'react'
import { useTranslation } from 'react-i18next'

/**
 * Under a password field whose saved value the renderer never receives: says one is saved and
 * offers to remove it. Typing in the field replaces it; leaving it empty keeps it.
 */
export const SavedSecretNote: React.FC<{ saved: boolean; onClear: () => void }> = ({
  saved,
  onClear
}) => {
  const { t } = useTranslation()
  if (!saved) return null
  return (
    <div className="-mt-1 flex items-center gap-2 text-xs text-ink-muted">
      <span>{t('savedSecret.saved')}</span>
      <button type="button" onClick={onClear} className="text-accent hover:underline">
        {t('savedSecret.clear')}
      </button>
    </div>
  )
}
