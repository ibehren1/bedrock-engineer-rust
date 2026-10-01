import React, { useState, useRef, useEffect } from 'react'
import { LLM } from '@/types/llm'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { PricingCalculator } from '@common/models/pricing'
import { FiChevronDown } from 'react-icons/fi'
import { getModelIcon, isWideModelIcon } from '@renderer/components/ModelIcon'

type ModelSelectorProps = {
  openable: boolean
  value?: string // 外部からのモデルID指定
  onChange?: (modelId: string) => void // 外部への変更通知
  className?: string // 追加のスタイリング
}

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  openable,
  value,
  onChange,
  className
}) => {
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const { currentLLM, updateLLM, availableModels, visibleModels } = useSettings()

  // 外部から値が指定されている場合はそれを使用、そうでなければcurrentLLMを使用
  const selectedModelId = value || currentLLM.modelId
  const selectedModel =
    availableModels.find((model) => model.modelId === selectedModelId) || currentLLM

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const handleModelSelect = (model: LLM) => {
    if (onChange) {
      // 外部制御モード：onChangeコールバックを呼び出し
      onChange(model.modelId)
    } else {
      // デフォルトモード：設定を更新
      updateLLM(model)
    }
    setIsOpen(false)
  }

  const modelColors = {
    icon: 'text-ink-muted',
    hover: 'hover:bg-surface-2'
  }

  // Pricing stored in the model config is per 1,000 tokens; display it per
  // 1,000,000 tokens to match AWS Bedrock's published pricing convention.
  const getModelPricingLabel = (modelId: string): string | null => {
    const pricing = new PricingCalculator(modelId).getPricing()
    if (!pricing) return null
    const perMillion = (perThousand: number) =>
      (perThousand * 1000).toLocaleString('en-US', {
        style: 'currency',
        currency: 'USD',
        minimumFractionDigits: 2,
        maximumFractionDigits: 2
      })
    return `${perMillion(pricing.input)} in / ${perMillion(pricing.output)} out per 1M tokens`
  }

  return (
    <div
      className={`justify-start flex items-center relative ${className || ''}`}
      ref={dropdownRef}
    >
      <div className="relative">
        {isOpen && (
          <div
            className="absolute z-20 w-[25rem] bottom-full mb-1 bg-surface rounded-container shadow-lg
            border border-subtle py-2 px-2 max-h-[40vh] overflow-y-auto"
          >
            {visibleModels.map((model: LLM) => {
              const isInferenceProfile = model.isInferenceProfile || false
              return (
                <div
                  key={model.modelId}
                  onClick={() => handleModelSelect(model)}
                  className={`
                    flex items-center gap-4 px-3 py-2.5 cursor-pointer
                    ${model.modelId === selectedModelId ? 'bg-surface-2' : 'bg-surface'}
                    ${modelColors.hover}
                    transition-colors rounded-control
                  `}
                  title={
                    isInferenceProfile
                      ? `Application Inference Profile: ${model.inferenceProfileArn}`
                      : ''
                  }
                >
                  {/*
                    アイコン列の幅を固定し、横長のワードマークにはその幅いっぱいを使わせる。
                    A fixed-width icon column keeps every row's text aligned while letting a
                    wide wordmark use twice the width a square glyph needs.
                  */}
                  <div
                    className={`rounded-control shrink-0 w-8 flex items-center justify-center ${modelColors.icon} ${
                      isWideModelIcon(model.modelId) ? 'h-[12px]' : ''
                    }`}
                  >
                    {getModelIcon(model.modelId, isInferenceProfile)}
                  </div>
                  <div className="flex flex-col">
                    <span className="font-medium text-ink">
                      {model.modelName}
                      {isInferenceProfile && (
                        <span className="ml-2 px-2 py-0.5 text-xs bg-accent-tint text-accent rounded-control">
                          Profile
                        </span>
                      )}
                    </span>
                    <span className="text-xs text-ink-muted mt-0.5">
                      {isInferenceProfile
                        ? model.description || 'Application Inference Profile for cost tracking'
                        : model.toolUse
                          ? 'Supports tool use'
                          : 'Does not support tool use'}
                    </span>
                    {!isInferenceProfile &&
                      (() => {
                        const pricingLabel = getModelPricingLabel(model.modelId)
                        return pricingLabel ? (
                          <span className="text-xs text-ink-faint mt-0.5">{pricingLabel}</span>
                        ) : null
                      })()}
                  </div>
                </div>
              )
            })}
          </div>
        )}

        <button
          type="button"
          onClick={() => (openable ? setIsOpen(!isOpen) : undefined)}
          className="flex items-center gap-1 text-sm text-ink-muted rounded-control transition-colors"
        >
          <span className="flex items-center gap-1.5">
            <span
              className={`${modelColors.icon} ${
                // 横長のワードマークは1行の高さに収まる範囲で幅を広く取る。
                // Wide marks take the width a single-line button can afford.
                isWideModelIcon(selectedModel.modelId) ? 'flex items-center w-7 h-[11px]' : ''
              }`}
            >
              {getModelIcon(selectedModel.modelId, selectedModel.isInferenceProfile)}
            </span>
            <span className="text-left whitespace-nowrap">{selectedModel.modelName}</span>
            <FiChevronDown className="text-ink-faint" size={16} />
          </span>
        </button>
      </div>
    </div>
  )
}
