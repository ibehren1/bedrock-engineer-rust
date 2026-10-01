import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiX, FiZap, FiPlus } from 'react-icons/fi'
import { ScenariosSectionProps } from './types'

export const ScenariosSection: React.FC<ScenariosSectionProps> = ({
  scenarios,
  name,
  description,
  system,
  onChange,
  isGenerating,
  onAutoGenerate
}) => {
  const { t } = useTranslation()

  const addScenario = () => {
    onChange([...scenarios, { title: '', content: '' }])
  }

  const removeScenario = (index: number) => {
    onChange(scenarios.filter((_, i) => i !== index))
  }

  const updateScenario = (index: number, field: 'title' | 'content', value: string) => {
    const updated = [...scenarios]
    updated[index] = { ...updated[index], [field]: value }
    onChange(updated)
  }

  return (
    <div className="space-y-2">
      <div className="mb-2">
        <div className="flex items-center space-x-2">
          <label className="text-sm font-medium text-ink flex-shrink-0">
            Scenarios {t('optional')}
          </label>
          {name && description && system && (
            <button
              type="button"
              onClick={onAutoGenerate}
              disabled={isGenerating}
              className="inline-flex items-center text-xs bg-accent-tint hover:bg-accent-tint-strong
              text-accent rounded-control px-1.5 py-0.5 transition-colors duration-200 border border-accent"
            >
              <FiZap className={`w-3 h-3 mr-1 ${isGenerating ? 'animate-pulse' : ''}`} />
              <span>{isGenerating ? t('generating') : t('autoGenerateScinario')}</span>
            </button>
          )}
        </div>
        <p className="text-xs text-ink-muted mb-1 mt-1">{t('scenariosDescription')}</p>
      </div>

      {isGenerating && scenarios.length === 0 ? (
        <Loading />
      ) : (
        <>
          {scenarios.length > 0 && (
            <div className="space-y-2">
              {scenarios.map((scenario, index) => (
                <div key={index} className="flex items-center space-x-2">
                  <textarea
                    value={scenario.title}
                    onChange={(e) => updateScenario(index, 'title', e.target.value)}
                    placeholder={t('scenarioTitlePlaceholder')}
                    className="flex-2 rounded-control border-strong bg-surface
                      text-ink shadow-sm focus:border-accent focus:ring-accent sm:text-sm"
                    disabled={isGenerating}
                  />
                  <textarea
                    value={scenario.content}
                    onChange={(e) => updateScenario(index, 'content', e.target.value)}
                    placeholder={t('scenarioContentPlaceholder')}
                    className="flex-1 rounded-control border-strong bg-surface
                      text-ink shadow-sm focus:border-accent focus:ring-accent sm:text-sm"
                    disabled={isGenerating}
                  />
                  <button
                    type="button"
                    onClick={() => removeScenario(index)}
                    title={t('deleteScenario')}
                    disabled={isGenerating}
                    className="flex items-center justify-center rounded-control border border-strong
                      bg-surface-2 hover:bg-raised
                      text-ink transition-colors duration-200 px-3 h-[60px]
                      disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <FiX className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}
          {isGenerating && scenarios.length > 0 && (
            <div className="mt-2 flex items-center space-x-2 text-sm text-ink-muted">
              <div className="animate-pulse h-2 w-2 bg-accent-tint rounded-full"></div>
              <span>{t('generating')}</span>
            </div>
          )}
          {!isGenerating && (
            <button
              type="button"
              onClick={addScenario}
              className="w-full mt-2 py-2 px-4 bg-surface-2 hover:bg-raised
                text-ink rounded-control transition-colors duration-200
                flex items-center justify-center space-x-2 border border-strong"
            >
              <FiPlus className="w-4 h-4" />
              <span>{t('addScenario')}</span>
            </button>
          )}
        </>
      )}
    </div>
  )
}

const Loading = () => {
  return (
    <div className="flex flex-col gap-2 w-full">
      <span className="animate-pulse h-2 w-12 bg-raised rounded-control"></span>
      <div className="flex-1 space-y-3 py-1">
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-4">
            <div className="h-2 bg-raised rounded-control col-span-2"></div>
            <div className="h-2 bg-raised rounded-control col-span-1"></div>
          </div>
          <div className="h-2 bg-raised rounded-control"></div>
        </div>
      </div>
    </div>
  )
}
