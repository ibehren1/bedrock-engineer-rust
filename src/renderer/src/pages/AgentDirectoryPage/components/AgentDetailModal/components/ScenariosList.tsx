import React from 'react'
import { useTranslation } from 'react-i18next'
import { Scenario } from '@/types/agent-chat'

interface ScenariosListProps {
  scenarios: Scenario[]
}

export const ScenariosList: React.FC<ScenariosListProps> = ({ scenarios }) => {
  const { t } = useTranslation()

  if (!scenarios || scenarios.length === 0) {
    return null
  }

  return (
    <div>
      <h3 className="text-heading font-medium mb-2 text-ink">{t('scenariosLabel')}</h3>
      <div className="space-y-3">
        {scenarios.map((scenario, index) => (
          <div key={index} className="p-2.5 bg-surface-2 rounded-container border border-subtle">
            <h4 className="font-medium text-sm text-ink mb-2">{scenario.title}</h4>
            <div className="text-sm text-ink-muted whitespace-pre-wrap">{scenario.content}</div>
          </div>
        ))}
      </div>
    </div>
  )
}
