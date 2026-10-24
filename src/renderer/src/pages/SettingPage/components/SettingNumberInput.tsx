import React from 'react'
import { NumberInput } from '@renderer/components/KnowledgeBase/NumberInput'

/** `SettingInput` for whole numbers: can be emptied while typing, clamped on blur. */
export const SettingNumberInput: React.FC<{
  label: string
  description?: string
  value: number
  min: number
  max: number
  step?: number
  onChange: (value: number) => void
}> = ({ label, description, ...input }) => (
  <div className="space-y-1">
    <label className="block text-sm font-medium text-ink">{label}</label>
    {description && <p className="text-xs text-ink-muted">{description}</p>}
    <NumberInput
      {...input}
      className="bg-surface border border-strong text-ink text-sm rounded-container focus:ring-accent focus:border-accent block w-full p-2.5"
    />
  </div>
)
