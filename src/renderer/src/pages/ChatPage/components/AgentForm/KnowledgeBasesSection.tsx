import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { KnowledgeBase } from '@/types/agent-chat'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { Button, Input, Label, Textarea } from '@renderer/components/ui'

interface KnowledgeBasesSectionProps {
  knowledgeBases: KnowledgeBase[]
  onChange: (knowledgeBases: KnowledgeBase[]) => void
}

export const KnowledgeBasesSection: React.FC<KnowledgeBasesSectionProps> = ({
  knowledgeBases = [],
  onChange
}) => {
  const { t } = useTranslation()
  const [newId, setNewId] = useState('')
  const [newDescription, setNewDescription] = useState('')
  const [editMode, setEditMode] = useState<string | null>(null)
  const [editData, setEditData] = useState<KnowledgeBase>({
    knowledgeBaseId: '',
    description: ''
  })

  const handleAddKnowledgeBase = () => {
    if (newId.trim() && newDescription.trim()) {
      onChange([
        ...knowledgeBases,
        {
          knowledgeBaseId: newId.trim(),
          description: newDescription.trim()
        }
      ])
      setNewId('')
      setNewDescription('')
    }
  }

  const handleRemoveKnowledgeBase = (id: string) => {
    onChange(knowledgeBases.filter((kb) => kb.knowledgeBaseId !== id))
  }

  const handleEditKnowledgeBase = (kb: KnowledgeBase) => {
    setEditMode(kb.knowledgeBaseId)
    setEditData({ ...kb })
  }

  const handleSaveEdit = () => {
    if (editData.knowledgeBaseId.trim() && editData.description.trim()) {
      onChange(knowledgeBases.map((kb) => (kb.knowledgeBaseId === editMode ? { ...editData } : kb)))
      setEditMode(null)
      setEditData({
        knowledgeBaseId: '',
        description: ''
      })
    }
  }

  const handleCancelEdit = () => {
    setEditMode(null)
    setEditData({
      knowledgeBaseId: '',
      description: ''
    })
  }

  return (
    <div className="space-y-2 border border-subtle rounded-control p-2.5">
      <div className="flex justify-between items-center">
        <h3 className="text-heading font-semibold text-ink">{t('Knowledge Bases')}</h3>
      </div>

      <p className="text-sm text-ink-muted">
        {t('Configure which knowledge bases this agent can access.')}
      </p>

      {/* Knowledge Base 追加フォーム */}
      <div className="flex flex-col gap-2 mt-4">
        <h4 className="font-medium text-sm mb-2">{t('Add New Knowledge Base')}</h4>

        <div>
          <Label>{t('Knowledge Base ID')}</Label>
          <Input
            type="text"
            value={newId}
            onChange={(e) => setNewId(e.target.value)}
            placeholder="e.g., KB123456"
          />
        </div>

        <div className="mt-2">
          <Label>{t('Description')}</Label>
          <Textarea
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            placeholder="e.g., Customer support knowledge base"
            rows={2}
          />
        </div>

        <Button
          onClick={handleAddKnowledgeBase}
          disabled={!newId.trim() || !newDescription.trim()}
          variant="primary"
          className="mt-2 w-fit"
        >
          {t('Add Knowledge Base')}
        </Button>
      </div>

      {/* 登録済み Knowledge Base リスト */}
      <div className="space-y-3 mt-3">
        <h4 className="font-medium text-sm">{t('Available Knowledge Bases')}</h4>
        <div className="grid grid-cols-1 gap-2">
          {knowledgeBases.length === 0 ? (
            <p className="text-sm text-ink-muted italic">
              {t('No knowledge bases registered yet')}
            </p>
          ) : (
            knowledgeBases.map((kb) => (
              <div
                key={kb.knowledgeBaseId}
                className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
              >
                {editMode === kb.knowledgeBaseId ? (
                  // 編集モード
                  <div className="flex flex-col gap-2">
                    <div>
                      <Label>{t('Knowledge Base ID')}</Label>
                      <Input
                        type="text"
                        value={editData.knowledgeBaseId}
                        onChange={(e) =>
                          setEditData({ ...editData, knowledgeBaseId: e.target.value })
                        }
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
                        disabled={!editData.knowledgeBaseId.trim() || !editData.description.trim()}
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
                        <span className="font-medium">{kb.description}</span>
                        <div className="text-xs text-ink-muted mt-1">
                          <div>
                            <span className="font-mono">ID:</span> {kb.knowledgeBaseId}
                          </div>
                        </div>
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditKnowledgeBase(kb)}
                          className="text-accent hover:text-accent p-1"
                          title="Edit"
                          aria-label="Edit knowledge base"
                        >
                          <EditIcon />
                        </button>
                        <button
                          onClick={() => handleRemoveKnowledgeBase(kb.knowledgeBaseId)}
                          className="text-danger hover:text-danger-strong p-1"
                          title="Remove"
                          aria-label="Remove knowledge base"
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

export default KnowledgeBasesSection
