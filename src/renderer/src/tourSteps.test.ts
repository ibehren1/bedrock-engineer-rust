import i18next from 'i18next'
import en from './i18n/locales/en'
import ja from './i18n/locales/ja'
import { TOUR_STOPS, buildTourSteps, tourSelector } from './tourSteps'

const translator = async (lng: 'en' | 'ja') => {
  const i18n = i18next.createInstance()
  await i18n.init({
    resources: { en: { translation: en }, ja: { translation: ja } },
    lng,
    interpolation: { escapeValue: false, defaultVariables: { appName: 'Bedrock Engineer' } }
  })
  return i18n.t
}

describe('app tour', () => {
  it('has one stop per sidebar item, Settings first and Help last', () => {
    const hrefs = TOUR_STOPS.map((s) => s.href)
    expect(hrefs[0]).toBe('/setting')
    expect(hrefs[hrefs.length - 1]).toBe('/help')
    expect(new Set(hrefs).size).toBe(hrefs.length)
    expect(hrefs.length).toBeGreaterThan(1)
  })

  it('points each step at its sidebar link', async () => {
    const steps = buildTourSteps(await translator('en'), [], '⌘K')
    expect(steps.map((s) => s.selector)).toEqual(TOUR_STOPS.map((s) => tourSelector(s.href)))
  })

  it('skips items hidden from the sidebar', async () => {
    const steps = buildTourSteps(await translator('en'), ['/memory', '/accounting'], '⌘K')
    expect(steps).toHaveLength(TOUR_STOPS.length - 2)
    expect(steps.map((s) => s.selector)).not.toContain(tourSelector('/memory'))
    expect(steps.map((s) => s.selector)).not.toContain(tourSelector('/accounting'))
  })

  it.each(['en', 'ja'] as const)('has text for every stop in %s', async (lng) => {
    const t = await translator(lng)
    for (const { content } of TOUR_STOPS) {
      const text = t(content, { shortcut: 'Ctrl+K' })
      // i18next returns the key itself when a translation is missing.
      expect(text).not.toBe(content)
      expect(text).not.toMatch(/\{\{/)
    }
    expect(t('tour.help', { shortcut: 'Ctrl+K' })).toContain('Ctrl+K')
  })
})
