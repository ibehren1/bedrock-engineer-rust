import { KnowledgeBase } from 'src/types/agent-chat'
import { useState } from 'react'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { useTranslation } from 'react-i18next'
import { Button, Input, Label, Textarea } from '@renderer/components/ui'

export const KnowledgeBaseSettingForm = ({
  knowledgeBases,
  setKnowledgeBases
}: {
  knowledgeBases: KnowledgeBase[]
  setKnowledgeBases: (knowledgeBase: KnowledgeBase[]) => void
}) => {
  const [newKnowledgeBaseId, setKnowledgeBaseId] = useState('')
  const [newDescription, setNewDescription] = useState('')
  const { t } = useTranslation()
  const [editMode, setEditMode] = useState<string | null>(null)
  const [editData, setEditData] = useState<KnowledgeBase>({ knowledgeBaseId: '', description: '' })

  const handleAddKB = () => {
    if (newKnowledgeBaseId.trim() && newDescription.trim()) {
      setKnowledgeBases([
        ...knowledgeBases,
        {
          knowledgeBaseId: newKnowledgeBaseId.trim(),
          description: newDescription.trim()
        }
      ])
      setKnowledgeBaseId('')
      setNewDescription('')
    }
  }

  const handleRemoveKB = (knowledgeBaseId: string) => {
    setKnowledgeBases(knowledgeBases.filter((kb) => kb.knowledgeBaseId !== knowledgeBaseId))
  }

  const handleEditKB = (kb: KnowledgeBase) => {
    setEditMode(kb.knowledgeBaseId)
    setEditData({ ...kb })
  }

  const handleSaveEdit = () => {
    if (editData.knowledgeBaseId.trim() && editData.description.trim()) {
      setKnowledgeBases(
        knowledgeBases.map((kb) => (kb.knowledgeBaseId === editMode ? { ...editData } : kb))
      )
      setEditMode(null)
      setEditData({ knowledgeBaseId: '', description: '' })
    }
  }

  const handleCancelEdit = () => {
    setEditMode(null)
    setEditData({ knowledgeBaseId: '', description: '' })
  }

  return (
    <div className="mt-4 space-y-2">
      {/* ツールの説明 */}
      <div className="max-w-none">
        <p className="mb-4 text-ink">{t('tool info.retrieve.description')}</p>

        <div className="bg-accent-tint p-2.5 rounded-control mb-5">
          <h5 className="font-medium mb-2 text-ink">{t('tool info.retrieve.about title')}</h5>
          <p className="text-sm text-ink">{t('tool info.retrieve.about description')}</p>
        </div>
      </div>

      {/* KnowledgeBase 追加フォーム */}
      <div className="flex flex-col gap-2 p-2.5 border border-subtle rounded-control">
        <h4 className="font-medium text-sm mb-2 text-ink">{t('Add New Knowledge Base')}</h4>
        <div className="flex-grow">
          <Label>{t('Knowledge Base ID')}</Label>
          <Input
            type="text"
            value={newKnowledgeBaseId}
            onChange={(e) => setKnowledgeBaseId(e.target.value)}
            placeholder="e.g., BM7GYFCKIA"
          />
        </div>
        <div className="flex-grow">
          <Label>{t('Description')}</Label>
          <Textarea
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            placeholder="e.g., Stores in-house manuals and past inquiry history"
            rows={3}
          />
        </div>

        <Button
          onClick={handleAddKB}
          disabled={!newKnowledgeBaseId.trim() || !newDescription.trim()}
          variant="primary"
        >
          {t('Add Knowledge Base')}
        </Button>
      </div>

      {/* 登録済み KnowledgeBase リスト */}
      <div className="space-y-3 mt-3">
        <h4 className="font-medium text-sm text-ink">{t('Registered Knowledge Bases')}</h4>
        {knowledgeBases.length === 0 ? (
          <p className="text-sm text-ink-muted italic">{t('No knowledge bases registered yet')}</p>
        ) : (
          knowledgeBases.map((kb) => (
            <div
              key={kb.knowledgeBaseId}
              className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
            >
              {editMode === kb.knowledgeBaseId ? (
                // 編集モード
                <div className="flex flex-col gap-2">
                  <div className="flex-grow">
                    <Label>{t('Knowledge Base ID')}</Label>
                    <Input
                      type="text"
                      value={editData.knowledgeBaseId}
                      onChange={(e) =>
                        setEditData({ ...editData, knowledgeBaseId: e.target.value })
                      }
                    />
                  </div>
                  <div className="flex-grow">
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
                    <span className="font-mono">
                      {t('Knowledge Base ID')}: {kb.knowledgeBaseId}
                    </span>
                    <div className="flex gap-2">
                      <button
                        onClick={() => handleEditKB(kb)}
                        className="text-accent hover:text-accent p-1"
                        title="Edit"
                        aria-label="Edit knowledge base"
                      >
                        <EditIcon />
                      </button>
                      <button
                        onClick={() => handleRemoveKB(kb.knowledgeBaseId)}
                        className="text-danger hover:text-danger-strong p-1"
                        title="Remove"
                        aria-label="Remove knowledge base"
                      >
                        <RemoveIcon />
                      </button>
                    </div>
                  </div>
                  <p className="text-xs text-ink-muted mt-1 whitespace-pre-line">
                    {kb.description}
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
