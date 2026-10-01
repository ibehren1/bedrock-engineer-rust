import React from 'react'

interface SettingSelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label: string
  description?: string
  error?: string
  options: {
    label: string
    value: string
    disabled?: boolean
  }[]
  groups?: {
    label: string
    options: {
      label: string
      value: string
      disabled?: boolean
    }[]
  }[]
}

export const SettingSelect: React.FC<SettingSelectProps> = ({
  label,
  description,
  error,
  options,
  groups,
  className,
  ...selectProps
}) => {
  return (
    <div className="space-y-1">
      <label className="block text-sm font-medium text-ink">{label}</label>
      {description && <p className="text-xs text-ink-muted">{description}</p>}
      <select
        {...selectProps}
        className={`
          bg-surface
          border border-strong
          text-ink
          text-sm rounded-container
          focus:ring-accent
          focus:border-accent
          block w-full p-2.5
          ${error ? 'border-danger' : ''}
          ${className || ''}
        `}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value} disabled={option.disabled}>
            {option.label}
          </option>
        ))}
        {groups?.map((group) => (
          <optgroup key={group.label} label={group.label}>
            {group.options.map((option) => (
              <option key={option.value} value={option.value} disabled={option.disabled}>
                {option.label}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
      {error && <p className="text-xs text-danger mt-1">{error}</p>}
    </div>
  )
}
