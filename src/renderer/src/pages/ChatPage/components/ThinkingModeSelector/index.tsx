import React, { useState, useRef, useEffect } from 'react'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { ThinkingModeBudget } from '@/types/llm'
import { LuBrain } from 'react-icons/lu'
import { useTranslation } from 'react-i18next'
import { FiChevronDown } from 'react-icons/fi'

type ThinkingModeSelectorProps = {
  className?: string
  /** Rendered next to the control, and hidden with it on models without thinking support */
  label?: string
}

export const ThinkingModeSelector: React.FC<ThinkingModeSelectorProps> = ({ className, label }) => {
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const { currentLLM, thinkingMode, updateThinkingMode } = useSettings()
  const { t } = useTranslation()

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  // Only show for models that support thinking (Claude 3.7 Sonnet, Claude 4)
  if (!currentLLM.supportsThinking) {
    return null
  }

  const supportsAdaptive = currentLLM.supportedThinkingTypes?.includes('adaptive')

  const options = supportsAdaptive
    ? [
        { label: t('thinkingMode.none'), value: ThinkingModeBudget.NONE },
        { label: t('thinkingMode.adaptive'), value: ThinkingModeBudget.NORMAL }
      ]
    : [
        { label: t('thinkingMode.none'), value: ThinkingModeBudget.NONE },
        { label: t('thinkingMode.quick'), value: ThinkingModeBudget.QUICK },
        { label: t('thinkingMode.normal'), value: ThinkingModeBudget.NORMAL },
        { label: t('thinkingMode.deep'), value: ThinkingModeBudget.DEEP },
        { label: t('thinkingMode.deeper'), value: ThinkingModeBudget.DEEPER }
      ]

  const getSelectedLabel = () => {
    if (supportsAdaptive && thinkingMode?.type === 'adaptive') {
      return t('thinkingMode.adaptive')
    }
    const selected = options.find((option) => option.value === thinkingMode?.budget_tokens)
    return selected ? selected.label : options[0].label
  }

  return (
    <div className={`relative flex items-center gap-1.5 ${className || ''}`} ref={dropdownRef}>
      {label && <span className="text-xs text-ink-muted">{label}</span>}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1 text-sm text-ink-muted rounded-control transition-colors"
        title={t('thinkingMode.title')}
      >
        <LuBrain className="size-4 text-accent" />
        <span className="whitespace-nowrap text-ink-muted">{getSelectedLabel()}</span>
        <FiChevronDown className="text-ink-faint" size={16} />
      </button>

      {isOpen && (
        <div
          className="absolute z-20 w-72 bottom-full mb-1 bg-surface rounded-container shadow-lg
          border border-subtle py-1"
        >
          {options.map((option) => (
            <div
              key={option.value}
              onClick={() => {
                let type: 'disabled' | 'enabled' | 'adaptive'
                if (option.value === 0) {
                  type = 'disabled'
                } else if (supportsAdaptive) {
                  type = 'adaptive'
                } else {
                  type = 'enabled'
                }
                updateThinkingMode({
                  type,
                  budget_tokens: option.value
                })
                setIsOpen(false)
              }}
              className={`
                flex items-center gap-3 px-3 py-2 cursor-pointer
                ${thinkingMode?.budget_tokens === option.value ? 'bg-surface-2' : 'bg-surface'}
                hover:bg-raised
                transition-colors
              `}
            >
              <span className="text-sm whitespace-nowrap text-ink">{option.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
