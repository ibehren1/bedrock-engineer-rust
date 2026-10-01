import { useTranslation } from 'react-i18next'
import { FaPlus } from 'react-icons/fa'

interface DomainInputProps {
  value: string
  error: boolean
  onChange: (value: string) => void
  onAdd: () => void
  variant?: 'include' | 'exclude'
  placeholder?: string
}

export const DomainInput = ({
  value,
  error,
  onChange,
  onAdd,
  placeholder = 'example.com'
}: DomainInputProps) => {
  const { t } = useTranslation()

  const handleKeyPress = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !error) {
      onAdd()
    }
  }

  return (
    <div>
      <div className="flex gap-2 mb-2">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className="flex-1 px-3 py-2 text-sm border rounded-control bg-surface border-subtle text-ink"
          onKeyPress={handleKeyPress}
        />
        <button
          onClick={onAdd}
          disabled={error || !value}
          className={`px-3 py-2 text-sm text-accent-fg rounded-control disabled:bg-sunken disabled:cursor-not-allowed cursor-pointer bg-accent hover:bg-accent-strong`}
        >
          <FaPlus className="w-3 h-3" />
        </button>
      </div>
      {error && (
        <p className="text-xs text-danger mt-1">
          {t(
            'Please enter a valid domain (e.g., example.com)',
            'Please enter a valid domain (e.g., example.com)'
          )}
        </p>
      )}
    </div>
  )
}
