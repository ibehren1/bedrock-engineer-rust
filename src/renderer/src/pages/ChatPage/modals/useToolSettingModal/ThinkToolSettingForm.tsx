import React from 'react'
import { useTranslation } from 'react-i18next'

export const ThinkToolSettingForm: React.FC = () => {
  const { t } = useTranslation()

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-base font-bold text-ink mb-2">{t('Think Tool')}</h3>
        <p className="text-sm text-ink-muted">
          {t(
            'The think tool gives the AI a dedicated space to reason through complex problems during a conversation, without changing data or fetching new information.'
          )}
        </p>
      </div>

      <div className="bg-accent-tint p-2.5 rounded-control">
        <h5 className="font-medium mb-2">{t('How to use')}</h5>
        <p className="text-sm text-ink mb-3">
          {t(
            'The think tool provides a dedicated space for the AI to stop and think during complex tasks. It helps the AI analyze information, plan next steps, and make better decisions without changing any data or fetching new information. Especially useful for multi-step problems and policy compliance.'
          )}
        </p>
        <p className="text-sm text-ink">
          {t(
            'Especially useful for multi-step problems and policy compliance. The AI will automatically use this tool when needed for complex reasoning.'
          )}
        </p>
      </div>
    </div>
  )
}
