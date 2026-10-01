import React from 'react'
import { useTranslation } from 'react-i18next'
import { ToggleSwitch } from 'flowbite-react'
import { FiServer } from 'react-icons/fi'
import { getOriginalMcpToolName, ToolName } from '@/types/tools'
import { toolIcons } from '@renderer/components/icons/ToolIcons'
import { ToolItemProps } from '../../types'
import { preventEventPropagation } from '../../utils/eventUtils'

/**
 * ツールアイテムコンポーネント
 */
export const ToolItem: React.FC<ToolItemProps> = ({ tool, isMcp, serverInfo, onToggle }) => {
  const { t } = useTranslation()

  const toolName = tool.toolSpec?.name
  if (!toolName) return null

  // ツール名の表示
  const displayedName = isMcp ? getOriginalMcpToolName(toolName) : toolName

  // ツールアイコン
  const ToolIcon = isMcp ? (
    <FiServer className="h-4 w-4" />
  ) : toolName ? (
    toolIcons[toolName as ToolName]
  ) : null

  return (
    <div
      className={`flex items-center justify-between p-3 ${
        isMcp ? 'bg-surface-2 border border-strong' : 'bg-surface-2 border border-strong'
      } rounded-control hover:border-strong transition-colors duration-200`}
    >
      <div className="flex items-center space-x-3 flex-1 min-w-0">
        <div className="text-ink-muted flex-shrink-0 w-7 h-7 flex items-center justify-center">
          {ToolIcon}
        </div>
        <div className="min-w-0 flex-1">
          <p className="font-medium text-ink break-words line-clamp-2" title={displayedName}>
            {displayedName}
            {isMcp && (
              <span className="ml-1 text-xs font-normal bg-raised text-ink py-0.5 px-1 rounded-control">
                MCP
              </span>
            )}
          </p>
          <div>
            {isMcp ? (
              <p className="text-xs text-ink-muted line-clamp-2 overflow-hidden">
                {tool.toolSpec?.description || t('MCP tool from Model Context Protocol server')}
                {serverInfo && (
                  <span className="block mt-0.5 text-ink-muted truncate">{serverInfo}</span>
                )}
              </p>
            ) : (
              <p className="text-xs text-ink-muted line-clamp-2 overflow-hidden">
                {toolName ? t(`descriptions.${toolName}`) : ''}
              </p>
            )}
          </div>
        </div>
      </div>
      <div className="flex-shrink-0" onClick={preventEventPropagation}>
        {isMcp ? (
          <div className="flex items-center">
            <ToggleSwitch checked={true} onChange={() => {}} disabled={true} label="" />
          </div>
        ) : (
          <ToggleSwitch checked={tool.enabled} onChange={() => onToggle(toolName)} label="" />
        )}
      </div>
    </div>
  )
}
