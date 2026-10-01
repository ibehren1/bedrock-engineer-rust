import { BsGlobeAmericas } from 'react-icons/bs'

type DeepSearchButtonProps = {
  enableDeepSearch: boolean
  handleToggleDeepSearch: () => void
}

/**
 * A toggle, so the enabled state reads as "pressed" rather than decorated.
 *
 * This was the last gradient button in the app — a blue-to-cyan fill wrapping an
 * inner panel, which both looked like a consumer app and could not follow an
 * appearance. It is now a tinted accent fill with an accent border, which works
 * in all five appearances including charcoal's amber.
 */
export const DeepSearchButton: React.FC<DeepSearchButtonProps> = (props) => {
  const { enableDeepSearch, handleToggleDeepSearch } = props
  return (
    <button
      type="button"
      onClick={handleToggleDeepSearch}
      aria-pressed={enableDeepSearch}
      className={`flex items-center gap-1.5 px-2 py-1 text-xs rounded-control transition-colors
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent
        ${
          enableDeepSearch
            ? 'bg-accent-tint border border-accent text-accent'
            : 'border border-subtle text-ink-muted hover:bg-raised hover:text-ink'
        }`}
    >
      <BsGlobeAmericas className="w-3.5 h-3.5" />
      Search
    </button>
  )
}
