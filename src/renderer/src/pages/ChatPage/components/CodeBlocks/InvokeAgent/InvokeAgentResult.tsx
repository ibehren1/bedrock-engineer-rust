import React from 'react'
import Markdown from 'react-markdown'
import { TbRobot } from 'react-icons/tb'
import { FiAlertTriangle, FiClock, FiTool } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import type { InvokeAgentResult as InvokeAgentResultType } from '@/types/tools'

const renderAgentIcon = (icon?: string, iconColor?: string) => (
  <AgentIconView
    icon={icon}
    iconColor={iconColor}
    className="w-4 h-4"
    fallback={<TbRobot className="w-4 h-4" />}
  />
)

/**
 * Renders the outcome of a delegated task: which agent ran, what it answered,
 * and how much work it did. Progress is not streamed, so this is the only view
 * of the sub-agent's run inside the chat.
 */
export const InvokeAgentResult: React.FC<{ response: InvokeAgentResultType }> = ({ response }) => {
  const { t } = useTranslation()
  const { result } = response

  if (!result) {
    return <div className="text-sm text-danger">{response.error || response.message}</div>
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 text-sm">
        <span className="text-ink-muted">
          {renderAgentIcon(result.agentIcon, result.agentIconColor)}
        </span>
        <span className="font-medium text-ink">{result.agentName}</span>
        <span className="rounded-control bg-raised px-1.5 py-0.5 text-xs text-ink-muted">
          {t('invokeAgentResult.depth', { depth: result.depth })}
        </span>
      </div>

      <div className="max-w-none">
        <Markdown>{result.finalText}</Markdown>
      </div>

      {result.truncated && (
        <div className="flex items-center gap-1.5 text-xs text-warning">
          <FiAlertTriangle className="shrink-0" />
          {t('invokeAgentResult.truncated')}
        </div>
      )}

      {result.stoppedReason === 'maxToolExecutions' && (
        <div className="flex items-center gap-1.5 text-xs text-warning">
          <FiAlertTriangle className="shrink-0" />
          {t('invokeAgentResult.toolBudgetReached')}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-faint pt-2 text-xs text-ink-muted">
        <span className="flex items-center gap-1">
          <FiTool className="shrink-0" />
          {t('invokeAgentResult.toolCalls', { count: result.toolCallCount })}
        </span>
        <span className="flex items-center gap-1">
          <FiClock className="shrink-0" />
          {(result.durationMs / 1000).toFixed(1)}s
        </span>
        {result.usage?.totalTokens ? (
          <span>{t('invokeAgentResult.tokens', { count: result.usage.totalTokens })}</span>
        ) : null}
        {result.toolNames.length > 0 && (
          <span className="flex flex-wrap gap-1">
            {result.toolNames.map((name) => (
              <span key={name} className="rounded-control bg-raised px-1.5 py-0.5 text-ink">
                {name}
              </span>
            ))}
          </span>
        )}
      </div>
    </div>
  )
}
