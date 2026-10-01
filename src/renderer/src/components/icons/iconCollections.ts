/* eslint-disable no-restricted-syntax -- the dynamic imports below are the point:
   each collection is megabytes of icon data and must stay out of the main bundle. */

/**
 * Agent icons come from two places:
 *
 * - the curated `AGENT_ICONS` list (values like `robot`), statically imported
 *   from react-icons and therefore always available, and
 * - any icon in one of the Iconify collections below, stored as the Iconify id
 *   `<collection>:<name>` (e.g. `tabler:rocket`).
 *
 * The collections hold ~38,000 icons between them, so each one is a lazily
 * imported chunk of pure SVG data. Loaded collections are cached and subscribers
 * are notified, so icons already on screen fill in as soon as their data lands.
 *
 * Note: icon data is deliberately *data* rather than react-icons modules. Most
 * react-icons libraries are also statically imported elsewhere in the app, and
 * dynamically importing the same module forces its entire icon set into the main
 * bundle instead of a lazy chunk.
 */

export type IconCollectionId =
  | 'tabler'
  | 'lucide'
  | 'ph'
  | 'mdi'
  | 'heroicons'
  | 'bi'
  | 'fa6-solid'
  | 'fa6-brands'
  | 'simple-icons'
  | 'game-icons'

/** The parts of the Iconify JSON format this renderer needs. */
type RawIconCollection = {
  icons: Record<string, RawIcon>
  width?: number
  height?: number
  left?: number
  top?: number
}

type RawIcon = {
  body: string
  width?: number
  height?: number
  left?: number
  top?: number
}

export type IconCollection = {
  id: IconCollectionId
  /** Shown in the picker's collection selector. */
  label: string
  load: () => Promise<{ default: unknown }>
}

export const ICON_COLLECTIONS: readonly IconCollection[] = [
  {
    id: 'tabler',
    label: 'Tabler',
    load: () => import('@iconify-json/tabler/icons.json')
  },
  {
    id: 'lucide',
    label: 'Lucide',
    load: () => import('@iconify-json/lucide/icons.json')
  },
  {
    id: 'ph',
    label: 'Phosphor',
    load: () => import('@iconify-json/ph/icons.json')
  },
  {
    id: 'mdi',
    label: 'Material',
    load: () => import('@iconify-json/mdi/icons.json')
  },
  {
    id: 'heroicons',
    label: 'Heroicons',
    load: () => import('@iconify-json/heroicons/icons.json')
  },
  {
    id: 'bi',
    label: 'Bootstrap',
    load: () => import('@iconify-json/bi/icons.json')
  },
  {
    id: 'fa6-solid',
    label: 'Font Awesome',
    load: () => import('@iconify-json/fa6-solid/icons.json')
  },
  {
    id: 'fa6-brands',
    label: 'Font Awesome Brands',
    load: () => import('@iconify-json/fa6-brands/icons.json')
  },
  {
    id: 'simple-icons',
    label: 'Brands',
    load: () => import('@iconify-json/simple-icons/icons.json')
  },
  {
    id: 'game-icons',
    label: 'Game Icons',
    load: () => import('@iconify-json/game-icons/icons.json')
  }
]

const COLLECTIONS_BY_ID = new Map(ICON_COLLECTIONS.map((collection) => [collection.id, collection]))

export const getIconCollection = (id: string): IconCollection | undefined =>
  COLLECTIONS_BY_ID.get(id as IconCollectionId)

/** One icon, ready to drop into an `<svg>`. */
export type IconData = {
  body: string
  viewBox: string
}

/** Iconify's default canvas when a collection or icon doesn't state one. */
const DEFAULT_ICON_SIZE = 16

type LoadedCollection = Record<string, IconData>

/** A reference to one icon in one collection, e.g. `{ collection: 'tabler', name: 'rocket' }`. */
export type IconRef = {
  collection: IconCollectionId
  name: string
}

export const formatIconValue = (collection: IconCollectionId, name: string): string =>
  `${collection}:${name}`

/**
 * Splits a stored icon value into a collection reference. Returns `undefined` for
 * curated values (`robot`) and for collections we don't ship.
 */
