import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcElectronics } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'

/** The default conversation model and the light processing model, side by side. */
export const DefaultModelsSection: React.FC = () => {
  const { t } = useTranslation()
  const {
    currentLLM,
    updateLLM,
    availableModels,
    lightProcessingModel,
    updateLightProcessingModel
  } = useSettings()

  const mainOptions = availableModels.map((model) => ({
    value: model.modelId,
    label: model.modelName
  }))

  // Models suited to light work (Haiku, Sonnet, Nova) are the ones offered here.
  const lightOptions = [
    { value: '', label: t('settings.lightModel.useMain') },
    ...availableModels
      .filter(
        (model) =>
          model.modelId.includes('haiku') ||
          model.modelId.includes('sonnet') ||
          model.modelId.includes('nova')
      )
      .map((model) => ({ value: model.modelId, label: model.modelName }))
  ]

  const handleMainChange = (modelId: string) => {
    const selected = availableModels.find((model) => model.modelId === modelId)
    if (selected) {
      updateLLM(selected)
    } else {
      console.error(t('Invalid model'))
    }
  }

  const handleLightChange = (modelId: string) => {
    if (!modelId) {
      // null means "use the default model"
      updateLightProcessingModel(null)
      return
    }
    const selected = availableModels.find((model) => model.modelId === modelId)
    if (selected) {
      updateLightProcessingModel(selected)
    }
  }

  return (
    <SettingSection title={t('settings.defaultModels.title')} icon={FcElectronics}>
      {/* Description under each select, so the two selects line up whatever the text length. */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="space-y-1">
          <SettingSelect
            label={t('settings.defaultModels.mainLabel')}
            value={currentLLM?.modelId}
            options={mainOptions}
            onChange={(e) => handleMainChange(e.target.value)}
          />
          <p className="text-xs text-ink-muted">{t('settings.defaultModels.mainDescription')}</p>
        </div>
        <div className="space-y-1">
          <SettingSelect
            label={t('settings.defaultModels.lightLabel')}
            value={lightProcessingModel?.modelId || ''}
            options={lightOptions}
            onChange={(e) => handleLightChange(e.target.value)}
          />
          <p className="text-xs text-ink-muted">{t('settings.defaultModels.lightDescription')}</p>
        </div>
      </div>
    </SettingSection>
  )
}
