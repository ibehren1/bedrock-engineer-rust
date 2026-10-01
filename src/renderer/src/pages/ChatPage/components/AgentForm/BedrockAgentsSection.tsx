import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { BedrockAgent } from '@/types/agent'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { Button, Input, Label, Textarea } from '@renderer/components/ui'

interface BedrockAgentsSectionProps {
  agents: BedrockAgent[]
  onChange: (agents: BedrockAgent[]) => void
}

export const BedrockAgentsSection: React.FC<BedrockAgentsSectionProps> = ({
  agents = [],
  onChange
}) => {
  const { t } = useTranslation()
  const [newAgentId, setNewAgentId] = useState('')
  const [newAliasId, setNewAliasId] = useState('')
  const [newDescription, setNewDescription] = useState('')
  const [editMode, setEditMode] = useState<string | null>(null)
  const [editData, setEditData] = useState<BedrockAgent>({
    agentId: '',
    aliasId: '',
    description: ''
  })

  const handleAddAgent = () => {
    if (newAgentId.trim() && newAliasId.trim() && newDescription.trim()) {
      onChange([
        ...agents,
        {
          agentId: newAgentId.trim(),
          aliasId: newAliasId.trim(),
          description: newDescription.trim()
        }
      ])
      setNewAgentId('')
      setNewAliasId('')
      setNewDescription('')
    }
  }

  const handleRemoveAgent = (agentId: string) => {
    onChange(agents.filter((agent) => agent.agentId !== agentId))
  }

  const handleEditAgent = (agent: BedrockAgent) => {
    setEditMode(agent.agentId)
    setEditData({ ...agent })
  }

  const handleSaveEdit = () => {
    if (editData.agentId.trim() && editData.aliasId.trim() && editData.description.trim()) {
      onChange(agents.map((agent) => (agent.agentId === editMode ? { ...editData } : agent)))
      setEditMode(null)
      setEditData({
        agentId: '',
        aliasId: '',
        description: ''
      })
    }
  }

  const handleCancelEdit = () => {
    setEditMode(null)
    setEditData({
      agentId: '',
      aliasId: '',
      description: ''
    })
  }

  return (
    <div className="space-y-2 border border-subtle rounded-control p-2.5">
      <div className="flex justify-between items-center">
        <h3 className="text-heading font-semibold text-ink">{t('Bedrock Agents')}</h3>
      </div>

      <p className="text-sm text-ink-muted">
        {t('Configure which Bedrock Agents this agent can access.')}
      </p>

      {/* Bedrock Agent 追加フォーム */}
      <div className="flex flex-col gap-2 mt-4">
        <h4 className="font-medium text-sm mb-2">{t('Add New Bedrock Agent')}</h4>

        <div>
          <Label>{t('Agent ID')}</Label>
          <Input
            type="text"
            value={newAgentId}
            onChange={(e) => setNewAgentId(e.target.value)}
            placeholder="e.g., AGENT123456"
          />
        </div>

        <div className="mt-2">
          <Label>{t('Alias ID')}</Label>
          <Input
            type="text"
            value={newAliasId}
            onChange={(e) => setNewAliasId(e.target.value)}
            placeholder="e.g., ALIAS123456"
          />
        </div>

        <div className="mt-2">
          <Label>{t('Description')}</Label>
          <Textarea
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            placeholder="e.g., Code interpreter agent"
            rows={2}
          />
        </div>

        <Button
          onClick={handleAddAgent}
          disabled={!newAgentId.trim() || !newAliasId.trim() || !newDescription.trim()}
          variant="primary"
          className="mt-2 w-fit"
        >
          {t('Add Agent')}
        </Button>
      </div>

      {/* 登録済み Bedrock Agent リスト */}
      <div className="space-y-3 mt-3">
        <h4 className="font-medium text-sm">{t('Available Bedrock Agents')}</h4>
        <div className="grid grid-cols-1 gap-2">
          {agents.length === 0 ? (
            <p className="text-sm text-ink-muted italic">{t('No Bedrock Agents registered yet')}</p>
          ) : (
            agents.map((agent) => (
              <div
                key={agent.agentId}
                className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
              >
                {editMode === agent.agentId ? (
                  // 編集モード
                  <div className="flex flex-col gap-2">
                    <div>
                      <Label>{t('Agent ID')}</Label>
                      <Input
                        type="text"
                        value={editData.agentId}
                        onChange={(e) => setEditData({ ...editData, agentId: e.target.value })}
                      />
                    </div>
                    <div className="mt-2">
                      <Label>{t('Alias ID')}</Label>
                      <Input
                        type="text"
                        value={editData.aliasId}
                        onChange={(e) => setEditData({ ...editData, aliasId: e.target.value })}
                      />
                    </div>
                    <div className="mt-2">
                      <Label>{t('Description')}</Label>
                      <Textarea
                        value={editData.description}
                        onChange={(e) => setEditData({ ...editData, description: e.target.value })}
                        rows={2}
                      />
                    </div>
                    <div className="flex justify-end gap-2 mt-2">
                      <button
                        onClick={handleCancelEdit}
                        className="px-3 py-1 text-sm text-ink-muted border border-strong rounded-control hover:bg-raised"
                      >
                        {t('Cancel')}
                      </button>
                      <Button
                        onClick={handleSaveEdit}
                        disabled={
                          !editData.agentId.trim() ||
                          !editData.aliasId.trim() ||
                          !editData.description.trim()
                        }
                        variant="primary"
                      >
                        {t('Save')}
                      </Button>
                    </div>
                  </div>
                ) : (
                  // 表示モード
                  <>
                    <div className="flex items-center justify-between">
                      <div>
                        <span className="font-medium">{agent.description}</span>
                        <div className="text-xs text-ink-muted mt-1">
                          <div>
                            <span className="font-mono">Agent ID:</span> {agent.agentId}
                          </div>
                          <div>
                            <span className="font-mono">Alias ID:</span> {agent.aliasId}
                          </div>
                        </div>
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditAgent(agent)}
                          className="text-accent hover:text-accent p-1"
                          title="Edit"
                          aria-label="Edit agent"
                        >
                          <EditIcon />
                        </button>
                        <button
                          onClick={() => handleRemoveAgent(agent.agentId)}
                          className="text-danger hover:text-danger-strong p-1"
                          title="Remove"
                          aria-label="Remove agent"
                        >
                          <RemoveIcon />
                        </button>
                      </div>
                    </div>
                  </>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}

export default BedrockAgentsSection
