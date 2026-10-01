import React from 'react'
import { useTranslation } from 'react-i18next'

interface SystemPromptSectionProps {
  systemPrompt: string
}

export const SystemPromptSection: React.FC<SystemPromptSectionProps> = ({ systemPrompt }) => {
  const { t } = useTranslation()

  return (
    <div>
      <h3 className="text-heading font-medium mb-2 text-ink flex items-center">
        <span className="mr-2">{t('systemPromptLabel')}</span>
      </h3>
      <div className="bg-surface-2 p-2.5 rounded-container overflow-y-auto max-h-[50vh] border border-subtle">
        <pre className="whitespace-pre-wrap text-sm text-ink font-mono">{systemPrompt}</pre>
      </div>
    </div>
  )
}
