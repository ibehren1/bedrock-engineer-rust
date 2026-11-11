import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { Message } from '@aws-sdk/client-bedrock-runtime'
import type { CustomAgent } from '@/types/agent-chat'
import type { MentionGrants } from '@/types/chat/history'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import {
  applyPromptToGrants,
  deriveGrants,
  EMPTY_GRANTS,
  resolveGrants,
  setGrant,
  type GrantTarget,
  type ResolvedGrants
} from '../utils/mentionGrants'
import type { KnowledgeBaseListItem, MentionItem } from '../utils/knowledgeMentions'
import { previousUserPrompts } from '../utils/userPrompts'

export type MentionGrantsControl = {
  /** The agents and knowledge bases on for this chat that still exist. */
  resolved: ResolvedGrants
  /** Turn an agent or knowledge base on or off for this chat (the grant buttons). */
  setGranted: (target: GrantTarget, on: boolean) => void
}

/**
 * The agents and knowledge bases on for the chat in `sessionId`, loaded from the session and
 * saved back to it. `applyPrompt` folds a new prompt's mentions in (at send time).
 */
export function useMentionGrants(
  sessionId: string | undefined,
  agents: CustomAgent[],
  knowledgeBases: KnowledgeBaseListItem[],
  agentId: string | undefined
) {
  const { getSession, getSessionField, setMentionGrants } = useChatHistory()
  const [grants, setGrantsState] = useState<MentionGrants>(EMPTY_GRANTS)
  // The latest grants, so two changes in one tick (a toggle, then a send) build on each other
  const grantsRef = useRef<MentionGrants>(EMPTY_GRANTS)

  useEffect(() => {
    // The saved grants are one small field; only chats from before them need the messages.
    const saved = sessionId ? getSessionField(sessionId, 'mentionGrants') : undefined
    const session = sessionId && !saved ? getSession(sessionId) : null
    const loaded =
      saved ??
      (session
        ? deriveGrants(
            previousUserPrompts(session.messages as Message[]),
            agents,
            knowledgeBases,
            agentId
          )
        : EMPTY_GRANTS)
    grantsRef.current = loaded
    setGrantsState(loaded)
    // Only on a session switch: agent edits mustn't reload (and lose) unsaved state
  }, [sessionId, getSession, getSessionField])

  const commit = useCallback(
    (next: MentionGrants) => {
      if (next === grantsRef.current) return
      grantsRef.current = next
      setGrantsState(next)
      if (sessionId) {
        setMentionGrants(sessionId, next).catch((error) =>
          console.error('Failed to save the chat’s agents and knowledge bases:', error)
        )
      }
    },
    [sessionId, setMentionGrants]
  )

  const setGranted = useCallback(
    (target: GrantTarget, on: boolean) => commit(setGrant(grantsRef.current, target, on)),
    [commit]
  )

  /** Fold `prompt`'s mentions in; returns the grants for its turn and what it disabled. */
  const applyPrompt = useCallback(
    (prompt: string): { resolved: ResolvedGrants; disabled: MentionItem[] } => {
      const result = applyPromptToGrants(grantsRef.current, prompt, agents, knowledgeBases, agentId)
      commit(result.grants)
      return {
        resolved: resolveGrants(result.grants, agents, knowledgeBases, agentId),
        disabled: result.disabled
      }
    },
    [agents, knowledgeBases, agentId, commit]
  )

  const resolved = useMemo(
    () => resolveGrants(grants, agents, knowledgeBases, agentId),
    [grants, agents, knowledgeBases, agentId]
  )

  const control: MentionGrantsControl = useMemo(
    () => ({ resolved, setGranted }),
    [resolved, setGranted]
  )

  return { control, applyPrompt }
}
