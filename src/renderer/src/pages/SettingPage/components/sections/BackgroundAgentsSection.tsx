import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'

/** Store key and bounds, as `background::scheduler` reads them. */
const KEY = 'backgroundAgentMaxToolCalls'
export const DEFAULT_BACKGROUND_MAX_TOOL_CALLS = 100
const MIN = 1
const MAX = 1000

/** The tool-call budget of one scheduled background agent run (AGENT-D5). */
export const BackgroundAgentsSection: React.FC = () => {
  const { t } = useTranslation()
  const stored = window.store.get(KEY)
  const [value, setValue] = useState<string>(
    String(typeof stored === 'number' ? stored : DEFAULT_BACKGROUND_MAX_TOOL_CALLS)
  )
  const parsed = Number(value)
  const valid = Number.isInteger(parsed) && parsed >= MIN && parsed <= MAX

  return (
    <SettingSection
      title={t('backgroundAgentsSettings.title')}
      description={t('backgroundAgentsSettings.description')}
    >
      <SettingInput
        label={t('backgroundAgentsSettings.maxToolCalls')}
        description={t('backgroundAgentsSettings.maxToolCallsHint', {
          min: MIN,
          max: MAX,
          default: DEFAULT_BACKGROUND_MAX_TOOL_CALLS
        })}
        type="number"
        min={MIN}
        max={MAX}
        value={value}
        error={valid ? undefined : t('backgroundAgentsSettings.invalid', { min: MIN, max: MAX })}
        onChange={(e) => {
          setValue(e.target.value)
          const n = Number(e.target.value)
          if (Number.isInteger(n) && n >= MIN && n <= MAX) window.store.set(KEY, n)
        }}
      />
    </SettingSection>
  )
}
