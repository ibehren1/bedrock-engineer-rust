import React from 'react'
import { useTranslation } from 'react-i18next'
import { ToolCategorySection } from './ToolCategorySection'
import { AvailableToolsTabProps } from '../../types'
import { preventEventPropagation } from '../../utils/eventUtils'

/**
 * 利用可能なツールタブコンポーネント
 */
export const AvailableToolsTab: React.FC<AvailableToolsTabProps> = ({
  categorizedTools,
  mcpServers,
  onToggleTool,
  onShowToolInfo,
  isLoadingMcpTools = false
}) => {
  const { t } = useTranslation()

  return (
    <div className="space-y-3" onClick={preventEventPropagation}>
      <p className="text-xs text-ink-muted mb-1 mt-1">{t('tools.description')}</p>

      {/* ツールカテゴリセクションをループで表示 */}
      {categorizedTools.map((category) => (
        <ToolCategorySection
          key={category.id}
          category={category}
          mcpServers={mcpServers}
          onToggleTool={onToggleTool}
          onShowToolInfo={onShowToolInfo}
          isLoadingMcpTools={category.id === 'mcp' ? isLoadingMcpTools : false}
        />
      ))}
    </div>
  )
}
