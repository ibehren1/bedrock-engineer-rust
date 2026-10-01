import { BedrockAgent } from '@/types/agent'
import { memo, useState } from 'react'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { useTranslation } from 'react-i18next'
import { Button, Input, Label, Textarea } from '@renderer/components/ui'

interface BedrockAgentSettingFormProps {
  bedrockAgents: BedrockAgent[]
  setBedrockAgents: (agents: BedrockAgent[]) => void
}

export const BedrockAgentSettingForm = memo(
  ({ bedrockAgents, setBedrockAgents }: BedrockAgentSettingFormProps) => {
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
        setBedrockAgents([
          ...bedrockAgents,
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
      setBedrockAgents(bedrockAgents.filter((agent) => agent.agentId !== agentId))
    }

    const handleEditAgent = (agent: BedrockAgent) => {
      setEditMode(agent.agentId)
      setEditData({ ...agent })
    }

    const handleSaveEdit = () => {
      if (editData.agentId.trim() && editData.aliasId.trim() && editData.description.trim()) {
        setBedrockAgents(
          bedrockAgents.map((agent) => (agent.agentId === editMode ? { ...editData } : agent))
        )
        setEditMode(null)
        setEditData({ agentId: '', aliasId: '', description: '' })
      }
    }

    const handleCancelEdit = () => {
      setEditMode(null)
      setEditData({ agentId: '', aliasId: '', description: '' })
    }

    return (
      <div className="mt-4 space-y-2">
        {/* ツールの説明 */}
        <div className="max-w-none">
          <p className="mb-4 text-ink">{t('tool info.invokeBedrockAgent.description')}</p>

          <div className="bg-accent-tint p-2.5 rounded-control mb-5">
            <h5 className="font-medium mb-2 text-ink">
              {t('tool info.invokeBedrockAgent.about title')}
            </h5>
            <p className="text-sm text-ink">
              {t('tool info.invokeBedrockAgent.about description')}
            </p>
          </div>

          <div className="bg-warning-soft p-2.5 rounded-control mb-5">
            <h5 className="font-medium mb-2 text-warning">
              {t('tool info.invokeBedrockAgent.file limitations title')}
            </h5>
            <p className="text-sm text-warning">
              {t('tool info.invokeBedrockAgent.file limitations description')}
            </p>
          </div>
        </div>

        {/* Agent 追加フォーム */}
        <div className="flex flex-col gap-2 p-2.5 border border-subtle rounded-control">
          <h4 className="font-medium text-sm mb-2 text-ink">{t('Add New Bedrock Agent')}</h4>
          <div className="flex gap-2">
            <div className="flex-grow">
              <Label>{t('Agent ID')}</Label>
              <Input
                type="text"
                value={newAgentId}
                onChange={(e) => setNewAgentId(e.target.value)}
                placeholder="e.g., VREKDPSXYP"
              />
            </div>
            <div className="flex-grow">
              <Label>{t('Alias ID')}</Label>
              <Input
                type="text"
                value={newAliasId}
                onChange={(e) => setNewAliasId(e.target.value)}
                placeholder="e.g., ZHSSM0WPXS"
              />
            </div>
          </div>
          <div>
            <Label>{t('Description')}</Label>
            <Textarea
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
              placeholder="e.g., Agent for processing customer inquiries"
              rows={3}
            />
          </div>
          <Button
            onClick={handleAddAgent}
            disabled={!newAgentId.trim() || !newAliasId.trim() || !newDescription.trim()}
            variant="primary"
          >
            {t('Add Agent')}
          </Button>
        </div>

        {/* 登録済み Agent リスト */}
        <div className="space-y-3 mt-3">
          <h4 className="font-medium text-sm text-ink">{t('Registered Agents')}</h4>
          {bedrockAgents.length === 0 ? (
            <p className="text-sm text-ink-muted italic">{t('No agents registered yet')}</p>
          ) : (
            bedrockAgents.map((agent) => (
              <div
                key={agent.agentId + '_' + agent.aliasId}
                className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
              >
                {editMode === agent.agentId ? (
                  // 編集モード
                  <div className="flex flex-col gap-2">
                    <div className="grid grid-cols-2 gap-2">
                      <div>
                        <Label>{t('Agent ID')}</Label>
                        <Input
                          type="text"
                          value={editData.agentId}
                          onChange={(e) => setEditData({ ...editData, agentId: e.target.value })}
                        />
                      </div>
                      <div>
                        <Label>{t('Alias ID')}</Label>
                        <Input
                          type="text"
                          value={editData.aliasId}
                          onChange={(e) => setEditData({ ...editData, aliasId: e.target.value })}
                        />
                      </div>
                    </div>
                    <div>
                      <Label>{t('Description')}</Label>
                      <Textarea
                        value={editData.description}
                        onChange={(e) => setEditData({ ...editData, description: e.target.value })}
                        rows={3}
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
                    <div className="flex items-center justify-between mb-2">
                      <div className="grid grid-cols-2 w-full">
                        <span className="font-mono">
                          {t('Agent ID')}: {agent.agentId}
                        </span>
                        <span className="font-mono">
                          {t('Alias ID')}: {agent.aliasId}
                        </span>
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
                    <p className="text-xs text-ink-muted whitespace-pre-line">
                      {agent.description}
                    </p>
                  </>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    )
  }
)

BedrockAgentSettingForm.displayName = 'BedrockAgentSettingForm'
