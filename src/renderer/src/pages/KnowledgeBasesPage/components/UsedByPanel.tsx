import React from 'react'
import { useTranslation } from 'react-i18next'
import { TbRobot } from 'react-icons/tb'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import { useKnowledgeBaseList } from '@renderer/contexts/KnowledgeBasesContext'
import useSetting from '@renderer/hooks/useSetting'

/**
 * Which custom agents use a knowledgebase: one checkbox per agent, saved right away. An agent
 * that uses it searches it with the knowledge base tools in every chat.
 */
export const UsedByPanel: React.FC<{ kbId: string }> = ({ kbId }) => {
  const { t } = useTranslation()
  const { customAgents } = useSetting()
  const { setAttached } = useKnowledgeBaseList()
  const agents = customAgents.filter((agent) => agent.id && !agent.isShared)
  const count = agents.filter((agent) =>
    agent.localKnowledgeBases?.some((ref) => ref.id === kbId)
  ).length

  return (
    <section className="border border-subtle rounded-container bg-surface p-2.5 space-y-2">
      <div>
        <h3 className="text-sm font-medium text-ink">
          {t('knowledgeBase.page.usedBy', { count })}
        </h3>
        <p className="text-xs text-ink-muted mt-0.5">{t('knowledgeBase.page.usedByHint')}</p>
      </div>
      {agents.length === 0 ? (
        <p className="text-sm text-ink-muted">{t('knowledgeBase.page.noAgents')}</p>
      ) : (
        <ul className="grid gap-1 sm:grid-cols-2">
          {agents.map((agent) => {
            const inputId = `kb-${kbId}-agent-${agent.id}`
            const on = !!agent.localKnowledgeBases?.some((ref) => ref.id === kbId)
            return (
              <li key={agent.id} className="flex items-center gap-2 min-w-0">
                <input
                  type="checkbox"
                  id={inputId}
                  checked={on}
                  onChange={(e) => setAttached(agent.id!, kbId, e.target.checked)}
                  className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
                />
                <label
                  htmlFor={inputId}
                  className="flex items-center gap-1.5 text-sm text-ink min-w-0 cursor-pointer"
                  title={agent.description || agent.name}
                >
                  <AgentIconView
                    icon={agent.icon}
                    iconColor={agent.iconColor}
                    className="w-4 h-4 shrink-0"
                    fallback={<TbRobot className="w-4 h-4 shrink-0" />}
                  />
                  <span className="truncate">{agent.name}</span>
                </label>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
