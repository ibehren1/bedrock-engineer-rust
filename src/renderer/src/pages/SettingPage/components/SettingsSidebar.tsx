import React from 'react'
import { useTranslation } from 'react-i18next'
import { SETTING_TABS, SettingTabId } from '../settingTabs'

interface SettingsSidebarProps {
  activeTab: SettingTabId
  onTabChange: (tab: SettingTabId) => void
}

/**
 * Left rail for the settings tabs. Mirrors the agent editor's sidebar
 * (AgentFormSidebar) so the two in-page rails look and behave the same.
 */
export const SettingsSidebar: React.FC<SettingsSidebarProps> = ({ activeTab, onTabChange }) => {
  const { t } = useTranslation()

  return (
    <nav aria-label={t('Settings')} className="py-2">
      <ul className="space-y-1 px-2">
        {SETTING_TABS.map((tab) => {
          const isActive = activeTab === tab.id
          return (
            <li key={tab.id}>
              <button
                type="button"
                onClick={() => onTabChange(tab.id)}
                aria-current={isActive ? 'page' : undefined}
                title={t(tab.labelKey)}
                className={`relative flex items-center w-full px-3 py-2.5 rounded-container text-left transition-all duration-200 ease-in-out ${
                  isActive
                    ? 'bg-accent-tint text-accent'
                    : 'text-ink hover:bg-raised hover:text-ink'
                }`}
              >
                <span className="flex items-center justify-center w-6">
                  <tab.icon className={tab.iconClassName ?? 'w-4 h-4'} />
                </span>
                <span className="text-sm font-medium ml-3 lg:block hidden">{t(tab.labelKey)}</span>
              </button>
            </li>
          )
        })}
      </ul>
    </nav>
  )
}
