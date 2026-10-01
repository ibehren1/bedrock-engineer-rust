import { useMemo } from 'react'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { ToolState } from '@/types/agent-chat'
import { ToolName } from '@/types/tools'
import { READ_ONLY_TOOLS } from '@/types/plan-mode-tools'

/**
 * Custom hook to filter tools based on Plan/Act mode
 * @param tools Array of tool states to filter
 * @returns Filtered array of tool states based on current mode
 */
export const useAgentTools = (tools: ToolState[]): ToolState[] => {
  const { planMode } = useSettings()

  return useMemo(() => {
    if (!planMode) {
      // In Act mode, return all enabled tools
      return tools
    }

    // In Plan mode, only return read-only tools
    return tools.filter((tool) => {
      const toolName = tool.toolSpec?.name as ToolName
      const isReadOnly = READ_ONLY_TOOLS.includes(toolName)

      return tool.enabled && isReadOnly
    })
  }, [tools, planMode])
}
