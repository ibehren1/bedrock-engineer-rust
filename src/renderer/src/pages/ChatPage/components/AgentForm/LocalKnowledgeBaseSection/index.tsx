import React from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { FiDatabase, FiExternalLink } from 'react-icons/fi'
import type { LocalKnowledgeBaseRef } from '@/types/agent-chat'
import { useKnowledgeBaseList } from '@renderer/contexts/KnowledgeBasesContext'
import { setKnowledgeRef } from '@renderer/contexts/knowledgeRefs'
import { formatBytes } from '@renderer/components/KnowledgeBase/knowledgeSettings'

/**
 * The agent form's Knowledge Base tab: which of the local knowledgebases (managed on the
 * Knowledge Bases page) this agent uses. The choice is form state, saved with the agent.
 */
export const LocalKnowledgeBaseSection: React.FC<{
  isShared?: boolean
  value?: LocalKnowledgeBaseRef[]
  onChange: (refs: LocalKnowledgeBaseRef[]) => void
}> = ({ isShared, value, onChange }) => {
  const { t } = useTranslation()
  const { knowledgeBases, loading } = useKnowledgeBaseList()

  if (isShared) {
    return (
      <div className="border border-subtle rounded-control p-3 text-sm text-ink-muted">
        {t('knowledgeBase.sharedUnavailable')}
      </div>
    )
  }

  const attached = new Set((value ?? []).map((ref) => ref.id))

  return (
    <div className="space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-ink flex items-center gap-1.5">
            <FiDatabase className="w-4 h-4" />
            {t('knowledgeBase.title')}
          </h3>
          <p className="text-xs text-ink-muted mt-0.5">{t('knowledgeBase.pickerDescription')}</p>
        </div>
        <Link
          to="/knowledge-bases"
          className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-control border border-subtle text-ink hover:bg-raised whitespace-nowrap"
          title={t('knowledgeBase.manageHint')}
        >
          <FiExternalLink className="w-3 h-3" />
          {t('knowledgeBase.manage')}
        </Link>
      </div>

      {!loading && knowledgeBases.length === 0 && (
        <div className="text-center p-2.5 border border-subtle rounded-control">
          <p className="text-sm text-ink-muted">{t('knowledgeBase.pickerEmpty')}</p>
        </div>
      )}

      {knowledgeBases.length > 0 && (
        <ul className="border border-subtle rounded-container divide-y divide-subtle">
          {knowledgeBases.map((kb) => {
            const inputId = `agent-kb-${kb.id}`
            return (
              <li key={kb.id} className="flex items-start gap-2 px-3 py-2">
                <input
                  type="checkbox"
                  id={inputId}
                  checked={attached.has(kb.id)}
                  onChange={(e) => onChange(setKnowledgeRef(value, kb, e.target.checked))}
                  className="mt-0.5 h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
                />
                <label htmlFor={inputId} className="flex-1 min-w-0 cursor-pointer">
                  <span className="block text-sm text-ink truncate">{kb.name}</span>
                  {kb.description && (
                    <span className="block text-xs text-ink-muted">{kb.description}</span>
                  )}
                  <span className="block text-xs text-ink-muted">
                    {t('knowledgeBase.passages', { count: kb.chunkCount })} ·{' '}
                    {formatBytes(kb.bytes)}
                  </span>
                </label>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
