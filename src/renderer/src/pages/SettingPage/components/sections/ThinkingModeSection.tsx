import React from 'react'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { useTranslation } from 'react-i18next'
import { FcIdea } from 'react-icons/fc'
import { ThinkingModeBudget } from '@/types/llm'
import { SettingSection } from '../SettingSection'

/** Thinking mode: what it does, what it applies to, and the budget for the default model. */
export const ThinkingModeSection: React.FC = () => {
  const { thinkingMode, updateThinkingMode, currentLLM } = useSettings()
  const { t } = useTranslation()
  const supportsThinking = currentLLM?.supportsThinking || false
  const supportsAdaptive = currentLLM?.supportedThinkingTypes?.includes('adaptive')

  const handleBudgetChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const budget_tokens = Number(e.target.value)
    let type: 'disabled' | 'enabled' | 'adaptive'
    if (budget_tokens === 0) {
      type = 'disabled'
    } else if (supportsAdaptive) {
      type = 'adaptive'
    } else {
      type = 'enabled'
    }
    updateThinkingMode({ type, budget_tokens })
  }

  return (
    <SettingSection title={t('settings.thinking.title')} icon={FcIdea}>
      <p className="text-sm text-ink-muted">{t('settings.thinking.description')}</p>
      <p className="text-xs text-ink-muted">{t('settings.thinking.affects')}</p>
      {!supportsThinking ? (
        <p className="text-sm text-ink-muted rounded-container border border-subtle bg-raised p-3">
          {t('settings.thinking.notSupported', { model: currentLLM?.modelName ?? '' })}
        </p>
      ) : (
        <div className="space-y-1">
          <label className="block text-sm font-medium text-ink">{t('Thinking Budget')}</label>
          {supportsAdaptive ? (
            <select
              value={thinkingMode?.type === 'adaptive' ? '1' : '0'}
              onChange={(e) => {
                const val = e.target.value
                if (val === '0') {
                  updateThinkingMode({ type: 'disabled', budget_tokens: ThinkingModeBudget.NONE })
                } else {
                  updateThinkingMode({ type: 'adaptive', budget_tokens: ThinkingModeBudget.NORMAL })
                }
              }}
              className="
              bg-surface
              border border-strong
              text-ink
              text-sm rounded-container
              focus:ring-accent
              focus:border-accent
              block w-full p-2.5
            "
            >
              <option value="0">{t('None (0 tokens)')}</option>
              <option value="1">{t('thinkingMode.adaptive')}</option>
            </select>
          ) : (
            <select
              value={
                thinkingMode?.budget_tokens?.toString() || ThinkingModeBudget.NORMAL.toString()
              }
              onChange={handleBudgetChange}
              className="
              bg-surface
              border border-strong
              text-ink
              text-sm rounded-container
              focus:ring-accent
              focus:border-accent
              block w-full p-2.5
            "
            >
              <option value={ThinkingModeBudget.NONE.toString()}>{t('None (0 tokens)')}</option>
              <option value={ThinkingModeBudget.QUICK.toString()}>{t('Quick (1K tokens)')}</option>
              <option value={ThinkingModeBudget.NORMAL.toString()}>
                {t('Normal (4K tokens)')}
              </option>
              <option value={ThinkingModeBudget.DEEP.toString()}>{t('Deep (16K tokens)')}</option>
              <option value={ThinkingModeBudget.DEEPER.toString()}>
                {t('Deeper (32K tokens)')}
              </option>
            </select>
          )}
        </div>
      )}
    </SettingSection>
  )
}
