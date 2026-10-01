import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcMindMap } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'
import { ThinkingModeSettings } from '../ThinkingModeSettings'

export const InferenceParametersSection: React.FC = () => {
  const { t } = useTranslation()
  const { currentLLM, inferenceParams, updateInferenceParams } = useSettings()

  return (
    <SettingSection title={t('Inference Parameters')} icon={FcMindMap}>
      <div className="space-y-2">
        <SettingInput
          label={t('Max Tokens')}
          type="number"
          placeholder={t('Max tokens')}
          value={inferenceParams.maxTokens}
          min={1}
          max={currentLLM?.maxTokensLimit || 8192}
          onChange={(e) => {
            updateInferenceParams({ maxTokens: parseInt(e.target.value, 10) })
          }}
        />

        <SettingInput
          label={t('Temperature')}
          type="number"
          placeholder={t('Temperature')}
          value={inferenceParams.temperature}
          min={0}
          max={1.0}
          step={0.1}
          onChange={(e) => {
            updateInferenceParams({ temperature: parseFloat(e.target.value) })
          }}
        />

        <SettingInput
          label={t('topP')}
          type="number"
          placeholder={t('topP')}
          value={inferenceParams.topP}
          min={0}
          max={1}
          step={0.1}
          onChange={(e) => {
            updateInferenceParams({ topP: parseFloat(e.target.value) })
          }}
        />

        <ThinkingModeSettings />
      </div>
    </SettingSection>
  )
}
