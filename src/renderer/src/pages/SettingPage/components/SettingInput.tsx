import React, { useState } from 'react'
import { FaEye, FaEyeSlash } from 'react-icons/fa'

interface SettingInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label: string
  description?: string
  error?: string
}

export const SettingInput: React.FC<SettingInputProps> = ({
  label,
  description,
  error,
  className,
  ...inputProps
}) => {
  const [showPassword, setShowPassword] = useState(false)
  const isPassword = inputProps.type === 'password'

  return (
    <div className="space-y-1">
      <label className="block text-sm font-medium text-ink">{label}</label>
      {description && <p className="text-xs text-ink-muted">{description}</p>}
      <div className="relative">
        <input
          {...inputProps}
          value={inputProps.value ?? ''}
          type={!showPassword ? inputProps.type : 'text'}
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
        />
        {isPassword && (
          <button
            type="button"
            className="absolute inset-y-0 right-0 flex items-center px-3 text-ink-faint hover:text-ink-muted"
            onClick={() => setShowPassword(!showPassword)}
          >
            {showPassword ? <FaEyeSlash className="w-4 h-4" /> : <FaEye className="w-4 h-4" />}
          </button>
        )}
      </div>
      {error && <p className="text-xs text-danger mt-1">{error}</p>}
    </div>
  )
}
