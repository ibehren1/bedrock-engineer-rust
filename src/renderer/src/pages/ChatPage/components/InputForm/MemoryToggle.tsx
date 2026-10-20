import React from 'react'
import { useTranslation } from 'react-i18next'
import { Tooltip } from 'flowbite-react'

export type MemoryToggleProps = {
  checked: boolean
  /** Enabled memory items, the ones the checkbox adds to the prompt. */
  enabledCount: number
  /** No chat session yet, so there is nowhere to save the setting. */
  disabled?: boolean
  onChange: (checked: boolean) => void
}

/** The "Use Memory Items" checkbox next to Tools in the chat input area. */
export const MemoryToggle: React.FC<MemoryToggleProps> = ({
  checked,
  enabledCount,
  disabled = false,
  onChange
}) => {
  const { t } = useTranslation()
  return (
    <Tooltip
      content={disabled ? t('memoryItems.noSession') : t('memoryItems.useMemoryItemsTooltip')}
      placement="top"
      animation="duration-500"
    >
      <label
        className={`flex items-center gap-1.5 text-xs text-ink ${
          disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'
        }`}
      >
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
        />
        <span>{t('memoryItems.useMemoryItems')}</span>
        <span className="rounded-full bg-accent-tint text-accent px-1.5 text-micro">
          {enabledCount}
        </span>
      </label>
    </Tooltip>
  )
}
