import { DEFAULT_SETTING_TAB, resolveSettingTab, SETTING_TABS } from './settingTabs'

describe('resolveSettingTab', () => {
  it('accepts every declared tab id', () => {
    SETTING_TABS.forEach((tab) => {
      expect(resolveSettingTab(tab.id)).toBe(tab.id)
    })
  })

  it('falls back to the default tab for a missing segment', () => {
    expect(resolveSettingTab(undefined)).toBe(DEFAULT_SETTING_TAB)
    expect(resolveSettingTab('')).toBe(DEFAULT_SETTING_TAB)
  })

  it('falls back to the default tab for an unknown segment', () => {
    expect(resolveSettingTab('bogus')).toBe(DEFAULT_SETTING_TAB)
    expect(resolveSettingTab('General')).toBe(DEFAULT_SETTING_TAB)
  })
})

describe('SETTING_TABS', () => {
  it('has unique ids', () => {
    const ids = SETTING_TABS.map((tab) => tab.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('starts with the default tab', () => {
    expect(SETTING_TABS[0].id).toBe(DEFAULT_SETTING_TAB)
  })
})
