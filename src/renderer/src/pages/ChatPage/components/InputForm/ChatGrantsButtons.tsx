import React, { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { TbRobot } from 'react-icons/tb'
import { FiDatabase } from 'react-icons/fi'
import type { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import { OnOffToggle } from '@renderer/components/OnOffToggle'
import { getMentionableAgents } from '../../utils/agentMentions'
import { getMentionableKnowledgeBases, kbGrantKey } from '../../utils/knowledgeMentions'
import type { GrantTarget } from '../../utils/mentionGrants'
import type { MentionGrantsControl } from '../../hooks/useMentionGrants'

export type ChatGrantsButtonsProps = {
  agents: CustomAgent[]
  /** The chat's own agent (not listed). */
  agentId?: string
  control: MentionGrantsControl
  /** The chat's agent has `invokeAgent`, so a granted agent can actually be handed work. */
  delegationAvailable: boolean
}

type Row = {
  key: string
  target: GrantTarget
  label: string
  detail?: string
  title?: string
  icon: React.ReactNode
  on: boolean
}

type GrantsMenuProps = {
  icon: React.ReactNode
  title: string
  help: string
  warning?: string
  rows: Row[]
  onToggle: (target: GrantTarget, on: boolean) => void
}

/** One input-bar button with a badge of what is on, opening a list of on / off switches. */
const GrantsMenu: React.FC<GrantsMenuProps> = ({ icon, title, help, warning, rows, onToggle }) => {
  const { t } = useTranslation()
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!isOpen) return
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setIsOpen(false)
    }
    document.addEventListener('mousedown', handleClickOutside)
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleKey)
    }
  }, [isOpen])

  // What is on first. Sorted when the list opens, so a row doesn't jump while it's switched.
  const [order, setOrder] = useState<string[]>([])
  const open = () => {
    setOrder([...rows].sort((a, b) => Number(b.on) - Number(a.on)).map((row) => row.key))
    setIsOpen(true)
  }
  const sorted = useMemo(() => {
    const rank = (key: string) => {
      const i = order.indexOf(key)
      return i === -1 ? order.length : i
    }
    return [...rows].sort((a, b) => rank(a.key) - rank(b.key))
  }, [rows, order])
  const onCount = rows.filter((row) => row.on).length

  return (
    <div className="relative" ref={containerRef}>
      <button
        type="button"
        onClick={() => (isOpen ? setIsOpen(false) : open())}
        aria-expanded={isOpen}
        aria-label={title}
        title={t('chatGrants.buttonTooltip', { title, count: onCount })}
        className={`relative p-2 rounded-full transition-colors duration-200 ${
          onCount > 0 ? 'bg-surface text-accent' : 'text-ink-muted hover:text-ink hover:bg-surface'
        }`}
      >
        {icon}
        {onCount > 0 && (
          <span className="absolute -top-0.5 -right-0.5 min-w-4 h-4 px-1 rounded-full bg-accent text-accent-fg text-[10px] leading-4 text-center">
            {onCount}
          </span>
        )}
      </button>

      {isOpen && (
        <div className="absolute right-0 z-20 w-80 bottom-full mb-1 bg-surface rounded-container shadow-lg border border-subtle">
          <div className="px-3 py-2 border-b border-subtle">
            <span className="text-sm font-medium text-ink">{title}</span>
            <p className="mt-1 text-xs text-ink-muted">{help}</p>
            {warning && <p className="mt-1 text-xs text-warning">{warning}</p>}
          </div>
          <div className="max-h-80 overflow-y-auto py-1">
            {sorted.map((row) => (
              <div
                key={row.key}
                className="flex items-center gap-2 px-3 py-1.5 text-sm"
                title={row.title}
              >
                <span className="shrink-0 text-ink-muted">{row.icon}</span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-ink">{row.label}</span>
                  {row.detail && (
                    <span className="block truncate text-xs text-ink-muted">{row.detail}</span>
                  )}
                </span>
                <OnOffToggle
                  sizing="md"
                  checked={row.on}
                  onChange={(on) => onToggle(row.target, on)}
                  aria-label={row.label}
                />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

/**
 * The chat's agents and knowledge bases buttons, next to the memory toggle. Each lists what
 * may be turned on for the chat, with what is on first; the same switches an `@Name` or a
 * `disable @Name` in a message flips. A button only appears when there is something to list.
 */
export const ChatGrantsButtons: React.FC<ChatGrantsButtonsProps> = ({
  agents,
  agentId,
  control,
  delegationAvailable
}) => {
  const { t } = useTranslation()
  const { resolved, setGranted } = control

  const agentRows: Row[] = useMemo(() => {
    const on = new Set(resolved.agents.map((a) => a.id))
    return getMentionableAgents(agents, agentId).map((agent) => ({
      key: `agent:${agent.id}`,
      target: { kind: 'agent', agentId: agent.id! },
      label: agent.name,
      title: agent.description || agent.name,
      icon: (
        <AgentIconView
          icon={agent.icon}
          iconColor={agent.iconColor}
          className="w-4 h-4"
          fallback={<TbRobot className="w-4 h-4" />}
        />
      ),
      on: on.has(agent.id)
    }))
  }, [agents, agentId, resolved.agents])

  const kbRows: Row[] = useMemo(() => {
    const on = new Set(resolved.knowledgeBases.map(kbGrantKey))
    return getMentionableKnowledgeBases(agents, agentId).map((kb) => ({
      key: kbGrantKey(kb),
      target: { kind: 'kb', agentId: kb.agentId, kbId: kb.kbId },
      label: kb.kbName,
      detail: kb.isOwn ? t('textarea.mention.thisAgent') : kb.agentName,
      title: [`@${kb.mention}`, kb.description].filter(Boolean).join('\n'),
      icon: <FiDatabase className="w-4 h-4" />,
      on: on.has(kbGrantKey(kb))
    }))
  }, [agents, agentId, resolved.knowledgeBases, t])

  return (
    <>
      {agentRows.length > 0 && (
        <GrantsMenu
          icon={<TbRobot />}
          title={t('chatGrants.agents.title')}
          help={t('chatGrants.agents.help')}
          warning={delegationAvailable ? undefined : t('chatGrants.agents.noInvokeAgent')}
          rows={agentRows}
          onToggle={setGranted}
        />
      )}
      {kbRows.length > 0 && (
        <GrantsMenu
          icon={<FiDatabase />}
          title={t('chatGrants.knowledgeBases.title')}
          help={t('chatGrants.knowledgeBases.help')}
          rows={kbRows}
          onToggle={setGranted}
        />
      )}
    </>
  )
}
