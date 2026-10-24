import React from 'react'
import useSetting from '@renderer/hooks/useSetting'
import { AgentFormProps } from './types'
import { useAgentForm } from './useAgentForm'
import { formEventUtils } from './utils/formEventUtils'
import { useAgentFilter } from '../AgentList'

// 分割されたコンポーネントのインポート
import { AgentFormSidebar } from './components/AgentFormSidebar'
import { AgentFormContent } from './components/AgentFormContent'
import { FormActionButtons } from './components/FormActionButtons'

/**
 * エージェント作成・編集フォームコンポーネント
 * サイドバー形式のレイアウトに変更し、階層をより明確に表示
 */
export const AgentForm: React.FC<AgentFormProps> = ({ agent, onSave, onCancel }) => {
  const { projectPath, agents } = useSetting()
  const { availableTags } = useAgentFilter(agents)

  // フォーム状態管理フック
  const {
    formData,
    activeTab,
    agentTools,
    agentCategory,
    isLoadingMcpTools,
    tempMcpTools,
    generateSystemPrompt,
    generateScenarios,
    isGeneratingSystem,
    isGeneratingScenarios,
    updateField,
    handleSubmit,
    handleToolsChange,
    handleCategoryChange,
    handleTabChange,
    fetchMcpTools
  } = useAgentForm(agent, onSave)

  const handleAdditionalInstructionChange = React.useCallback(
    (value: string) => updateField('additionalInstruction', value),
    [updateField]
  )

  // Discarding a new agent also discards the knowledgebases created for it.
  const handleCancel = React.useCallback(() => {
    if (!agent?.id && formData.localKnowledgeBases?.length) {
      window.api.knowledge
        .deleteAgent(formData.id)
        .catch((error) => console.error('Failed to delete knowledge bases:', error))
    }
    onCancel()
  }, [agent?.id, formData.id, formData.localKnowledgeBases, onCancel])

  // 合成された生成状態
  const isGenerating = isGeneratingSystem || isGeneratingScenarios

  return (
    <div className="h-full">
      <form
        onSubmit={formEventUtils.createSubmitHandler(handleSubmit)}
        className="flex flex-col h-full"
        onClick={formEventUtils.preventPropagation}
      >
        {/* スクロール可能なメインコンテンツエリア */}
        <div className="flex flex-1 min-h-0 overflow-hidden">
          {/* 左側のサイドバーナビゲーション - 画面幅に応じて幅を変更 */}
          <div className="lg:w-64 w-16 border-r border-subtle flex-shrink-0 overflow-y-auto transition-all duration-300">
            <AgentFormSidebar
              activeTab={activeTab}
              onTabChange={handleTabChange}
              onToolsTabClick={() => fetchMcpTools(formData.mcpServers)}
            />
          </div>

          {/* 右側のコンテンツエリア - スクロール可能なコンテンツ */}
          <div className="flex-1 overflow-y-auto p-3 pb-16 bg-surface">
            {/* タブコンテンツ */}
            <AgentFormContent
              activeTab={activeTab}
              formData={formData}
              agentTools={agentTools}
              agentCategory={agentCategory}
              updateField={updateField}
              handleToolsChange={handleToolsChange}
              handleCategoryChange={handleCategoryChange}
              projectPath={projectPath}
              isLoadingMcpTools={isLoadingMcpTools}
              tempMcpTools={tempMcpTools}
              handleAutoGeneratePrompt={generateSystemPrompt}
              handleGenerateScenarios={generateScenarios}
              isGeneratingSystem={isGeneratingSystem}
              isGeneratingScenarios={isGeneratingScenarios}
              availableTags={availableTags}
              fetchMcpTools={fetchMcpTools}
              handleAdditionalInstructionChange={handleAdditionalInstructionChange}
              isSharedAgent={agent?.isShared}
            />
          </div>
        </div>

        {/* フッター固定部分 - 常に画面下部に表示 */}
        <div className="flex-shrink-0 px-3 py-2 sticky bottom-0 z-10 bg-surface border-t border-subtle backdrop-blur-sm">
          <FormActionButtons onCancel={handleCancel} isGenerating={isGenerating} />
        </div>
      </form>
    </div>
  )
}
