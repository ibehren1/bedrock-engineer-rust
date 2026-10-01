import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiSettings, FiServer, FiTool } from 'react-icons/fi'
import { formEventUtils } from '../utils/formEventUtils'

// タブ識別子の型定義
type AgentFormTabId = 'basic' | 'mcp-servers' | 'tools'

/**
 * サイドバーナビゲーションコンポーネント
 * AgentFormTabs をサイドバー形式に置き換える
 */
export const AgentFormSidebar: React.FC<{
  activeTab: AgentFormTabId
  onTabChange: (tabId: AgentFormTabId) => void
  onToolsTabClick: (mcpServers?: any) => Promise<void>
}> = ({ activeTab, onTabChange, onToolsTabClick }) => {
  const { t } = useTranslation()

  // タブ切り替えハンドラー
  const handleToolsTabClick = async (e: React.MouseEvent) => {
    formEventUtils.preventPropagation(e)
    onTabChange('tools' as AgentFormTabId)
    await onToolsTabClick()
  }

  // タブ定義
  const tabs = [
    {
      id: 'basic' as AgentFormTabId,
      label: t('Basic Settings'),
      icon: <FiSettings className="w-4 h-4" />,
      onClick: (e: React.MouseEvent) => {
        formEventUtils.preventPropagation(e)
        onTabChange('basic')
      }
    },
    {
      id: 'mcp-servers' as AgentFormTabId,
      label: t('MCP Servers'),
      icon: <FiServer className="w-4 h-4" />,
      onClick: (e: React.MouseEvent) => {
        formEventUtils.preventPropagation(e)
        onTabChange('mcp-servers')
      }
    },
    {
      id: 'tools' as AgentFormTabId,
      label: t('Tools'),
      icon: <FiTool className="w-4 h-4" />,
      onClick: handleToolsTabClick
    }
  ]

  return (
    <div
      className="py-4 flex flex-col h-full bg-surface-2"
      onClick={formEventUtils.preventPropagation}
    >
      <ul className="space-y-1 px-2">
        {tabs.map((tab) => (
          <li key={tab.id}>
            <button
              type="button"
              className={`relative flex items-center w-full px-3 py-2.5 rounded-container text-left transition-all duration-200 ease-in-out ${
                activeTab === tab.id
                  ? 'bg-accent-tint text-accent'
                  : 'text-ink hover:bg-raised hover:text-ink'
              }`}
              onClick={tab.onClick}
              title={tab.label}
            >
              <span className="flex items-center justify-center">{tab.icon}</span>
              <span className="text-sm font-medium ml-3 lg:block hidden">{tab.label}</span>
            </button>
          </li>
        ))}
      </ul>
      {/* 残りのスペースを埋めるための空のdiv */}
      <div className="flex-grow"></div>
    </div>
  )
}
