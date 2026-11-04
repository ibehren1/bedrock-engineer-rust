import i18next from 'i18next'
import en from './en'
import ja from './ja'

/** Every key path in a translation tree, e.g. `chat.attachments.menu.summary_one`. */
function keyPaths(tree: unknown, prefix = ''): string[] {
  if (tree === null || typeof tree !== 'object') return [prefix]
  return Object.entries(tree as Record<string, unknown>).flatMap(([k, v]) =>
    keyPaths(v, prefix ? `${prefix}.${k}` : k)
  )
}

describe('translation plurals', () => {
  it('uses the i18next v4 plural suffixes (_one / _other), not the old _plural', () => {
    // i18next ignores `_plural` since v21, so such a key would never be shown.
    const old = [...keyPaths(en), ...keyPaths(ja)].filter((k) => k.endsWith('_plural'))
    expect(old).toEqual([])
  })

  it('picks the singular and plural English forms by count', async () => {
    const i18n = i18next.createInstance()
    await i18n.init({
      resources: { en: { translation: en }, ja: { translation: ja } },
      lng: 'en',
      interpolation: { escapeValue: false }
    })
    expect(i18n.t('invokeAgentResult.toolCalls', { count: 1 })).toBe('1 tool call')
    expect(i18n.t('invokeAgentResult.toolCalls', { count: 3 })).toBe('3 tool calls')
    expect(i18n.t('attachments.menu.summary', { count: 2, size: '4 KB' })).toBe('2 files · 4 KB')
    await i18n.changeLanguage('ja')
    expect(i18n.t('invokeAgentResult.toolCalls', { count: 3 })).toBe('ツール実行 3 回')
    expect(i18n.t('attachments.menu.summary', { count: 2, size: '4 KB' })).toBe('2 件 · 4 KB')
  })
})
