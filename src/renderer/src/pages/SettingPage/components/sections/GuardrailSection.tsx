import React from 'react'
import { useTranslation } from 'react-i18next'
import { FaShieldAlt } from 'react-icons/fa'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'
import { SettingSelect } from '../SettingSelect'

export const GuardrailSection: React.FC = () => {
  const { t } = useTranslation()
  const { guardrailSettings, updateGuardrailSettings } = useSettings()

  const traceOptions = [
    { value: 'enabled', label: t('Enabled') },
    { value: 'disabled', label: t('Disabled') }
  ]

  return (
    <SettingSection title={t('Amazon Bedrock Guardrails')} icon={FaShieldAlt}>
      <div className="space-y-2">
        <div className="space-y-2">
          <label className="inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              className="form-checkbox h-4 w-4 text-accent rounded-control border-strong
                focus:ring-accent ring-offset-surface
                focus:ring-2 bg-raised border-subtle"
              checked={guardrailSettings.enabled}
              onChange={(e) => updateGuardrailSettings({ enabled: e.target.checked })}
            />
            <span className="ml-2 text-sm text-ink">{t('Enable Guardrails')}</span>
          </label>
          <p className="text-xs text-ink-muted">
            {t(
              'When enabled, guardrails will be applied to all model interactions to filter harmful content.'
            )}
          </p>
        </div>

        {guardrailSettings.enabled && (
          <>
            <SettingInput
              label={t('Guardrail Identifier')}
              type="text"
              placeholder="gr-xxxxxxxxxx"
              value={guardrailSettings.guardrailIdentifier}
              onChange={(e) => updateGuardrailSettings({ guardrailIdentifier: e.target.value })}
              description={t('The ID of the guardrail you want to use')}
            />

            <SettingInput
              label={t('Guardrail Version')}
              type="text"
              placeholder="DRAFT"
              value={guardrailSettings.guardrailVersion}
              onChange={(e) => updateGuardrailSettings({ guardrailVersion: e.target.value })}
              description={t('The version of the guardrail (DRAFT or a version number)')}
            />

            <SettingSelect
              label={t('Trace')}
              value={guardrailSettings.trace}
              options={traceOptions}
              onChange={(e) =>
                updateGuardrailSettings({ trace: e.target.value as 'enabled' | 'disabled' })
              }
            />
          </>
        )}
      </div>
    </SettingSection>
  )
}
