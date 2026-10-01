import React, { useEffect, useMemo, useReducer, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AGENT_ICONS, AgentIconOption } from './AgentIcons'
import { IconifyIcon } from './AgentIconView'
import {
  ICON_COLLECTIONS,
  IconCollectionId,
  IconSearchResult,
  isIconCollectionLoaded,
  listCollectionIcons,
  loadAllIconCollections,
  loadIconCollection,
  searchIcons,
  subscribeToIconCollections
} from './iconCollections'

/** `curated` keeps the hand-picked, categorised list; `all` searches every collection. */
type PickerScope = 'curated' | 'all' | IconCollectionId

/**
 * Rendering thousands of buttons at once locks the UI, so results are capped and
 * the user is told to narrow the search.
 */
const MAX_RESULTS = 300

const CURATED_CATEGORY_ORDER: readonly AgentIconOption['category'][] = [
  'general',
  'lifestyle',
  'health',
  'education',
  'travel',
  'food',
  'shopping',
  'development',
  'cloud',
  'devops',
  'security',
  'monitoring'
]

type IconPickerProps = {
  selectedIcon?: string
  onSelect: (value: string) => void
}

export const IconPicker: React.FC<IconPickerProps> = ({ selectedIcon, onSelect }) => {
  const { t } = useTranslation()
  const [scope, setScope] = useState<PickerScope>('curated')
  const [searchQuery, setSearchQuery] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  // Bumped whenever a collection finishes loading, so results recompute against
  // the newly available data.
  const [collectionVersion, bumpCollectionVersion] = useReducer((count: number) => count + 1, 0)

  useEffect(() => subscribeToIconCollections(bumpCollectionVersion), [])

  // Load whatever the current scope needs. Collections are cached, so returning
  // to one already visited is instant.
  useEffect(() => {
    if (scope === 'curated') return

    let cancelled = false
    const load =
      scope === 'all' ? loadAllIconCollections() : loadIconCollection(scope).then(() => undefined)

    if (scope === 'all' || !isIconCollectionLoaded(scope)) setIsLoading(true)
    load
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setIsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [scope])

  const query = searchQuery.trim()

  const curatedGroups = useMemo(() => {
    const lowered = query.toLowerCase()
    return CURATED_CATEGORY_ORDER.map((category) => ({
      category,
      icons: AGENT_ICONS.filter(
        (option) =>
          option.category === category &&
          (lowered === '' || option.label.toLowerCase().includes(lowered))
      )
    })).filter((group) => group.icons.length > 0)
  }, [query])

  const searchResults = useMemo<{ results: IconSearchResult[]; total: number }>(() => {
    if (scope === 'curated') return { results: [], total: 0 }

    const collectionIds: IconCollectionId[] =
      scope === 'all' ? ICON_COLLECTIONS.map((collection) => collection.id) : [scope]

    if (query === '') {
      // No query: page through a single collection from the top. Doing that
      // across every collection at once is neither useful nor cheap.
      if (scope === 'all') return { results: [], total: 0 }
      const all = listCollectionIcons(scope)
      return { results: all.slice(0, MAX_RESULTS), total: all.length }
    }

    return searchIcons(query, collectionIds, MAX_RESULTS)
  }, [scope, query, collectionVersion])

  return (
    <div className="flex flex-col gap-2">
      <div className="flex gap-2">
        <select
          value={scope}
          onChange={(event) => setScope(event.target.value as PickerScope)}
          className="w-[45%] rounded-control border border-strong bg-surface-2
            text-sm py-2 pl-2 pr-6 focus:outline-none focus:ring-2 focus:ring-accent
            text-ink"
        >
          <option value="curated">{t('iconLibraryCurated')}</option>
          <option value="all">{t('iconLibraryAll')}</option>
          {ICON_COLLECTIONS.map((collection) => (
            <option key={collection.id} value={collection.id}>
              {collection.label}
            </option>
          ))}
        </select>
        <input
          type="text"
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          placeholder={t('searchIcons')}
          className="flex-1 min-w-0 px-3 py-2 border border-strong rounded-control
            bg-surface-2 text-sm focus:outline-none focus:ring-2
            focus:ring-accent text-ink"
        />
      </div>

      <div className="max-h-[420px] overflow-y-auto p-1">
        {scope === 'curated' ? (
          curatedGroups.length === 0 ? (
            <PickerMessage>{t('noIconsFound')}</PickerMessage>
          ) : (
            curatedGroups.map((group) => (
              <div key={group.category} className="mb-4 last:mb-0">
                <h3 className="text-sm font-medium text-ink mb-2 px-1">
                  {t(`iconCategory.${group.category}`)}
                </h3>
                <IconGrid>
                  {group.icons.map((option) => (
                    <IconButton
                      key={option.value}
                      title={option.label}
                      isSelected={selectedIcon === option.value}
                      onClick={() => onSelect(option.value)}
                    >
                      {React.cloneElement(option.icon as React.ReactElement, {
                        className: 'w-6 h-6'
                      })}
                    </IconButton>
                  ))}
                </IconGrid>
              </div>
            ))
          )
        ) : isLoading ? (
          <PickerMessage>{t('iconsLoading')}</PickerMessage>
        ) : scope === 'all' && query === '' ? (
          <PickerMessage>{t('searchIconsHint')}</PickerMessage>
        ) : searchResults.results.length === 0 ? (
          <PickerMessage>{t('noIconsFound')}</PickerMessage>
        ) : (
          <>
            <IconGrid>
              {searchResults.results.map((icon) => (
                <IconButton
                  key={icon.value}
                  title={`${icon.label} · ${icon.collectionLabel}`}
                  isSelected={selectedIcon === icon.value}
                  onClick={() => onSelect(icon.value)}
                >
                  <IconifyIcon data={icon.data} className="w-6 h-6" />
                </IconButton>
              ))}
            </IconGrid>
            {searchResults.total > searchResults.results.length && (
              <p className="mt-2 px-1 text-xs text-ink-muted">
                {t('iconResultsTruncated', {
                  shown: searchResults.results.length,
                  total: searchResults.total
                })}
              </p>
            )}
          </>
        )}
      </div>
    </div>
  )
}

const PickerMessage: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <p className="p-2 text-sm text-ink-muted">{children}</p>
)

const IconGrid: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className="grid grid-cols-8 gap-1">{children}</div>
)

type IconButtonProps = {
  title: string
  isSelected: boolean
  onClick: () => void
  children: React.ReactNode
}

const IconButton: React.FC<IconButtonProps> = ({ title, isSelected, onClick, children }) => (
  <button
    type="button"
    onClick={onClick}
    title={title}
    className={`flex items-center justify-center p-1.5 rounded-container hover:bg-raised
      hover:bg-raised ${isSelected ? 'bg-accent-tint text-accent' : 'text-ink-muted'}`}
  >
    <div className="w-6 h-6 flex items-center justify-center">{children}</div>
  </button>
)
