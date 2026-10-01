import React from 'react'
import { useTranslation } from 'react-i18next'
import { ToolName, isMcpTool } from '@/types/tools'
import { toolIcons } from '@renderer/components/icons/ToolIcons'

interface ToolsListProps {
  tools: string[]
}

export const ToolsList: React.FC<ToolsListProps> = ({ tools }) => {
  const { t } = useTranslation()

  if (!tools || tools.length === 0) {
    return null
  }

  // MCPツールを除外
  const standardTools = tools.filter((tool) => !isMcpTool(tool))

  if (standardTools.length === 0) {
    return null
  }

  return (
    <div>
      <h3 className="text-heading font-medium mb-2 text-ink">{t('toolsLabel')}</h3>
      <div className="p-2.5 bg-surface-2 rounded-container border border-subtle">
        <div className="flex flex-wrap gap-2">
          {standardTools.map((tool) => (
            <span
              key={tool}
              className="bg-accent-tint text-accent text-xs font-medium px-2.5 py-1.5 rounded-control flex items-center gap-1.5"
            >
              <span className="flex-shrink-0 [&>svg]:!size-4">{toolIcons[tool as ToolName]}</span>
              <span>{tool}</span>
            </span>
          ))}
        </div>
      </div>
    </div>
  )
}