export const parseIconValue = (value?: string): IconRef | undefined => {
  if (!value) return undefined
  const separator = value.indexOf(':')
  if (separator <= 0) return undefined

  const collection = value.slice(0, separator)
  const name = value.slice(separator + 1)
  if (!name || !COLLECTIONS_BY_ID.has(collection as IconCollectionId)) return undefined

  return { collection: collection as IconCollectionId, name }
}

/** `rocket-launch` becomes `Rocket launch`. */
export const humanizeIconName = (name: string): string => {
  const words = name.replace(/[-_]+/g, ' ').trim()
  if (words === '') return name
  return words.charAt(0).toUpperCase() + words.slice(1)
}

const toLoadedCollection = (raw: RawIconCollection): LoadedCollection => {
  const collection: LoadedCollection = {}
  for (const [name, icon] of Object.entries(raw.icons ?? {})) {
    if (typeof icon?.body !== 'string') continue
    const left = icon.left ?? raw.left ?? 0
    const top = icon.top ?? raw.top ?? 0
    const width = icon.width ?? raw.width ?? DEFAULT_ICON_SIZE
    const height = icon.height ?? raw.height ?? DEFAULT_ICON_SIZE
    collection[name] = { body: icon.body, viewBox: `${left} ${top} ${width} ${height}` }
  }
  return collection
}

const loadedCollections = new Map<IconCollectionId, LoadedCollection>()
const inFlight = new Map<IconCollectionId, Promise<LoadedCollection>>()
const listeners = new Set<() => void>()

/** Notified whenever a collection finishes loading, so mounted icons can re-render. */
export const subscribeToIconCollections = (listener: () => void): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

const notify = () => listeners.forEach((listener) => listener())

export const isIconCollectionLoaded = (id: IconCollectionId): boolean => loadedCollections.has(id)

export const loadIconCollection = (id: IconCollectionId): Promise<LoadedCollection> => {
  const loaded = loadedCollections.get(id)
  if (loaded) return Promise.resolve(loaded)

  const pending = inFlight.get(id)
  if (pending) return pending

  const collection = COLLECTIONS_BY_ID.get(id)
  if (!collection) return Promise.reject(new Error(`Unknown icon collection: ${id}`))

  const promise = collection
    .load()
    .then((module) => {
      const icons = toLoadedCollection(module.default as RawIconCollection)
      loadedCollections.set(id, icons)
      inFlight.delete(id)
      notify()
      return icons
    })
    .catch((error) => {
      inFlight.delete(id)
      throw error
    })

  inFlight.set(id, promise)
  return promise
}

export const loadAllIconCollections = async (): Promise<void> => {
  await Promise.all(
    ICON_COLLECTIONS.map((collection) =>
      loadIconCollection(collection.id).catch(() => {
        // One collection failing to load shouldn't take the picker down; its
        // icons just stay unavailable.
      })
    )
  )
}

/** Resolves a `collection:name` value to its SVG data, if that collection is loaded. */
export const getIconData = (value?: string): IconData | undefined => {
  const ref = parseIconValue(value)
  if (!ref) return undefined
  return loadedCollections.get(ref.collection)?.[ref.name]
}

export type IconSearchResult = {
  /** Stored value, e.g. `tabler:rocket`. */
  value: string
  name: string
  collection: IconCollectionId
  collectionLabel: string
  label: string
  data: IconData
}

/** Every icon of one loaded collection, in collection order. */
export const listCollectionIcons = (id: IconCollectionId): IconSearchResult[] => {
  const collection = COLLECTIONS_BY_ID.get(id)
  const icons = loadedCollections.get(id)
  if (!collection || !icons) return []

  return Object.entries(icons).map(([name, data]) => ({
    value: formatIconValue(collection.id, name),
    name,
    collection: collection.id,
    collectionLabel: collection.label,
    label: humanizeIconName(name),
    data
  }))
}

/**
 * Matches a query against icon names across the given collections (loaded ones
 * only). Every word in the query must appear in the name, so `heart rate`
 * matches `heart-rate-monitor`.
 */
export const searchIcons = (
  query: string,
  collectionIds: readonly IconCollectionId[],
  limit: number
): { results: IconSearchResult[]; total: number } => {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  const results: IconSearchResult[] = []
  let total = 0

  for (const id of collectionIds) {
    for (const icon of listCollectionIcons(id)) {
      if (!words.every((word) => icon.name.includes(word))) continue
      total += 1
      if (results.length < limit) results.push(icon)
    }
  }

  return { results, total }
}
