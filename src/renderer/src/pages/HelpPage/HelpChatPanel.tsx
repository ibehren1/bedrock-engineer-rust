import React, { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiPlus, FiSend, FiSquare, FiX } from 'react-icons/fi'
import type { GuideHeading } from '@renderer/components/Markdown/slug'
import MD from '@renderer/components/Markdown/MD'
import { Button } from '@renderer/components/ui'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import { useLightProcessingModel } from '@renderer/lib/modelSelection'
import { useAgentChat } from '@renderer/pages/ChatPage/hooks/useAgentChat'
import {
  HELP_AGENT_ID,
  HELP_AGENT_SYSTEM_PROMPT,
  HELP_CHAT_TITLE,
  LEGACY_GUIDE_ATTACHMENT
} from '@renderer/pages/ChatPage/constants/helpAgent'
import { makeHelpPrepareTurn } from './helpChat'
import { headingSlugForPath } from './helpPageUtils'

type Props = {
  headings: GuideHeading[]
  /** Scroll the guide to a section (answers link their sources). */
  onOpenSection: (slug: string) => void
  onClose: () => void
  /** Bedrock isn't set up: show why the chat can't answer yet. */
  bedrockMissing: boolean
}

/**
 * The Help chat, docked beside the guide. It is the same "Bedrock Engineer Help" session the
 * chat history lists; this panel finds the newest one (or starts one) and talks to it.
 */
export const HelpChatPanel: React.FC<Props> = (props) => {
  const { t } = useTranslation()
  const { sessions, createSession, updateSessionTitle } = useChatHistory()
  const { getLightModelId } = useLightProcessingModel()
  const [sessionId, setSessionId] = useState<string>()
  const starting = useRef(false)

  const start = async (fresh: boolean) => {
    if (starting.current) return
    starting.current = true
    try {
      const previous = fresh
        ? undefined
        : sessions
            .filter((s) => s.agentId === HELP_AGENT_ID)
            .sort((a, b) => b.updatedAt - a.updatedAt)[0]
      if (previous) {
        // Help chats from before the guide index carried the whole guide as an attachment,
        // which would still go out with every message.
        await window.api.chatAttachments
          .remove(previous.id, LEGACY_GUIDE_ATTACHMENT)
          .catch(() => undefined)
        setSessionId(previous.id)
      } else {
        const id = await createSession(HELP_AGENT_ID, getLightModelId(), HELP_AGENT_SYSTEM_PROMPT)
        // A title that isn't "Chat …" is never auto-renamed.
        await updateSessionTitle(id, HELP_CHAT_TITLE)
        setSessionId(id)
      }
    } finally {
      starting.current = false
    }
  }

  // Once, when the panel opens.
  useEffect(() => {
    void start(false)
  }, [])

  return (
    <aside className="flex flex-col w-[26rem] max-w-[45%] shrink-0 border-l border-subtle pl-3 min-h-0">
      <div className="flex items-center gap-2 pb-2">
        <h2 className="text-heading font-medium text-ink flex-1">{t('help.chat.title')}</h2>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void start(true)}
          title={t('help.chat.new')}
        >
          <FiPlus aria-hidden />
          {t('help.chat.new')}
        </Button>
        <Button size="sm" variant="ghost" onClick={props.onClose} aria-label={t('help.chat.close')}>
          <FiX aria-hidden />
        </Button>
      </div>
      {props.bedrockMissing && (
        <p className="text-xs text-warning pb-2">{t('help.page.needsBedrock')}</p>
      )}
      {sessionId ? (
        <HelpConversation key={sessionId} sessionId={sessionId} {...props} />
      ) : (
        <div className="flex-1" />
      )}
    </aside>
  )
}

const HelpConversation: React.FC<Props & { sessionId: string }> = ({
  sessionId,
  headings,
  onOpenSection
}) => {
  const { t } = useTranslation()
  const { getLightModelId } = useLightProcessingModel()
  const [input, setInput] = useState('')
  const scroller = useRef<HTMLDivElement>(null)
  // Follow the conversation while the reader is at (or near) the bottom; scrolling up to
  // reread stops following until they scroll back down or send another question.
  const following = useRef(true)

  const prepareTurn = useMemo(
    () =>
      makeHelpPrepareTurn({
        anchorFor: (path) => headingSlugForPath(headings, path),
        onFallback: () => toast(t('help.excerptsFailed'))
      }),
    [headings, t]
  )

  const { messages, loading, waitingForResponse, handleSubmit, stopGeneration } = useAgentChat(
    getLightModelId(),
    HELP_AGENT_SYSTEM_PROMPT,
    HELP_AGENT_ID,
    sessionId,
    { prepareTurn }
  )

  useEffect(() => {
    const box = scroller.current
    // Only the chat's own box scrolls (scrollIntoView would also move the guide).
    if (box && following.current) box.scrollTop = box.scrollHeight
    // `messages` changes with every streamed chunk, so the answer is followed as it grows.
  }, [messages, waitingForResponse])

  const send = () => {
    const text = input.trim()
    if (!text || loading) return
    following.current = true
    setInput('')
    void handleSubmit(text)
  }

  const textOf = (content: unknown[] | undefined) =>
    (content ?? [])
      .map((c: any) => c?.text ?? c?.guardContent?.text?.text ?? '')
      .filter(Boolean)
      .join('\n\n')

  return (
    <>
      <div
        ref={scroller}
        onScroll={(e) => {
          const box = e.currentTarget
          following.current = box.scrollHeight - box.scrollTop - box.clientHeight < 48
        }}
        className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-3 pr-1 text-sm"
      >
        {messages.length === 0 && <p className="text-ink-muted text-xs">{t('help.chat.empty')}</p>}
        {messages.map((m, i) => {
          // The message still streaming has no id yet.
          const key = m.id ?? `streaming-${i}`
          const text = textOf(m.content)
          if (!text) return null
          return m.role === 'user' ? (
            <div
              key={key}
              className="self-end max-w-[85%] rounded-container bg-accent-tint px-3 py-2 text-ink whitespace-pre-wrap"
            >
              {text}
            </div>
          ) : (
            <div key={key} className="max-w-full text-ink">
              <MD onAnchorClick={onOpenSection}>{text}</MD>
            </div>
          )
        })}
        {waitingForResponse && <p className="text-xs text-ink-muted">{t('help.chat.thinking')}</p>}
      </div>
      <form
        className="flex items-end gap-2 pt-2"
        onSubmit={(e) => {
          e.preventDefault()
          send()
        }}
      >
        <textarea
          aria-label={t('help.chat.placeholder')}
          placeholder={t('help.chat.placeholder')}
          rows={2}
          className="flex-1 resize-none rounded-control border border-subtle bg-surface px-2 py-1 text-sm text-ink"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault()
              send()
            }
          }}
        />
        {loading ? (
          <Button size="sm" onClick={() => stopGeneration()} aria-label={t('help.chat.stop')}>
            <FiSquare aria-hidden />
          </Button>
        ) : (
          <Button
            size="sm"
            variant="primary"
            type="submit"
            disabled={!input.trim()}
            aria-label={t('help.chat.send')}
          >
            <FiSend aria-hidden />
          </Button>
        )}
      </form>
    </>
  )
}
