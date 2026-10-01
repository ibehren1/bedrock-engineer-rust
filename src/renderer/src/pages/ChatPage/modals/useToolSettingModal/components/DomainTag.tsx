import { FaTimes } from 'react-icons/fa'

interface DomainTagProps {
  domain: string
  onRemove: (domain: string) => void
  variant?: 'include' | 'exclude'
}

export const DomainTag = ({ domain, onRemove, variant = 'include' }: DomainTagProps) => {
  const bgColor = variant === 'include' ? 'bg-accent-tint text-accent' : 'bg-raised text-ink'

  const hoverColor = variant === 'include' ? 'hover:text-accent' : 'hover:text-ink-muted'

  return (
    <span className={`inline-flex items-center gap-1 px-2 py-1 text-xs rounded-control ${bgColor}`}>
      {domain}
      <button onClick={() => onRemove(domain)} className={`cursor-pointer ${hoverColor}`}>
        <FaTimes className="w-3 h-3" />
      </button>
    </span>
  )
}
