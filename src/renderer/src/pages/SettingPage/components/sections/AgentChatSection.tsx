import React from 'react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'

export const AgentChatSection: React.FC = () => {
  const { t } = useTranslation()
  const {
    tavilySearchApiKey,
    setTavilySearchApiKey,
    contextLength,
    updateContextLength,
    enablePromptCache,
    setEnablePromptCache,
    requestTimeout,
    setRequestTimeout
  } = useSettings()

  const handleContextLengthChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10)
    if (!isNaN(value) && value > 0) {
      updateContextLength(value)
    }
  }

  const handleRequestTimeoutChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10)
    if (!isNaN(value) && value > 0) {
      setRequestTimeout(value)
    }
  }

  return (
    <SettingSection title={t('Agent Chat')}>
      <div className="space-y-2">
        <SettingInput
          label={t('Tavily Search API Key')}
          type="password"
          placeholder={t('tavilySearchApiKeyPlaceholder', 'tvly-xxxxxxxxxxxxxxx')}
          value={tavilySearchApiKey}
          onChange={(e) => setTavilySearchApiKey(e.target.value)}
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

        <div className="pt-4">
          <SettingInput
            label={t('Context Length (number of messages to include in API requests)')}
            type="number"
            min={t('minContextLength', '1')}
            placeholder={t('contextLengthPlaceholder', '10')}
            value={contextLength.toString()}
            onChange={handleContextLengthChange}
          />
          <div className="mt-1 text-xs text-ink-muted">
            {t(
              'Limiting context length reduces token usage but may affect conversation continuity'
            )}
          </div>
        </div>

        <div className="pt-4">
          <SettingInput
            label={t('Request Timeout (minutes)')}
            type="number"
            min="1"
            placeholder="15"
            value={requestTimeout.toString()}
            onChange={handleRequestTimeoutChange}
          />
          <div className="mt-1 text-xs text-ink-muted">
            {t('Maximum time to wait for agent responses before timing out')}
          </div>
        </div>

        <div className="pt-4">
          <div className="flex items-center">
            <input
              type="checkbox"
              id="enable-prompt-cache"
              className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent bg-raised ring-offset-surface"
              checked={enablePromptCache}
              onChange={(e) => setEnablePromptCache(e.target.checked)}
            />
            <label
              htmlFor="enable-prompt-cache"
              className="ml-2 block text-sm font-medium text-ink"
            >
              {t('Enable Prompt Cache')}
            </label>
          </div>
          <div className="mt-1 text-xs text-ink-muted ml-6">
            {t('Prompt Cache reduces token usage by caching parts of the conversation')}
          </div>
        </div>
      </div>
    </SettingSection>
  )
}
