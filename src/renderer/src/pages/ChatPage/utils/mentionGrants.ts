/**
 * The agents and knowledgebases on for a chat ("grants").
 *
 * An @mention turns its agent or knowledgebase on for the rest of the chat; writing
 * `disable @Name` turns it off again, and so do the chat's grant buttons. The grants are saved
 * with the session (`mentionGrants`). Chats saved before that existed derive them once from
 * their messages.
 *
 * An agent that is on may be delegated to (`invokeAgent`); while any knowledgebase is on, the
 * knowledgebase tools search only those.
 */

import type { CustomAgent } from '@/types/agent-chat'
import type { MentionGrants } from '@/types/chat/history'
import { findMentionSpans, getMentionableAgents } from './agentMentions'
import {
  getMentionableKnowledgeBases,
  kbGrantKey,
  mentionCandidates,
  type KnowledgeBaseListItem,
  type MentionableKnowledgeBase,
  type MentionItem
} from './knowledgeMentions'

export const EMPTY_GRANTS: MentionGrants = { agentIds: [], knowledgeBases: [] }

/** `disable` right before a mention (`disable @Writer`) turns it off. */
const DISABLE_PREFIX = /(?:^|[^\w-])disable\s+$/i

/** What a grant names: an agent id, or a knowledgebase id. */
export type GrantTarget = { kind: 'agent'; agentId: string } | { kind: 'kb'; kbId: string }

export const grantTargetOf = (item: MentionItem): GrantTarget =>
  item.kind === 'agent'
    ? { kind: 'agent', agentId: item.agent.id! }
    : { kind: 'kb', kbId: item.kb.kbId }

export function isGranted(grants: MentionGrants, target: GrantTarget): boolean {
  return target.kind === 'agent'
    ? grants.agentIds.includes(target.agentId)
    : grants.knowledgeBases.some((kb) => kbGrantKey(kb) === kbGrantKey(target))
}

/** `grants` with `target` turned on (appended, so the list keeps the order things were turned on) or off. */
export function setGrant(grants: MentionGrants, target: GrantTarget, on: boolean): MentionGrants {
  if (isGranted(grants, target) === on) return grants
  if (target.kind === 'agent') {
    return {
      ...grants,
      agentIds: on
        ? [...grants.agentIds, target.agentId]
        : grants.agentIds.filter((id) => id !== target.agentId)
    }
  }
  const key = kbGrantKey(target)
  return {
    ...grants,
    knowledgeBases: on
      ? [...grants.knowledgeBases, { kbId: target.kbId }]
      : grants.knowledgeBases.filter((kb) => kbGrantKey(kb) !== key)
  }
}

export type PromptGrantResult = {
  grants: MentionGrants
  /** What the prompt turned off with `disable @Name` (and didn't turn back on later in it). */
  disabled: MentionItem[]
}

/** Apply one prompt's mentions: each `@Name` turns it on, each `disable @Name` off. */
export function applyPromptToGrants(
  grants: MentionGrants,
  prompt: string,
  agents: CustomAgent[],
  kbs: KnowledgeBaseListItem[],
  selfAgentId?: string
): PromptGrantResult {
  let next = grants
  const disabled = new Map<string, MentionItem>()
  for (const span of findMentionSpans(prompt, mentionCandidates(agents, kbs, selfAgentId))) {
    const on = !DISABLE_PREFIX.test(prompt.slice(0, span.index))
    next = setGrant(next, grantTargetOf(span.value), on)
    if (on) disabled.delete(span.key)
    else disabled.set(span.key, span.value)
  }
  return { grants: next, disabled: [...disabled.values()] }
}

/** The grants a run of prompts (oldest first) leaves, for chats saved without `mentionGrants`. */
export function deriveGrants(
  prompts: string[],
  agents: CustomAgent[],
  kbs: KnowledgeBaseListItem[],
  selfAgentId?: string
): MentionGrants {
  return prompts.reduce(
    (grants, prompt) => applyPromptToGrants(grants, prompt, agents, kbs, selfAgentId).grants,
    EMPTY_GRANTS
  )
}

export type ResolvedGrants = {
  agents: CustomAgent[]
  knowledgeBases: MentionableKnowledgeBase[]
}

/**
 * The granted agents and knowledgebases that still exist and may be mentioned from
 * `selfAgentId`, in the order they were turned on. A deleted agent or knowledgebase, or the
 * chat's own agent, simply drops out. Knowledgebase grants match by id alone, so grants saved
 * by older builds (which also name the owning agent) keep working.
 */
export function resolveGrants(
  grants: MentionGrants,
  agents: CustomAgent[],
  knowledgeBases: KnowledgeBaseListItem[],
  selfAgentId?: string
): ResolvedGrants {
  const mentionable = getMentionableAgents(agents, selfAgentId)
  const kbs = getMentionableKnowledgeBases(knowledgeBases, agents, selfAgentId)
  return {
    agents: grants.agentIds.flatMap((id) => mentionable.filter((a) => a.id === id)),
    knowledgeBases: grants.knowledgeBases.flatMap((grant) =>
      kbs.filter((kb) => kbGrantKey(kb) === kbGrantKey(grant))
    )
  }
}
