import React from 'react'

interface ToggleSwitchProps {
  enabled: boolean
  onToggle: (e: React.MouseEvent) => void
  disabled?: boolean
  enabledLabel?: string
  disabledLabel?: string
}

export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({
  enabled,
  onToggle,
  disabled = false,
  enabledLabel,
  disabledLabel
}) => (
  <button
    onClick={onToggle}
    disabled={disabled}
    className={`relative inline-flex h-4 w-7 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-1 ${
      enabled ? 'bg-success' : 'bg-sunken'
    } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
    aria-pressed={enabled}
    aria-label={enabled ? enabledLabel : disabledLabel}
  >
    <span
      className={`inline-block h-3 w-3 transform rounded-full bg-knob shadow-raised transition ${
        enabled ? 'translate-x-3.5' : 'translate-x-0.5'
      }`}
    />
  </button>
)
