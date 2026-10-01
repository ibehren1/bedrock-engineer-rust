import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcIdea } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'

export const LightModelSection: React.FC = () => {
  const { t } = useTranslation()
  const { availableModels, lightProcessingModel, updateLightProcessingModel } = useSettings()

  // 軽量処理に適したモデル（Haiku や Sonnet など）を優先表示
  const lightModels = availableModels.filter(
    (model) =>
      model.modelId.includes('haiku') ||
      model.modelId.includes('sonnet') ||
      model.modelId.includes('nova')
  )

  const modelOptions = [
    { value: '', label: t('settings.lightModel.useMain') },
    ...lightModels.map((model) => ({
      value: model.modelId,
      label: model.modelName
    }))
  ]

  const handleLightModelChange = (modelId: string) => {
    if (!modelId) {
      // 空の値が選択された場合は null をセット（メインモデル使用を示す）
      updateLightProcessingModel(null)
      return
    }

    const selectedModel = availableModels.find((model) => model.modelId === modelId)
    if (selectedModel) {
      updateLightProcessingModel(selectedModel)
    }
  }

  return (
    <SettingSection title={t('settings.lightModel.title')} icon={FcIdea}>
      <div className="space-y-2">
        <p className="text-sm text-ink-muted">{t('settings.lightModel.description')}</p>

        <SettingSelect
          label={t('settings.lightModel.title')}
          value={lightProcessingModel?.modelId || ''}
          options={modelOptions}
          onChange={(e) => handleLightModelChange(e.target.value)}
        />

        <p className="text-xs text-ink-muted mt-1">{t('settings.lightModel.info')}</p>
      </div>
    </SettingSection>
  )
}
