import { IconType } from 'react-icons'
import { FaAws } from 'react-icons/fa'
import { FiCpu, FiFolder, FiMessageSquare, FiSliders } from 'react-icons/fi'

export type SettingTabId = 'general' | 'aws' | 'models' | 'chat' | 'workspace'

export interface SettingTab {
  id: SettingTabId
  /** i18n key for the rail label */
  labelKey: string
  icon: IconType
  /** Overrides the default icon sizing, for marks that aren't square. */
  iconClassName?: string
}

/** Rail order, top to bottom. */
export const SETTING_TABS: SettingTab[] = [
  { id: 'general', labelKey: 'settingTabs.general', icon: FiSliders },
  // The AWS mark is a wide wordmark, so it needs a wider box than the square
  // line icons to read at the same optical size.
  { id: 'aws', labelKey: 'settingTabs.aws', icon: FaAws, iconClassName: 'w-6 h-5' },
  { id: 'models', labelKey: 'settingTabs.models', icon: FiCpu },
  { id: 'chat', labelKey: 'settingTabs.chat', icon: FiMessageSquare },
  { id: 'workspace', labelKey: 'settingTabs.workspace', icon: FiFolder }
]

export const DEFAULT_SETTING_TAB: SettingTabId = 'general'

/**
 * Map the `:tab` URL segment to a tab. Anything unknown — including a missing
 * segment, which is what plain `/setting` gives — falls back to the first tab.
 */
export const resolveSettingTab = (raw?: string): SettingTabId => {
  const match = SETTING_TABS.find((tab) => tab.id === raw)
  return match ? match.id : DEFAULT_SETTING_TAB
}
