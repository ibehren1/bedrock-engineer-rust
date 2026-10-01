import { AgentIconSchema } from '@/types/agent-chat.schema'
import {
  formatIconValue,
  getIconCollection,
  getIconData,
  humanizeIconName,
  ICON_COLLECTIONS,
  isIconCollectionLoaded,
  loadIconCollection,
  parseIconValue,
  searchIcons,
  subscribeToIconCollections
} from './iconCollections'

describe('parseIconValue', () => {
  it('parses an Iconify id', () => {
    expect(parseIconValue('tabler:rocket')).toEqual({ collection: 'tabler', name: 'rocket' })
  })

  it('parses collection and icon names containing dashes', () => {
    expect(parseIconValue('simple-icons:amazonaws')).toEqual({
      collection: 'simple-icons',
      name: 'amazonaws'
    })
    expect(parseIconValue('tabler:brand-aws')).toEqual({
      collection: 'tabler',
      name: 'brand-aws'
    })
  })

  it('rejects curated values, which carry no collection prefix', () => {
    expect(parseIconValue('robot')).toBeUndefined()
    expect(parseIconValue('calendar-stats')).toBeUndefined()
  })

  it('rejects unknown collections and malformed values', () => {
    expect(parseIconValue('nope:rocket')).toBeUndefined()
    expect(parseIconValue(':rocket')).toBeUndefined()
    expect(parseIconValue('tabler:')).toBeUndefined()
    expect(parseIconValue(undefined)).toBeUndefined()
  })

  it('round-trips with formatIconValue', () => {
    const value = formatIconValue('ph', 'rocket-launch')
    expect(value).toBe('ph:rocket-launch')
    expect(parseIconValue(value)).toEqual({ collection: 'ph', name: 'rocket-launch' })
  })
})

describe('humanizeIconName', () => {
  it('turns an icon name into a label', () => {
    expect(humanizeIconName('rocket-launch')).toBe('Rocket launch')
    expect(humanizeIconName('heart_rate_monitor')).toBe('Heart rate monitor')
    expect(humanizeIconName('rocket')).toBe('Rocket')
  })
})

describe('ICON_COLLECTIONS', () => {
  it('has unique ids resolvable by getIconCollection', () => {
    const ids = ICON_COLLECTIONS.map((collection) => collection.id)
    expect(new Set(ids).size).toBe(ids.length)
    ids.forEach((id) => expect(getIconCollection(id)?.id).toBe(id))
    expect(getIconCollection('nope')).toBeUndefined()
  })

  it('produces icon values the agent schema accepts', () => {
    ICON_COLLECTIONS.forEach((collection) => {
      expect(AgentIconSchema.parse(formatIconValue(collection.id, 'rocket-launch-2'))).toBe(
        `${collection.id}:rocket-launch-2`
      )
    })
    expect(AgentIconSchema.parse('robot')).toBe('robot')
    expect(() => AgentIconSchema.parse('Not An Icon')).toThrow()
  })
})

describe('loadIconCollection', () => {
  it('resolves icon data and notifies subscribers', async () => {
    const listener = jest.fn()
    const unsubscribe = subscribeToIconCollections(listener)

    expect(getIconData('tabler:rocket')).toBeUndefined()
    await loadIconCollection('tabler')
    unsubscribe()

    expect(listener).toHaveBeenCalled()
    expect(isIconCollectionLoaded('tabler')).toBe(true)
    expect(getIconData('tabler:rocket')).toEqual({
      body: expect.stringContaining('<path'),
      viewBox: '0 0 24 24'
    })
  })

  it('falls back to a 16x16 canvas for collections that state no size', async () => {
    await loadIconCollection('bi')
    expect(getIconData('bi:rocket')?.viewBox).toBe('0 0 16 16')
  })

  it('caches the module, so a second load is the same data', async () => {
    const first = await loadIconCollection('tabler')
    const second = await loadIconCollection('tabler')
    expect(second).toBe(first)
  })

  it('searches only loaded collections and reports the untruncated total', async () => {
    await loadIconCollection('tabler')

    const { results, total } = searchIcons('brand', ['tabler', 'ph'], 3)
    expect(results.length).toBe(3)
    expect(total).toBeGreaterThan(3)
    // 'ph' was never loaded, so nothing from it can appear.
    expect(results.every((icon) => icon.collection === 'tabler')).toBe(true)
    expect(results[0]).toMatchObject({ collectionLabel: 'Tabler', name: expect.any(String) })
  })
})
