/**
 * `@` mention typeahead for the chat textarea.
 *
 * Mentioning an agent permits the current agent to delegate to it for that turn
 * (see `utils/agentMentions.ts`); this hook is only the typing convenience.
 */

import { useCallback, useMemo, useState } from 'react'
import type { CustomAgent } from '@/types/agent-chat'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { getMentionableAgents } from '../../utils/agentMentions'
import { getCaretCoordinates, type CaretCoordinates } from './caretCoordinates'

/**
 * `@` plus up to 40 non-@ characters, at the caret, following a boundary.
 * The boundary keeps an email address like `foo@bar` from opening the popup.
 */
const TRIGGER_PATTERN = /(?:^|[\s([{"'>])@([^\s@]{0,40})$/

/** The popup scrolls, so this only guards against an absurdly long agent list. */
export const MAX_MENTION_SUGGESTIONS = 50

type MentionState = {
  /** Index of the `@` in the textarea value. */
  startIndex: number
  query: string
  coordinates: CaretCoordinates
}

export function useAgentMention(
  textareaRef: React.RefObject<HTMLTextAreaElement>,
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

  const items = useMemo(() => {
    if (!state) return []
    const query = state.query.toLowerCase()

    return candidates
      .filter((agent) => {
        if (!query) return true
        const nameMatch = agent.name.toLowerCase().includes(query)
        const descMatch = agent.description?.toLowerCase().includes(query) || false
        const tagMatch = agent.tags?.some((tag) => tag.toLowerCase().includes(query)) || false
        return nameMatch || descMatch || tagMatch
      })
      .sort((a, b) => {
        // Prefix matches first, so typing "@Rev" surfaces "Reviewer" over "Code Review Helper"
        const aPrefix = a.name.toLowerCase().startsWith(query) ? 0 : 1
        const bPrefix = b.name.toLowerCase().startsWith(query) ? 0 : 1
        return aPrefix - bPrefix || a.name.localeCompare(b.name)
      })
      .slice(0, MAX_MENTION_SUGGESTIONS)
  }, [candidates, state])

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

  /** Replace the `@query` fragment with `@AgentName `. */
  const select = useCallback(
    (agent: CustomAgent) => {
      const textarea = textareaRef.current
      if (!textarea || !state) return

      const caret = textarea.selectionStart ?? value.length
      const insertion = `@${agent.name} `
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
    const agent = items[activeIndex]
    if (agent) select(agent)
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
