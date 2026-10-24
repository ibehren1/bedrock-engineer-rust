import React from 'react'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingNumberInput } from '../SettingNumberInput'
import { KnowledgeSettings } from '@/types/knowledge'

export const KnowledgeRetrievalSection: React.FC<{
  settings: KnowledgeSettings
  onChange: (next: KnowledgeSettings) => void
}> = ({ settings, onChange }) => {
  const { t } = useTranslation()
  return (
    <SettingSection
      title={t('knowledgeBase.settings.retrievalTitle')}
      description={t('knowledgeBase.settings.retrievalDescription')}
    >
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <SettingNumberInput
          label={t('knowledgeBase.settings.chunkSize')}
          min={50}
          max={2000}
          value={settings.chunkSize}
          onChange={(chunkSize) => onChange({ ...settings, chunkSize })}
        />
        <SettingNumberInput
          label={t('knowledgeBase.settings.chunkOverlap')}
          min={0}
          max={Math.floor(settings.chunkSize / 2)}
          value={settings.chunkOverlap}
          onChange={(chunkOverlap) => onChange({ ...settings, chunkOverlap })}
        />
        <SettingNumberInput
          label={t('knowledgeBase.settings.topK')}
          min={1}
          max={50}
          value={settings.topK}
          onChange={(topK) => onChange({ ...settings, topK })}
        />
      </div>
      <div className="space-y-1">
        <label className="block text-sm font-medium text-ink">
          {t('knowledgeBase.settings.hybridWeight')}: {settings.hybridWeight.toFixed(2)}
        </label>
        <p className="text-xs text-ink-muted">{t('knowledgeBase.settings.hybridWeightHint')}</p>
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          className="w-full"
          value={settings.hybridWeight}
          onChange={(e) => onChange({ ...settings, hybridWeight: Number(e.target.value) })}
        />
      </div>
    </SettingSection>
  )
}
