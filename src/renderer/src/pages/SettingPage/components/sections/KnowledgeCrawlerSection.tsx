import React from 'react'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingNumberInput } from '../SettingNumberInput'
import { KnowledgeSettings } from '@/types/knowledge'

const Checkbox: React.FC<{
  id: string
  label: string
  checked: boolean
  onChange: (v: boolean) => void
}> = ({ id, label, checked, onChange }) => (
  <div className="flex items-center">
    <input
      type="checkbox"
      id={id}
      className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent bg-raised ring-offset-surface"
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
    />
    <label htmlFor={id} className="ml-2 block text-sm font-medium text-ink">
      {label}
    </label>
  </div>
)

export const KnowledgeCrawlerSection: React.FC<{
  settings: KnowledgeSettings
  onChange: (next: KnowledgeSettings) => void
}> = ({ settings, onChange }) => {
  const { t } = useTranslation()
  const c = settings.crawler
  const set = (patch: Partial<KnowledgeSettings['crawler']>) =>
    onChange({ ...settings, crawler: { ...c, ...patch } })
  return (
    <SettingSection
      title={t('knowledgeBase.settings.crawlerTitle')}
      description={t('knowledgeBase.settings.crawlerDescription')}
    >
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <SettingNumberInput
          label={t('knowledgeBase.settings.maxDepth')}
          min={0}
          max={10}
          value={c.maxDepth}
          onChange={(maxDepth) => set({ maxDepth })}
        />
        <SettingNumberInput
          label={t('knowledgeBase.settings.maxPages')}
          min={1}
          max={1000}
          value={c.maxPages}
          onChange={(maxPages) => set({ maxPages })}
        />
        <SettingNumberInput
          label={t('knowledgeBase.settings.delayMs')}
          min={0}
          max={60000}
          step={100}
          value={c.delayMs}
          onChange={(delayMs) => set({ delayMs })}
        />
      </div>
      <Checkbox
        id="kb-respect-robots"
        label={t('knowledgeBase.settings.respectRobots')}
        checked={c.respectRobots}
        onChange={(v) => set({ respectRobots: v })}
      />
      <Checkbox
        id="kb-use-sitemap"
        label={t('knowledgeBase.settings.useSitemap')}
        checked={c.useSitemap}
        onChange={(v) => set({ useSitemap: v })}
      />
    </SettingSection>
  )
}
