import React, { useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiDatabase, FiPlus } from 'react-icons/fi'
import type { LocalKnowledgeBaseRef } from '@/types/agent-chat'
import useSetting from '@renderer/hooks/useSetting'
import {
  currentEmbeddingSpec,
  embeddingLabel,
  useKnowledgeSettings
} from '@renderer/components/KnowledgeBase/knowledgeSettings'
import { KnowledgeBaseCard } from './KnowledgeBaseCard'
import { useLocalKnowledgeBases } from './useLocalKnowledgeBases'

const inputClass =
  'bg-surface border border-strong text-ink text-sm rounded-control focus:ring-accent focus:border-accent block w-full px-2 py-1.5'

/**
 * The agent form's Knowledge Base tab: the agent's local knowledgebases, their sources and a
 * search tester. Changes happen right away (they are files on disk, not form state); the agent's
 * `localKnowledgeBases` list follows them, in the form and, for a saved agent, in the store.
 */
export const LocalKnowledgeBaseSection: React.FC<{
  agentId: string
  isShared?: boolean
  value?: LocalKnowledgeBaseRef[]
  onChange: (refs: LocalKnowledgeBaseRef[]) => void
}> = ({ agentId, isShared, value, onChange }) => {
  const { t } = useTranslation()
  const { customAgents, saveCustomAgents } = useSetting()
  const [settings] = useKnowledgeSettings()
  const spec = currentEmbeddingSpec(settings)

  // A saved agent gets the new list right away, so the chat sees new knowledgebases without
  // the form being saved (and a cancelled edit doesn't leave a stale list).
  const handleRefs = useCallback(
    (refs: LocalKnowledgeBaseRef[]) => {
      onChange(refs)
      if (customAgents.some((a) => a.id === agentId)) {
        saveCustomAgents(
          customAgents.map((a) => (a.id === agentId ? { ...a, localKnowledgeBases: refs } : a))
        )
      }
    },
    [agentId, customAgents, onChange, saveCustomAgents]
  )

  const kb = useLocalKnowledgeBases(agentId, value, handleRefs)
  const [creating, setCreating] = useState(false)
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')

  if (isShared) {
    return (
      <div className="border border-subtle rounded-control p-3 text-sm text-ink-muted">
        {t('knowledgeBase.sharedUnavailable')}
      </div>
    )
  }

  const create = async () => {
    if (!name.trim()) return
    const created = await kb.create(name.trim(), description.trim())
    if (created) {
      setCreating(false)
      setName('')
      setDescription('')
    }
  }
  const onEnter = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      create()
    }
  }

  return (
    <div className="space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-ink flex items-center gap-1.5">
            <FiDatabase className="w-4 h-4" />
            {t('knowledgeBase.title')}
          </h3>
          <p className="text-xs text-ink-muted mt-0.5">{t('knowledgeBase.description')}</p>
        </div>
        {!creating && (
          <button
            type="button"
            className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent whitespace-nowrap"
            onClick={() => setCreating(true)}
          >
            <FiPlus className="w-3 h-3" />
            {t('knowledgeBase.create')}
          </button>
        )}
      </div>

      {creating && (
        <div className="border border-accent rounded-container p-2.5 space-y-2 bg-surface">
          <h4 className="text-sm font-medium text-ink">{t('knowledgeBase.createTitle')}</h4>
          <label className="block text-xs text-ink">
            {t('knowledgeBase.name')}
            <input
              className={`${inputClass} mt-0.5`}
              placeholder={t('knowledgeBase.namePlaceholder')}
              value={name}
              autoFocus
              onChange={(e) => setName(e.target.value)}
              onKeyDown={onEnter}
            />
          </label>
          <label className="block text-xs text-ink">
            {t('knowledgeBase.descriptionLabel')}
            <input
              className={`${inputClass} mt-0.5`}
              placeholder={t('knowledgeBase.descriptionPlaceholder')}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onKeyDown={onEnter}
            />
          </label>
          <p className="text-xs text-ink-muted">
            {t('knowledgeBase.embeddingModel', { model: embeddingLabel(spec) })}.{' '}
            {t('knowledgeBase.embeddingFromSettings')}
          </p>
          <div className="flex gap-2">
            <button
              type="button"
              className="text-xs px-2 py-1 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent disabled:opacity-50"
              disabled={!name.trim()}
              onClick={create}
            >
              {t('knowledgeBase.createButton')}
            </button>
            <button
              type="button"
              className="text-xs px-2 py-1 rounded-control border border-subtle text-ink hover:bg-raised"
              onClick={() => setCreating(false)}
            >
              {t('knowledgeBase.cancel')}
            </button>
          </div>
        </div>
      )}

      {!kb.loading && kb.kbs.length === 0 && !creating && (
        <div className="text-center p-2.5 border border-subtle rounded-control">
          <p className="text-sm text-ink-muted">{t('knowledgeBase.empty')}</p>
        </div>
      )}

      <div className="space-y-2">
        {kb.kbs.map((info) => (
          <KnowledgeBaseCard
            key={info.id}
            kb={info}
            sources={kb.sources[info.id] ?? []}
            jobs={kb.jobs}
            actions={kb}
            currentSpec={spec}
            crawlerDefaults={settings.crawler}
          />
        ))}
      </div>
    </div>
  )
}
