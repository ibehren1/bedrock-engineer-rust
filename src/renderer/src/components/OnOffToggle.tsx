import React from 'react'
import { ToggleSwitch, type ToggleSwitchProps } from 'flowbite-react'

/**
 * Soft green track when on, soft red when off, each with a matching hairline so the track
 * stands out from the card behind it. Merged over the app-wide toggle theme.
 */
const onOffTheme = {
  toggle: {
    base: 'inset-ring',
    checked: {
      off: 'bg-danger-soft-strong inset-ring-danger/60',
      color: { success: 'bg-success-soft-strong inset-ring-success/60' }
    }
  }
}

/** A toggle that reads as on / off at a glance (tool switches, chat grants). */
export const OnOffToggle: React.FC<Omit<ToggleSwitchProps, 'color' | 'theme'>> = ({
  sizing = 'lg',
  label = '',
  ...props
}) => <ToggleSwitch {...props} label={label} sizing={sizing} color="success" theme={onOffTheme} />
