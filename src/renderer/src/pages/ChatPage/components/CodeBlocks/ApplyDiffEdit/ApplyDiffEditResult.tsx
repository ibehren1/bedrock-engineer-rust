import React from 'react'
import { useTranslation } from 'react-i18next'
import { DiffViewer } from './DiffViewer'
import { ApplyDiffEditResultProps } from './types'

export const ApplyDiffEditResult: React.FC<ApplyDiffEditResultProps> = ({ response }) => {
  const { t } = useTranslation()

  // If there was an error
  if (!response.success) {
    return (
      <div className="bg-danger-soft text-danger p-2.5 rounded-control">
        <h3 className="text-heading font-semibold">{t('errors.failedToApplyChanges')}</h3>
        <p>{response.error}</p>
      </div>
    )
  }

  // If there's no result data
  if (!response.result) {
    return (
      <div className="bg-warning-soft text-warning p-2.5 rounded-control">
        <h3 className="text-heading font-semibold">{t('common.noResults')}</h3>
      </div>
    )
  }

  return (
    <DiffViewer
      originalText={response.result.originalText}
      updatedText={response.result.updatedText}
      filePath={response.result.path}
    />
  )
}
