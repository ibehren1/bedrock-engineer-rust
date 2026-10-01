import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiChevronDown, FiChevronRight } from 'react-icons/fi'
import { KnowledgeBasesContent } from './KnowledgeBasesContent'
import { CommandsContent } from './CommandsContent'
import { BedrockAgentsContent } from './BedrockAgentsContent'
import { FlowsContent } from './FlowsContent'
import { ToolDetailsTabProps } from '../../types'
import { preventEventPropagation } from '../../utils/eventUtils'

/**
 * ツール詳細設定タブコンポーネント
 */
export const ToolDetailsTab: React.FC<ToolDetailsTabProps> = ({
  enabledTools,
  expandedTools,
  toggleToolExpand,
  toolsWithConfigurations,
  knowledgeBases,
  onKnowledgeBasesChange,
  allowedCommands,
  onAllowedCommandsChange,
  bedrockAgents,
  onBedrockAgentsChange,
  flows = [],
  onFlowsChange = () => console.warn('onFlowsChange not provided')
}) => {
  const { t } = useTranslation()

  return (
    <div className="space-y-2" onClick={preventEventPropagation}>
      <div className="mb-4 bg-accent-tint p-3 rounded-control">
        <p className="text-sm text-ink font-medium mb-1">
          {t('Configure settings for enabled tools')}
        </p>
        <p className="text-xs text-ink-muted">{t('Tool Detail Settings Description')}</p>
      </div>

      {enabledTools.length === 0 ? (
        <div className="p-2.5 bg-surface-2 rounded-control text-center">
          <p className="text-ink-muted">
            {t('No tools enabled. Enable tools in the Available Tools tab to configure them.')}
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {/* retrieve ツール設定 */}
          {toolsWithConfigurations.retrieve.isEnabled && (
            <div className="border border-subtle rounded-container overflow-hidden">
              <div
                className="flex items-center justify-between p-2.5 bg-surface-2 cursor-pointer"
                onClick={(e) => {
                  preventEventPropagation(e)
                  toggleToolExpand('retrieve')
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="text-ink-muted">
                    {expandedTools.retrieve ? <FiChevronDown /> : <FiChevronRight />}
                  </div>
                  <div>
                    <h4 className="font-medium text-ink">
                      {toolsWithConfigurations.retrieve.title}
                    </h4>
                    <p className="text-xs text-ink-muted">
                      {toolsWithConfigurations.retrieve.description}
                    </p>
                  </div>
                </div>
              </div>

              {expandedTools.retrieve && (
                <div className="p-2.5 border-t border-subtle">
                  <KnowledgeBasesContent
                    knowledgeBases={knowledgeBases}
                    onChange={onKnowledgeBasesChange}
                  />
                </div>
              )}
            </div>
          )}

          {/* executeCommand ツール設定 */}
          {toolsWithConfigurations.executeCommand.isEnabled && (
            <div className="border border-subtle rounded-container overflow-hidden">
              <div
                className="flex items-center justify-between p-2.5 bg-surface-2 cursor-pointer"
                onClick={(e) => {
                  preventEventPropagation(e)
                  toggleToolExpand('executeCommand')
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="text-ink-muted">
                    {expandedTools.executeCommand ? <FiChevronDown /> : <FiChevronRight />}
                  </div>
                  <div>
                    <h4 className="font-medium text-ink">
                      {toolsWithConfigurations.executeCommand.title}
                    </h4>
                    <p className="text-xs text-ink-muted">
                      {toolsWithConfigurations.executeCommand.description}
                    </p>
                  </div>
                </div>
              </div>

              {expandedTools.executeCommand && (
                <div className="p-2.5 border-t border-subtle">
                  <CommandsContent commands={allowedCommands} onChange={onAllowedCommandsChange} />
                </div>
              )}
            </div>
          )}

          {/* invokeBedrockAgent ツール設定 */}
          {toolsWithConfigurations.invokeBedrockAgent.isEnabled && (
            <div className="border border-subtle rounded-container overflow-hidden">
              <div
                className="flex items-center justify-between p-2.5 bg-surface-2 cursor-pointer"
                onClick={(e) => {
                  preventEventPropagation(e)
                  toggleToolExpand('invokeBedrockAgent')
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="text-ink-muted">
                    {expandedTools.invokeBedrockAgent ? <FiChevronDown /> : <FiChevronRight />}
                  </div>
                  <div>
                    <h4 className="font-medium text-ink">
                      {toolsWithConfigurations.invokeBedrockAgent.title}
                    </h4>
                    <p className="text-xs text-ink-muted">
                      {toolsWithConfigurations.invokeBedrockAgent.description}
                    </p>
                  </div>
                </div>
              </div>

              {expandedTools.invokeBedrockAgent && (
                <div className="p-2.5 border-t border-subtle">
                  <BedrockAgentsContent agents={bedrockAgents} onChange={onBedrockAgentsChange} />
                </div>
              )}
            </div>
          )}

          {/* invokeFlow ツール設定 */}
          {toolsWithConfigurations.invokeFlow.isEnabled && (
            <div className="border border-subtle rounded-container overflow-hidden">
              <div
                className="flex items-center justify-between p-2.5 bg-surface-2 cursor-pointer"
                onClick={(e) => {
                  preventEventPropagation(e)
                  toggleToolExpand('invokeFlow')
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="text-ink-muted">
                    {expandedTools.invokeFlow ? <FiChevronDown /> : <FiChevronRight />}
                  </div>
                  <div>
                    <h4 className="font-medium text-ink">
                      {toolsWithConfigurations.invokeFlow.title}
                    </h4>
                    <p className="text-xs text-ink-muted">
                      {toolsWithConfigurations.invokeFlow.description}
                    </p>
                  </div>
                </div>
              </div>

              {expandedTools.invokeFlow && (
                <div className="p-2.5 border-t border-subtle">
                  <FlowsContent flows={flows} onChange={onFlowsChange} />
                </div>
              )}
            </div>
          )}

          {/* 有効なツールがあるが、設定が必要なツールがない場合 */}
          {enabledTools.length > 0 &&
            !Object.values(toolsWithConfigurations).some((config) => config.isEnabled) && (
              <div className="p-2.5 bg-surface-2 rounded-control text-center">
                <p className="text-ink-muted">
                  {t(
                    'No configurable tools enabled. Enable retrieve, executeCommand, invokeBedrockAgent, or invokeFlow tools to access their configurations.'
                  )}
                </p>
              </div>
            )}
        </div>
      )}
    </div>
  )
}
