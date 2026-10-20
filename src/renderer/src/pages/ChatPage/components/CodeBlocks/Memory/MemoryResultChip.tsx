import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import toast from 'react-hot-toast'
import { FiCheck } from 'react-icons/fi'
import type { MemoryItem } from '@/types/bridge/memory'
import { useOptionalMemoryItems } from '@renderer/contexts/MemoryContext'

export const MEMORY_TOOL_NAMES = ['addMemory', 'updateMemory', 'removeMemory'] as const
type MemoryToolName = (typeof MEMORY_TOOL_NAMES)[number]

const MAX_PREVIEW = 80

/** The memory tool result shape: `{ name, success, message, result: { item } }`. */
export const getMemoryToolResult = (
  json: unknown
): { name: MemoryToolName; item: MemoryItem } | undefined => {
  if (!json || typeof json !== 'object') return undefined
  const { name, success, result } = json as Record<string, any>
  if (!MEMORY_TOOL_NAMES.includes(name)) return undefined
  if (success === false) return undefined
  const item = result?.item
  if (!item || typeof item.id !== 'string' || typeof item.content !== 'string') return undefined
  return { name, item }
}

const preview = (text: string) => {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length > MAX_PREVIEW ? `${flat.slice(0, MAX_PREVIEW - 1)}…` : flat
}

/**
 * Compact chip under a memory tool result: what was saved / updated / removed, a link to the
 * Memory page and, for add and remove, an Undo.
 */
export const MemoryResultChip: React.FC<{ name: MemoryToolName; item: MemoryItem }> = ({
  name,
  item
}) => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const memory = useOptionalMemoryItems()
  const [undone, setUndone] = useState(false)
  const [busy, setBusy] = useState(false)

  const label =
    name === 'addMemory'
      ? t('memoryItems.chip.saved')
      : name === 'updateMemory'
        ? t('memoryItems.chip.updated')
        : t('memoryItems.chip.removed')

  // Undo only while it still means something: the added item is still there, or the removed one
  // hasn't come back (re-adding gives it a new id, so compare the text too).
  const items = memory?.items ?? []
  const canUndo =
    !!memory &&
    !undone &&
    (name === 'addMemory'
      ? items.some((i) => i.id === item.id)
      : name === 'removeMemory'
        ? !items.some((i) => i.id === item.id || i.content === item.content)
        : false)

  const handleUndo = async () => {
    if (!memory || busy) return
    setBusy(true)
    try {
      if (name === 'addMemory') {
        await memory.remove(item.id)
      } else {
        await memory.add({ content: item.content, kind: item.kind, source: 'chat' })
      }
      setUndone(true)
    } catch (error) {
      toast.error(
        t('memoryItems.error', { error: error instanceof Error ? error.message : String(error) })
      )
    } finally {
      setBusy(false)
    }
  }

  const linkClass = 'underline underline-offset-2 hover:text-success-strong disabled:opacity-50'

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5 self-start max-w-full text-xs text-success bg-success/10 border border-success/40 rounded-full px-2 py-0.5">
      <FiCheck className="w-3 h-3 shrink-0" />
      <span className="min-w-0 break-words">
        {label}: &quot;{preview(item.content)}&quot;
      </span>
      <span aria-hidden>·</span>
      <button type="button" className={linkClass} onClick={() => navigate('/memory')}>
        {t('memoryItems.chip.view')}
      </button>
      {undone ? (
        <>
          <span aria-hidden>·</span>
          <span>{t('memoryItems.chip.undone')}</span>
        </>
      ) : (
        canUndo && (
          <>
            <span aria-hidden>·</span>
            <button type="button" className={linkClass} disabled={busy} onClick={handleUndo}>
              {t('memoryItems.chip.undo')}
            </button>
          </>
        )
      )}
    </span>
  )
}
