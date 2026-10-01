import React from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from 'react-router-dom'
import { SettingsSidebar } from './components/SettingsSidebar'
import { AwsTab, ChatTab, GeneralTab, ModelsTab, WorkspaceTab } from './components/tabs'
import { resolveSettingTab, SettingTabId } from './settingTabs'

const TAB_PANELS: Record<SettingTabId, React.FC> = {
  general: GeneralTab,
  aws: AwsTab,
  models: ModelsTab,
  chat: ChatTab,
  workspace: WorkspaceTab
}

export const SettingPage: React.FC = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { tab } = useParams<{ tab?: string }>()

  // 不明なタブ（プレーンな /setting も含む）は最初のタブにフォールバックする
  const activeTab = resolveSettingTab(tab)
  const ActivePanel = TAB_PANELS[activeTab]

  const handleTabChange = (nextTab: SettingTabId) => {
    // replace: タブの切り替えで履歴を積み上げない
    navigate(`/setting/${nextTab}`, { replace: true })
  }

  return (
    <div className="flex flex-col h-full text-ink">
      <h1 className="text-title text-ink px-5 pt-3 pb-2">{t('Settings')}</h1>

      <div className="flex flex-1 min-h-0 overflow-hidden">
        <div
          className="lg:w-64 w-16 border-r border-subtle flex-shrink-0
            overflow-y-auto transition-all duration-300"
        >
          <SettingsSidebar activeTab={activeTab} onTabChange={handleTabChange} />
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4">
          <div className="min-w-[320px] max-w-[1024px]">
            <ActivePanel />
          </div>
        </div>
      </div>
    </div>
  )
}

export default SettingPage
