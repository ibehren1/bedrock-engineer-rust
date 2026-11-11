/**
 * Agents' `localKnowledgeBases` against the central knowledgebase list.
 *
 * An agent attaches a knowledgebase by listing its id; the name and description it keeps are only
 * a display cache for its system prompt. These helpers keep that cache true to the list.
 */

import type { CustomAgent, LocalKnowledgeBaseRef } from '@/types/agent-chat'
import type { KbInfo } from '@/types/knowledge'

type KbSummary = Pick<KbInfo, 'id' | 'name' | 'description'>

export const toKnowledgeBaseRef = (kb: KbSummary): LocalKnowledgeBaseRef => ({
  id: kb.id,
  name: kb.name,
  description: kb.description
})

const sameRefs = (a: LocalKnowledgeBaseRef[], b: LocalKnowledgeBaseRef[]): boolean =>
  a.length === b.length &&
  a.every(
    (ref, i) =>
      ref.id === b[i].id &&
      ref.name === b[i].name &&
      (ref.description ?? '') === (b[i].description ?? '')
  )

/**
 * `agents` with every knowledgebase ref checked against `kbs`: refs to knowledgebases that no
 * longer exist are dropped, the rest get the current name and description. `null` when nothing
 * changes, so callers write the store only when they must. `kbs` must come from a successful
 * load: an empty list from a failed one would detach everything.
 */
export function reconcileKnowledgeRefs(
  agents: CustomAgent[],
  kbs: KbSummary[]
): CustomAgent[] | null {
  const byId = new Map(kbs.map((kb) => [kb.id, kb]))
  let changed = false
  const next = agents.map((agent) => {
    const refs = agent.localKnowledgeBases
    if (!refs) return agent
    const seen = new Set<string>()
    const updated = refs.flatMap((ref) => {
      const kb = byId.get(ref.id)
      if (!kb || seen.has(ref.id)) return []
      seen.add(ref.id)
      return [toKnowledgeBaseRef(kb)]
    })
    if (sameRefs(refs, updated)) return agent
    changed = true
    return { ...agent, localKnowledgeBases: updated }
  })
  return changed ? next : null
}

/** `refs` with `kb` attached (appended) or detached; the same array when nothing changes. */
export function setKnowledgeRef(
  refs: LocalKnowledgeBaseRef[] | undefined,
  kb: KbSummary,
  on: boolean
): LocalKnowledgeBaseRef[] {
  const list = refs ?? []
  const has = list.some((ref) => ref.id === kb.id)
  if (has === on) return list
  return on ? [...list, toKnowledgeBaseRef(kb)] : list.filter((ref) => ref.id !== kb.id)
}

/** The custom agents that attach the knowledgebase `kbId`. */
export const agentsUsing = (agents: CustomAgent[], kbId: string): CustomAgent[] =>
  agents.filter((agent) => agent.localKnowledgeBases?.some((ref) => ref.id === kbId))
