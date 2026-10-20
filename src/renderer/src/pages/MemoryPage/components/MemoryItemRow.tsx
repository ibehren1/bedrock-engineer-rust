import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiCheck, FiEdit2, FiTrash2, FiX } from 'react-icons/fi'
import type { MemoryItem, MemoryUpdateParams } from '@/types/bridge/memory'

type MemoryItemRowProps = {
  item: MemoryItem
  /** Resolves true when the update was saved. */
  onUpdate: (params: MemoryUpdateParams) => Promise<boolean>
  onDelete: (id: string) => void
}

export const MemoryItemRow: React.FC<MemoryItemRowProps> = ({ item, onUpdate, onDelete }) => {
  const { t, i18n } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(item.content)
  const [saving, setSaving] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (editing) {
      const el = textareaRef.current
      el?.focus()
      el?.setSelectionRange(el.value.length, el.value.length)
    }
  }, [editing])

  const startEdit = () => {
    setDraft(item.content)
    setEditing(true)
  }

  const cancelEdit = () => {
    setDraft(item.content)
    setEditing(false)
  }

  const saveEdit = async () => {
    const text = draft.trim()
    if (!text || saving) return
    if (text === item.content) {
      setEditing(false)
      return
    }
    setSaving(true)
    const saved = await onUpdate({ id: item.id, content: text })
    setSaving(false)
    if (saved) setEditing(false)
  }

  const date = new Date(item.updatedAt || item.createdAt).toLocaleDateString(i18n.language, {
    month: 'short',
    day: 'numeric'
  })
  const subtitle = `${t(`memoryItems.source.${item.source}`, item.source)} · ${date}`

  return (
    <div
      className={`group flex items-start gap-3 p-2.5 rounded-container bg-surface border border-subtle hover:bg-surface-2 transition-colors ${
        item.enabled ? '' : 'opacity-50'
      }`}
    >
      <input
        type="checkbox"
        checked={item.enabled}
        onChange={(e) => onUpdate({ id: item.id, enabled: e.target.checked })}
        title={t('memoryItems.includeInContext')}
        aria-label={t('memoryItems.includeInContext')}
        className="mt-0.5 h-4 w-4 rounded-control border-strong text-accent focus:ring-accent cursor-pointer"
      />

      <div className="flex-1 min-w-0">
        {editing ? (
          <div className="flex flex-col gap-1.5">
            <textarea
              ref={textareaRef}
              value={draft}
              rows={Math.min(6, Math.max(2, draft.split('\n').length))}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault()
                  saveEdit()
                } else if (e.key === 'Escape') {
                  e.preventDefault()
                  cancelEdit()
                }
              }}
              className="w-full text-sm px-2 py-1.5 border border-strong rounded-control bg-surface text-ink resize-y focus:outline-none focus:ring-2 focus:ring-accent"
            />
            <div className="flex items-center gap-2">
              <button
                onClick={saveEdit}
                disabled={!draft.trim() || saving}
                className="flex items-center gap-1 px-2 py-0.5 text-xs bg-accent text-accent-fg rounded-control hover:bg-accent-strong disabled:opacity-50"
              >
                <FiCheck className="w-3.5 h-3.5" />
                {t('memoryItems.save')}
              </button>
              <button
                onClick={cancelEdit}
                className="flex items-center gap-1 px-2 py-0.5 text-xs text-ink-muted border border-subtle rounded-control hover:text-ink"
              >
                <FiX className="w-3.5 h-3.5" />
                {t('memoryItems.cancel')}
              </button>
              <span className="text-xs text-ink-faint">{t('memoryItems.editHint')}</span>
            </div>
          </div>
        ) : (
          <p className="text-sm text-ink whitespace-pre-wrap break-words">{item.content}</p>
        )}
        <p className="text-xs text-ink-muted mt-1">{subtitle}</p>
      </div>

      <button
        onClick={() => onUpdate({ id: item.id, kind: item.kind === 'rule' ? 'memory' : 'rule' })}
        title={t('memoryItems.toggleKind')}
        className="shrink-0 text-micro rounded-control px-1.5 py-0.5 bg-surface-2 text-ink-muted border border-subtle hover:text-ink cursor-pointer"
      >
        {t(`memoryItems.kind.${item.kind}`, item.kind)}
      </button>

      {!editing && (
        <div className="shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
          <button
            onClick={startEdit}
            title={t('memoryItems.edit')}
            className="p-1 text-ink-faint hover:text-ink"
          >
            <FiEdit2 className="w-4 h-4" />
          </button>
          <button
            onClick={() => {
              if (window.confirm(t('memoryItems.confirmDelete'))) onDelete(item.id)
            }}
            title={t('memoryItems.delete')}
            className="p-1 text-ink-faint hover:text-danger-strong"
          >
            <FiTrash2 className="w-4 h-4" />
          </button>
        </div>
      )}
    </div>
  )
}
