import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiDatabase, FiPlus, FiSettings } from 'react-icons/fi'
import {
  currentEmbeddingSpec,
  formatBytes,
  useKnowledgeSettings
} from '@renderer/components/KnowledgeBase/knowledgeSettings'
import { agentsUsing } from '@renderer/contexts/knowledgeRefs'
import useSetting from '@renderer/hooks/useSetting'
import { cn } from '@renderer/components/ui'
import { KnowledgeBaseCard } from './components/KnowledgeBaseCard'
import { CreateKnowledgeBaseForm } from './components/CreateKnowledgeBaseForm'
import { KnowledgeBaseSettings } from './components/KnowledgeBaseSettings'
import { UsedByPanel } from './components/UsedByPanel'
import { useKnowledgeBases } from './useKnowledgeBases'
import {
  ChatTabStrip,
  type ChatTabItem
} from '@renderer/pages/ChatPage/components/ChatTabs/ChatTabStrip'

type View = 'list' | 'settings'

/**
 * Knowledge Bases: every local knowledgebase in one place. Create and fill them here, pick the
 * agents that use each one, and set how all of them are embedded, chunked, searched and synced.
 */
export const KnowledgeBasesPage: React.FC = () => {
  const { t } = useTranslation()
  const kb = useKnowledgeBases()
  const { customAgents } = useSetting()
  const [settings, setSettings] = useKnowledgeSettings()
  const spec = currentEmbeddingSpec(settings)
  const [view, setView] = useState<View>('list')
  const [creating, setCreating] = useState(false)
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // The selection falls back to the first knowledgebase (a deleted one drops out).
  const selected = kb.kbs.find((k) => k.id === selectedId) ?? kb.kbs[0]

  const tabs: ChatTabItem<View>[] = [
    {
      key: 'list',
      label: t('knowledgeBase.page.listTab'),
      icon: <FiDatabase className="w-3.5 h-3.5" />
    },
    {
      key: 'settings',
      label: t('knowledgeBase.page.settingsTab'),
      icon: <FiSettings className="w-3.5 h-3.5" />
    }
  ]

  return (
    <div className="flex flex-col h-full px-2.5 py-1.5 gap-4 overflow-y-auto">
      <header className="flex flex-col gap-1">
        <div className="flex items-center gap-4 flex-wrap">
          <h1 className="text-title text-ink">{t('knowledgeBase.page.title')}</h1>
          {kb.kbs.length > 0 && (
            <span className="text-sm text-ink-muted">
              {t('knowledgeBase.page.count', { count: kb.kbs.length })}
            </span>
          )}
        </div>
        <p className="text-ink-muted">{t('knowledgeBase.page.description')}</p>
      </header>

      <ChatTabStrip items={tabs} active={view} onChange={setView} />

      {view === 'settings' ? (
        <div className="max-w-[1024px]">
          <KnowledgeBaseSettings settings={settings} setSettings={setSettings} />
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[16rem_minmax(0,1fr)] items-start">
          <aside className="space-y-2">
            {!creating && (
              <button
                type="button"
                className="w-full inline-flex items-center justify-center gap-1 text-xs px-2 py-1.5 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent"
                onClick={() => setCreating(true)}
              >
                <FiPlus className="w-3 h-3" />
                {t('knowledgeBase.create')}
              </button>
            )}
            {creating && (
              <CreateKnowledgeBaseForm
                onCreated={(created) => {
                  setCreating(false)
                  setSelectedId(created.id)
                }}
                onCancel={() => setCreating(false)}
              />
            )}
            {!kb.loading && kb.kbs.length === 0 && !creating && (
              <p className="text-sm text-ink-muted p-2.5 border border-subtle rounded-control text-center">
                {t('knowledgeBase.empty')}
              </p>
            )}
            {kb.kbs.length > 0 && (
              <ul className="border border-subtle rounded-container divide-y divide-subtle bg-surface">
                {kb.kbs.map((info) => {
                  const users = agentsUsing(customAgents, info.id).length
                  const active = selected?.id === info.id
                  return (
                    <li key={info.id}>
                      <button
                        type="button"
                        aria-current={active || undefined}
                        className={cn(
                          'w-full text-left px-2.5 py-2 flex items-start gap-2',
                          active ? 'bg-accent-tint' : 'hover:bg-surface-2'
                        )}
                        onClick={() => setSelectedId(info.id)}
                      >
                        <FiDatabase className="w-4 h-4 mt-0.5 shrink-0 text-ink-muted" />
                        <span className="min-w-0">
                          <span className="block text-sm text-ink truncate">{info.name}</span>
                          <span className="block text-xs text-ink-muted">
                            {users > 0
                              ? t('knowledgeBase.page.agentCount', { count: users })
                              : t('knowledgeBase.settings.notAttached')}{' '}
                            · {formatBytes(info.bytes)}
                          </span>
                        </span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            )}
          </aside>

          {selected && (
            <div className="space-y-3 min-w-0">
              <KnowledgeBaseCard
                key={selected.id}
                kb={selected}
                sources={kb.sources[selected.id] ?? []}
                jobs={kb.jobs}
                actions={kb}
                currentSpec={spec}
                crawlerDefaults={settings.crawler}
                defaultOpen
              />
              <UsedByPanel kbId={selected.id} />
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export default KnowledgeBasesPage
