import { useTranslation } from 'react-i18next'
import { DomainInput } from './DomainInput'
import { DomainTag } from './DomainTag'

interface DomainListSectionProps {
  label: string
  domains: string[]
  newDomain: string
  error: boolean
  variant: 'include' | 'exclude'
  onDomainChange: (value: string) => void
  onAddDomain: () => void
  onRemoveDomain: (domain: string) => void
  onClearAll: () => void
}

export const DomainListSection = ({
  label,
  domains,
  newDomain,
  error,
  variant,
  onDomainChange,
  onAddDomain,
  onRemoveDomain,
  onClearAll
}: DomainListSectionProps) => {
  const { t } = useTranslation()

  return (
    <div className="mb-4">
      <div className="flex items-center justify-between mb-2">
        <label className="text-xs text-ink-muted">{label}</label>
        {domains.length > 0 && (
          <button
            onClick={onClearAll}
            className="text-xs text-ink-muted hover:text-ink hover:underline cursor-pointer"
          >
            {t('Clear All', 'Clear All')}
          </button>
        )}
      </div>
      <DomainInput value={newDomain} error={error} onChange={onDomainChange} onAdd={onAddDomain} />
      <div className="flex flex-wrap gap-1 mt-2">
        {domains.map((domain) => (
          <DomainTag key={domain} domain={domain} onRemove={onRemoveDomain} variant={variant} />
        ))}
      </div>
    </div>
  )
}
