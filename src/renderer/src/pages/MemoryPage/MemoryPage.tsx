import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiPlus } from 'react-icons/fi'
import { LuBrain } from 'react-icons/lu'
import type { MemoryItemKind, MemoryUpdateParams } from '@/types/bridge/memory'
import { useMemoryItems } from '@renderer/contexts/MemoryContext'
import { MemoryItemRow } from './components/MemoryItemRow'

const errorMessage = (error: unknown) => (error instanceof Error ? error.message : String(error))

/**
 * Memory: the global list of rules and facts. Chats with the memory toggle on get the enabled
 * ones in their system prompt (rules as binding instructions, facts as background); the model
 * edits the list with its memory tools.
 */
export const MemoryPage: React.FC = () => {
  const { t } = useTranslation()
  const { items, enabledCount, loading, add, update, remove } = useMemoryItems()
  const [content, setContent] = useState('')
  const [kind, setKind] = useState<MemoryItemKind>('rule')
  const [adding, setAdding] = useState(false)

  const reportError = (error: unknown) => {
    console.error('Memory item update failed:', error)
    toast.error(t('memoryItems.error', { error: errorMessage(error) }))
  }

  const handleAdd = async () => {
    const text = content.trim()
    if (!text || adding) return
    setAdding(true)
    try {
      await add({ content: text, kind, source: 'manual' })
      setContent('')
    } catch (error) {
      reportError(error)
    } finally {
      setAdding(false)
    }
  }

  const handleUpdate = async (params: MemoryUpdateParams) => {
    try {
      await update(params)
      return true
    } catch (error) {
      reportError(error)
      return false
    }
  }

  const handleDelete = async (id: string) => {
    try {
      await remove(id)
    } catch (error) {
      reportError(error)
    }
  }

  return (
    <div className="flex flex-col h-full px-2.5 py-1.5">
      <header className="mb-4">
        <div className="flex items-center gap-4">
          <h1 className="text-title text-ink">{t('memoryItems.title')}</h1>
          {items.length > 0 && (
            <span className="text-sm text-ink-muted">
              {t('memoryItems.count', { enabled: enabledCount, total: items.length })}
            </span>
          )}
        </div>
        <p className="text-ink-muted mt-1">
          {t('memoryItems.description')}{' '}
          <span className="text-ink-muted">{t('memoryItems.hint')}</span>
        </p>
        <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 items-baseline text-sm">
          {(['rule', 'memory'] as const).map((k) => (
            <React.Fragment key={k}>
              <dt className="text-micro rounded-control px-1.5 py-0.5 bg-surface-2 text-ink-muted border border-subtle justify-self-start">
                {t(`memoryItems.kind.${k}`)}
              </dt>
              <dd className="text-ink-muted">{t(`memoryItems.kindDescription.${k}`)}</dd>
            </React.Fragment>
          ))}
        </dl>
      </header>

      <form
        className="flex gap-2 mb-3"
        onSubmit={(e) => {
          e.preventDefault()
          handleAdd()
        }}
      >
        <input
          type="text"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={t('memoryItems.addPlaceholder')}
          className="flex-1 text-sm px-2.5 py-1.5 border border-strong rounded-control bg-surface text-ink placeholder:text-ink-faint focus:outline-none focus:ring-2 focus:ring-accent"
        />
        <select
          value={kind}
          onChange={(e) => setKind(e.target.value as MemoryItemKind)}
          aria-label={t('memoryItems.kindLabel')}
          className="text-sm border border-strong rounded-control px-2 py-1 bg-surface text-ink"
        >
          <option value="rule">{t('memoryItems.kindOption.rule')}</option>
          <option value="memory">{t('memoryItems.kindOption.memory')}</option>
        </select>
        <button
          type="submit"
          disabled={!content.trim() || adding}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-accent text-accent-fg rounded-control hover:bg-accent-strong transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <FiPlus className="w-4 h-4" />
          {t('memoryItems.add')}
        </button>
      </form>

      <div className="flex-1 min-h-0 overflow-y-auto">
        {loading && items.length === 0 ? (
          <p className="text-sm text-ink-muted p-2.5">{t('memoryItems.loading')}</p>
        ) : items.length === 0 ? (
          <div className="p-10 text-center">
            <LuBrain className="w-12 h-12 text-ink-faint mx-auto mb-3" />
            <p className="text-ink text-sm font-medium">{t('memoryItems.empty.title')}</p>
            <p className="text-ink-muted text-sm mt-1">{t('memoryItems.empty.description')}</p>
          </div>
        ) : (
          <div className="space-y-1">
            {items.map((item) => (
              <MemoryItemRow
                key={item.id}
                item={item}
                onUpdate={handleUpdate}
                onDelete={handleDelete}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
