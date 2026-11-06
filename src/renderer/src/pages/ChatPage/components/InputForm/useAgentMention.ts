/**
 * `@` mention typeahead for the chat textarea.
 *
 * Mentioning an agent permits the current agent to delegate to it for that turn
 * (see `utils/agentMentions.ts`); mentioning a knowledge base points the knowledge base
 * tools at it (`utils/knowledgeMentions.ts`). This hook is only the typing convenience.
 */

import { useCallback, useMemo, useState } from 'react'
import type { CustomAgent } from '@/types/agent-chat'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { getMentionableAgents } from '../../utils/agentMentions'
import {
  getMentionableKnowledgeBases,
  type MentionableKnowledgeBase
} from '../../utils/knowledgeMentions'
import { getCaretCoordinates, type CaretCoordinates } from './caretCoordinates'

/**
 * `@` plus up to 40 non-@ characters, at the caret, following a boundary.
 * The boundary keeps an email address like `foo@bar` from opening the popup.
 */
const TRIGGER_PATTERN = /(?:^|[\s([{"'>])@([^\s@]{0,40})$/

/** A row of the popup. */
export type MentionItem =
  { kind: 'agent'; agent: CustomAgent } | { kind: 'kb'; kb: MentionableKnowledgeBase }

export const mentionItemKey = (item: MentionItem): string =>
  item.kind === 'agent' ? `agent:${item.agent.id}` : `kb:${item.kb.agentId}:${item.kb.kbId}`

/** The text a row inserts after the `@`. */
const mentionText = (item: MentionItem): string =>
  item.kind === 'agent' ? item.agent.name : item.kb.mention

/** Prefix matches first, so typing "@Rev" surfaces "Reviewer" over "Code Review Helper". */
function rank<T>(list: T[], query: string, name: (item: T) => string): T[] {
  return [...list].sort((a, b) => {
    const aPrefix = name(a).toLowerCase().startsWith(query) ? 0 : 1
    const bPrefix = name(b).toLowerCase().startsWith(query) ? 0 : 1
    return aPrefix - bPrefix || name(a).localeCompare(name(b))
  })
}

/** The popup scrolls, so this only guards against an absurdly long list. */
export const MAX_MENTION_SUGGESTIONS = 50

type MentionState = {
  /** Index of the `@` in the textarea value. */
  startIndex: number
  query: string
  coordinates: CaretCoordinates
}

export function useAgentMention(
  textareaRef: React.RefObject<HTMLTextAreaElement | null>,
  value: string,
  onChange: (value: string) => void
) {
  const { agents, selectedAgentId } = useSettings()
  const [state, setState] = useState<MentionState | null>(null)
  const [activeIndex, setActiveIndex] = useState(0)

  const candidates = useMemo(
    () => getMentionableAgents(agents, selectedAgentId),
    [agents, selectedAgentId]
  )
  const knowledgeBases = useMemo(
    () => getMentionableKnowledgeBases(agents, selectedAgentId),
    [agents, selectedAgentId]
  )

  // Agents first, then knowledge bases (the current agent's own ones first).
  const items = useMemo((): MentionItem[] => {
    if (!state) return []
    const query = state.query.toLowerCase()
    const has = (text?: string) => !!text && text.toLowerCase().includes(query)

    const agentItems = rank(
      candidates.filter(
        (agent) =>
          !query ||
          has(agent.name) ||
          has(agent.description) ||
          !!agent.tags?.some((tag) => has(tag))
      ),
      query,
      (agent) => agent.name
    ).map((agent): MentionItem => ({ kind: 'agent', agent }))
    const kbItems = rank(
      knowledgeBases.filter(
        (kb) => !query || has(kb.mention) || has(kb.description) || has(kb.agentName)
      ),
      query,
      (kb) => kb.kbName
    )
      .sort((a, b) => Number(b.isOwn) - Number(a.isOwn))
      .map((kb): MentionItem => ({ kind: 'kb', kb }))

    return [...agentItems, ...kbItems].slice(0, MAX_MENTION_SUGGESTIONS)
  }, [candidates, knowledgeBases, state])

  const open = !!state && items.length > 0

  const close = useCallback(() => {
    setState(null)
    setActiveIndex(0)
  }, [])

  /** Recompute the trigger from the text before the caret. */
  const refresh = useCallback(() => {
    const textarea = textareaRef.current
    if (!textarea) return close()

    const caret = textarea.selectionStart ?? 0
    const match = TRIGGER_PATTERN.exec(textarea.value.substring(0, caret))

    if (!match) return close()

    const query = match[1] ?? ''
    const startIndex = caret - query.length - 1

    setState((prev) => {
      // Reset the highlight only when the query actually changed
      if (!prev || prev.query !== query || prev.startIndex !== startIndex) {
        setActiveIndex(0)
      }
      return {
        startIndex,
        query,
        coordinates: getCaretCoordinates(textarea, startIndex)
      }
    })
  }, [close, textareaRef])

  const moveActive = useCallback(
    (delta: number) => {
      if (items.length === 0) return
      setActiveIndex((prev) => (prev + delta + items.length) % items.length)
    },
    [items.length]
  )

  /** Replace the `@query` fragment with `@Name `. */
  const select = useCallback(
    (item: MentionItem) => {
      const textarea = textareaRef.current
      if (!textarea || !state) return

      const caret = textarea.selectionStart ?? value.length
      const insertion = `@${mentionText(item)} `
      const next = value.substring(0, state.startIndex) + insertion + value.substring(caret)
      const nextCaret = state.startIndex + insertion.length

      onChange(next)
      close()

      // The value lands via React state, so wait a frame before moving the caret
      requestAnimationFrame(() => {
        const el = textareaRef.current
        if (!el) return
        el.focus()
        el.setSelectionRange(nextCaret, nextCaret)
      })
    },
    [close, onChange, state, textareaRef, value]
  )

  const selectActive = useCallback(() => {
    const item = items[activeIndex]
    if (item) select(item)
  }, [activeIndex, items, select])

  return {
    open,
    items,
    activeIndex,
    query: state?.query ?? '',
    coordinates: state?.coordinates,
    refresh,
    close,
    moveActive,
    select,
    selectActive
  }
}
