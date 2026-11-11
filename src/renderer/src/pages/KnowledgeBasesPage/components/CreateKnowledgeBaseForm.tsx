import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { KbInfo } from '@/types/knowledge'
import {
  currentEmbeddingSpec,
  embeddingLabel,
  useKnowledgeSettings
} from '@renderer/components/KnowledgeBase/knowledgeSettings'
import { useKnowledgeBaseList } from '@renderer/contexts/KnowledgeBasesContext'

const inputClass =
  'bg-surface border border-strong text-ink text-sm rounded-control focus:ring-accent focus:border-accent block w-full px-2 py-1.5'

/** Name and describe a new knowledgebase. Names are unique; a taken one shows the error. */
export const CreateKnowledgeBaseForm: React.FC<{
  onCreated: (kb: KbInfo) => void
  onCancel: () => void
}> = ({ onCreated, onCancel }) => {
  const { t } = useTranslation()
  const { create } = useKnowledgeBaseList()
  const [settings] = useKnowledgeSettings()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    if (!name.trim() || busy) return
    setBusy(true)
    const kb = await create(name.trim(), description.trim())
    setBusy(false)
    if (kb) onCreated(kb)
  }
  const onEnter = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      submit()
    }
  }

  return (
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
        {t('knowledgeBase.embeddingModel', {
          model: embeddingLabel(currentEmbeddingSpec(settings))
        })}
        . {t('knowledgeBase.embeddingFromSettings')}
      </p>
      <div className="flex gap-2">
        <button
          type="button"
          className="text-xs px-2 py-1 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent disabled:opacity-50"
          disabled={!name.trim() || busy}
          onClick={submit}
        >
          {t('knowledgeBase.createButton')}
        </button>
        <button
          type="button"
          className="text-xs px-2 py-1 rounded-control border border-subtle text-ink hover:bg-raised"
          onClick={onCancel}
        >
          {t('knowledgeBase.cancel')}
        </button>
      </div>
    </div>
  )
}
