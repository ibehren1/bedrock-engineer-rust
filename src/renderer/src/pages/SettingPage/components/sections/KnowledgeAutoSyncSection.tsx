import React from 'react'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingNumberInput } from '../SettingNumberInput'
import { KnowledgeCheckbox } from './KnowledgeCrawlerSection'
import { KnowledgeSettings } from '@/types/knowledge'

export const KnowledgeAutoSyncSection: React.FC<{
  settings: KnowledgeSettings
  onChange: (next: KnowledgeSettings) => void
}> = ({ settings, onChange }) => {
  const { t } = useTranslation()
  const a = settings.autoSync
  const set = (patch: Partial<KnowledgeSettings['autoSync']>) =>
    onChange({ ...settings, autoSync: { ...a, ...patch } })
  return (
    <SettingSection
      title={t('knowledgeBase.settings.autoSyncTitle')}
      description={t('knowledgeBase.settings.autoSyncDescription')}
    >
      <KnowledgeCheckbox
        id="kb-auto-sync"
        label={t('knowledgeBase.settings.autoSyncEnabled')}
        checked={a.enabled}
        onChange={(enabled) => set({ enabled })}
      />
      {a.enabled && (
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          <SettingNumberInput
            label={t('knowledgeBase.settings.autoSyncInterval')}
            min={5}
            max={1440}
            step={5}
            value={a.intervalMinutes}
            onChange={(intervalMinutes) => set({ intervalMinutes })}
          />
        </div>
      )}
    </SettingSection>
  )
}
