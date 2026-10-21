import React from 'react'
import { useTranslation } from 'react-i18next'
import { LuBrain } from 'react-icons/lu'

export type MemoryToggleProps = {
  checked: boolean
  /** Enabled memory items, the ones the toggle adds to the prompt. */
  enabledCount: number
  /** No chat session yet, so there is nowhere to save the setting. */
  disabled?: boolean
  onChange: (checked: boolean) => void
}

/**
 * The per-chat "Use Memory Items" toggle, a brain button (the Memory page's icon) next to the
 * attach and export buttons. Highlighted while on; on by default.
 */
export const MemoryToggle: React.FC<MemoryToggleProps> = ({
  checked,
  enabledCount,
  disabled = false,
  onChange
}) => {
  const { t } = useTranslation()
  const tooltip = disabled
    ? t('memoryItems.noSession')
    : checked
      ? t('memoryItems.toggleOnTooltip', { count: enabledCount })
      : t('memoryItems.toggleOffTooltip', { count: enabledCount })

  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      disabled={disabled}
      aria-pressed={checked}
      title={tooltip}
      aria-label={t('memoryItems.useMemoryItems')}
      className={`relative p-2 rounded-full transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed ${
        checked ? 'bg-surface text-accent' : 'text-ink-muted hover:text-ink hover:bg-surface'
      }`}
    >
      <LuBrain />
      {checked && enabledCount > 0 && (
        <span className="absolute -top-0.5 -right-0.5 min-w-4 h-4 px-1 rounded-full bg-accent text-accent-fg text-[10px] leading-4 text-center">
          {enabledCount}
        </span>
      )}
    </button>
  )
}
