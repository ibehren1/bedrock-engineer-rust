import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiChevronsLeft, FiChevronsRight } from 'react-icons/fi'
import { TbMessagePlus } from 'react-icons/tb'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import { ChatHistory } from '../ChatHistory'

/** Recent chats shown as badges on the collapsed rail. */
const RAIL_CHATS = 12

/** Up to two letters for a chat's rail badge: the initials of its first two words. */
export const chatInitials = (title: string): string => {
  const words = title.trim().split(/\s+/).filter(Boolean)
  if (words.length === 0) return '?'
  const letters =
    words.length === 1 ? [...words[0]].slice(0, 2) : [[...words[0]][0], [...words[1]][0]]
  return letters.join('').toUpperCase()
}

type HistorySidebarProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  currentSessionId?: string
  onSessionSelect: (sessionId: string) => void
  onNewChat: () => void
}

/**
 * The chat history, docked at the left of the chat page. Collapsed, it is a narrow rail with
 * New chat, expand, and the most recent chats as initials.
 */
export const HistorySidebar: React.FC<HistorySidebarProps> = ({
  open,
  onOpenChange,
  currentSessionId,
  onSessionSelect,
  onNewChat
}) => {
  const { t } = useTranslation()
  const { sessions } = useChatHistory()

  const railButton =
    'size-8 grid place-items-center rounded-control text-ink-muted hover:bg-raised hover:text-ink'

  if (open) {
    return (
      <aside className="w-96 shrink-0 bg-canvas border-r border-subtle overflow-y-auto overflow-x-hidden">
        <ChatHistory
          onSessionSelect={onSessionSelect}
          currentSessionId={currentSessionId}
          headerAction={
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              title={t('Hide chat history')}
              aria-label={t('Hide chat history')}
              className="h-8 w-8 grid place-items-center rounded-control text-ink-faint hover:text-ink hover:bg-raised"
            >
              <FiChevronsLeft className="size-4" />
            </button>
          }
        />
      </aside>
    )
  }

  const recent = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, RAIL_CHATS)

  return (
    <aside className="w-12 shrink-0 bg-canvas border-r border-subtle flex flex-col items-center gap-1 py-3 overflow-y-auto overflow-x-hidden">
      <button
        type="button"
        onClick={() => onOpenChange(true)}
        title={t('Show chat history')}
        aria-label={t('Show chat history')}
        className={railButton}
      >
        <FiChevronsRight className="size-4" />
      </button>
      <button
        type="button"
        onClick={onNewChat}
        title={t('New chat')}
        aria-label={t('New chat')}
        className={`${railButton} text-success hover:text-success-strong`}
      >
        <TbMessagePlus className="size-4" />
      </button>
      <div className="w-6 h-px bg-raised my-1" />
      {recent.map((session) => {
        const current = session.id === currentSessionId
        return (
          <button
            key={session.id}
            type="button"
            onClick={() => onSessionSelect(session.id)}
            title={session.title}
            aria-label={session.title}
            aria-current={current || undefined}
            className={`size-7 shrink-0 grid place-items-center rounded-full text-[10px] font-semibold ${
              current
                ? 'bg-accent text-accent-fg'
                : 'bg-raised text-ink-muted hover:bg-sunken hover:text-ink'
            }`}
          >
            {chatInitials(session.title)}
          </button>
        )
      })}
    </aside>
  )
}
