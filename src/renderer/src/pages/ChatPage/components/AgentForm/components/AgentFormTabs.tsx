import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiSettings, FiServer, FiTool } from 'react-icons/fi'
import { formEventUtils } from '../utils/formEventUtils'

// タブ識別子の型定義
type AgentFormTabId = 'basic' | 'mcp-servers' | 'tools'

/**
 * タブナビゲーションコンポーネント
 */
export const AgentFormTabs: React.FC<{
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

  return (
    <div className="border-b border-subtle mb-3">
      <ul className="flex flex-wrap -mb-px" onClick={formEventUtils.preventPropagation}>
        <li className="mr-2">
          <button
            type="button"
            className={`inline-flex items-center gap-1.5 p-2.5 border-b-2 rounded-t-container ${
              activeTab === 'basic'
                ? 'text-accent border-accent'
                : 'text-ink-muted border-transparent hover:text-ink-muted hover:border-strong'
            }`}
            onClick={formEventUtils.createSafeHandler(() => onTabChange('basic' as AgentFormTabId))}
          >
            <FiSettings className="w-4 h-4" />
            {t('Basic Settings')}
          </button>
        </li>
        <li className="mr-2">
          <button
            type="button"
            className={`inline-flex items-center gap-1.5 p-2.5 border-b-2 rounded-t-container ${
              activeTab === 'mcp-servers'
                ? 'text-accent border-accent'
                : 'text-ink-muted border-transparent hover:text-ink-muted hover:border-strong'
            }`}
            onClick={formEventUtils.createSafeHandler(() =>
              onTabChange('mcp-servers' as AgentFormTabId)
            )}
          >
            <FiServer className="w-4 h-4" />
            {t('MCP Servers')}
          </button>
        </li>
        <li>
          <button
            type="button"
            className={`inline-flex items-center gap-1.5 p-2.5 border-b-2 rounded-t-container ${
              activeTab === 'tools'
                ? 'text-accent border-accent'
                : 'text-ink-muted border-transparent hover:text-ink-muted hover:border-strong'
            }`}
            onClick={handleToolsTabClick}
          >
            <FiTool className="w-4 h-4" />
            {t('Tools')}
          </button>
        </li>
      </ul>
    </div>
  )
}
