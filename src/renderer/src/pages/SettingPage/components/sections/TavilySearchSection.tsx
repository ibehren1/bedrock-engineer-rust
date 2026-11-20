import React from 'react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SavedSecretNote } from '../SavedSecretNote'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'

/** The Tavily API key used by the tavilySearch tool. */
export const TavilySearchSection: React.FC = () => {
  const { t } = useTranslation()
  const { tavilySearchApiKey, savedTavilyApiKey, clearSavedTavilyApiKey, setTavilySearchApiKey } =
    useSettings()

  return (
    <SettingSection title={t('Tavily Search')} description={t('tavilySearch.sectionDescription')}>
      <div className="space-y-2">
        <SettingInput
          label={t('Tavily Search API Key')}
          type="password"
          placeholder={
            savedTavilyApiKey
              ? t('savedSecret.placeholder')
              : t('tavilySearchApiKeyPlaceholder', 'tvly-xxxxxxxxxxxxxxx')
          }
          value={tavilySearchApiKey}
          onChange={(e) => setTavilySearchApiKey(e.target.value)}
        />
        <SavedSecretNote
          saved={savedTavilyApiKey && !tavilySearchApiKey}
          onClear={clearSavedTavilyApiKey}
        />
        <div className="flex gap-1 text-xs text-ink">
          <span>{t('Learn more about Tavily Search, go to')}</span>
          <button
            onClick={() => window.open(t('tavilySearchUrl', 'https://tavily.com/'))}
            className="text-accent hover:underline"
          >
            {t('tavilySearchUrl', 'https://tavily.com/')}
          </button>
        </div>
      </div>
    </SettingSection>
  )
}
