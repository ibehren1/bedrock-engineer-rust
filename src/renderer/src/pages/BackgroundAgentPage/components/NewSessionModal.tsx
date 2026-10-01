import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiX } from 'react-icons/fi'
import { ModelSelector } from '../../ChatPage/components/ModelSelector'
import { Agent } from '@/types/agent-chat'
import { TbRobot } from 'react-icons/tb'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import useSetting from '@renderer/hooks/useSetting'

interface NewSessionModalProps {
  isOpen: boolean
  onClose: () => void
  onCreateSession: (config: { modelId: string; agentId: string; systemPrompt: string }) => void
  agents: readonly Agent[]
}

export const NewSessionModal: React.FC<NewSessionModalProps> = ({
  isOpen,
  onClose,
  onCreateSession,
  agents
}) => {
  const { t } = useTranslation()
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null)
  const { currentLLM } = useSetting()

  const selectedAgent = agents.find((agent) => agent.id === selectedAgentId)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    // エージェントが選択されていない場合はエラー
    if (!selectedAgentId) {
      alert(t('backgroundAgent.newSession.selectAgent'))
      return
    }

    // 現在選択されているモデルIDを取得
    const modelId = currentLLM?.modelId || 'anthropic.claude-3-haiku-20240307-v1:0'

    // エージェントのシステムプロンプトを使用
    const systemPrompt = selectedAgent?.system || 'You are a helpful assistant.'

    onCreateSession({
      modelId,
      agentId: selectedAgentId,
      systemPrompt
    })

    // リセット
    setSelectedAgentId(null)
    onClose()
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-surface rounded-container shadow-xl w-full max-w-md mx-4">
        {/* ヘッダー */}
        <div className="flex items-center justify-between p-3 border-b border-subtle">
          <h2 className="text-heading font-semibold text-ink">
            {t('backgroundAgent.newSession.title')}
          </h2>
          <button onClick={onClose} className="text-ink-faint hover:text-ink-muted">
            <FiX className="w-4 h-4" />
          </button>
        </div>

        {/* フォーム */}
        <form onSubmit={handleSubmit} className="p-3 space-y-3">
          {/* モデル選択 */}
          <div>
            <label className="block text-sm font-medium text-ink mb-2">
              {t('backgroundAgent.newSession.modelSelection')}
            </label>
            <ModelSelector openable={true} />
          </div>

          {/* エージェント選択 */}
          <div>
            <label className="block text-sm font-medium text-ink mb-2">
              {t('backgroundAgent.newSession.agentSelection')}
            </label>
            <div className="relative">
              <select
                value={selectedAgentId || ''}
                onChange={(e) => setSelectedAgentId(e.target.value || null)}
                className="w-full px-3 py-2 border border-strong rounded-control focus:outline-none focus:ring-2 focus:ring-accent bg-raised text-ink appearance-none"
              >
                <option value="">{t('backgroundAgent.newSession.noAgent')}</option>
                {agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.name}
                  </option>
                ))}
              </select>
            </div>

            {/* 選択されたエージェントの詳細表示 */}
            {selectedAgent && (
              <div className="mt-3 p-3 bg-surface-2 rounded-control">
                <div className="flex items-center gap-2 mb-2">
                  {selectedAgent.icon ? (
                    <AgentIconView
                      icon={selectedAgent.icon}
                      iconColor={selectedAgent.iconColor}
                      className="w-4 h-4"
                    />
                  ) : (
                    <TbRobot className="w-4 h-4" />
                  )}
                  <span className="font-medium text-sm text-ink">{selectedAgent.name}</span>
                </div>
                <p className="text-xs text-ink-muted">{selectedAgent.description}</p>
              </div>
            )}
          </div>

          {/* アクションボタン */}
          <div className="flex gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 px-2.5 py-1 text-ink border border-strong rounded-control hover:bg-surface-2 transition-colors"
            >
              {t('backgroundAgent.newSession.cancel')}
            </button>
            <button
              type="submit"
              className="flex-1 px-2.5 py-1 bg-accent text-accent-fg rounded-control hover:bg-accent-strong transition-colors"
            >
              {t('backgroundAgent.newSession.create')}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
