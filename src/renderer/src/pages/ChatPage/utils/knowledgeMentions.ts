/**
 * @mentions of local knowledgebases.
 *
 * Every knowledgebase on the Knowledge Bases page can be mentioned. A mention turns it on for
 * the rest of the chat (`mentionGrants.ts`): while any are on, the knowledgebase tools search
 * only them, through `knowledge_search_granted`, with the grants taken from the user's text or
 * the chat's grant buttons (never the model's tool input).
 */

import type { CustomAgent, ToolState } from '@/types/agent-chat'
import type { KnowledgeBaseGrant } from '@/types/knowledge'
import { KNOWLEDGE_BASE_TOOL_DEFINITIONS } from '@common/agents/knowledgeTools'
import {
  agentMentionCandidates,
  getMentionableAgents,
  matchMentions,
  type MentionCandidate
} from './agentMentions'

/** A knowledgebase as the mention code needs it (a `KbInfo` or an agent's ref both fit). */
export type KnowledgeBaseListItem = { id: string; name: string; description?: string }

/** A knowledgebase that may be @mentioned. */
export type MentionableKnowledgeBase = {
  kbId: string
  kbName: string
  description?: string
  /** The agent doing the mentioning has it attached. */
  attached: boolean
  /** The text after the `@`: the name, or `kb/Name` when an agent has the same name. */
  mention: string
}

/** The prefix that tells a knowledgebase from an agent with the same name. */
export const KB_MENTION_PREFIX = 'kb/'

/**
 * Every knowledgebase, the ones attached to the current agent first. A knowledgebase is
 * mentioned by its name, or by `kb/Name` when a mentionable agent has the same name. Names are
 * unique; should two still match (case-insensitively), the later ones are left out.
 */
export function getMentionableKnowledgeBases(
  kbs: KnowledgeBaseListItem[],
  agents: CustomAgent[],
  selfAgentId?: string
): MentionableKnowledgeBase[] {
  const agentNames = new Set(
    getMentionableAgents(agents, selfAgentId).map((a) => a.name.trim().toLowerCase())
  )
  const self = selfAgentId ? agents.find((a) => a.id === selfAgentId) : undefined
  const attachedIds = new Set((self?.localKnowledgeBases ?? []).map((kb) => kb.id))
  const seen = new Set<string>()
  const list: MentionableKnowledgeBase[] = []
  for (const kb of kbs) {
    const kbName = kb.name.trim()
    const key = kbName.toLowerCase()
    if (!kb.id || !kbName || seen.has(key)) continue
    seen.add(key)
    list.push({
      kbId: kb.id,
      kbName,
      description: kb.description,
      attached: attachedIds.has(kb.id),
      mention: agentNames.has(key) ? `${KB_MENTION_PREFIX}${kbName}` : kbName
    })
  }
  return list.sort((a, b) => Number(b.attached) - Number(a.attached))
}

export type ParsedMentions = {
  agents: CustomAgent[]
  knowledgeBases: MentionableKnowledgeBase[]
}

/** An @mention target: an agent or a knowledgebase. */
export type MentionItem =
  { kind: 'agent'; agent: CustomAgent } | { kind: 'kb'; kb: MentionableKnowledgeBase }

/** Agents and knowledgebases as mention candidates, matched together. */
export function mentionCandidates(
  agents: CustomAgent[],
  kbs: KnowledgeBaseListItem[],
  selfAgentId?: string
): MentionCandidate<MentionItem>[] {
  return [
    ...agentMentionCandidates(agents, selfAgentId).map((c) => ({
      ...c,
      value: { kind: 'agent' as const, agent: c.value }
    })),
    ...getMentionableKnowledgeBases(kbs, agents, selfAgentId).map((kb) => ({
      mention: kb.mention,
      key: kbGrantKey(kb),
      value: { kind: 'kb' as const, kb }
    }))
  ]
}

/**
 * The mention key of a knowledgebase, also used to compare grants. Only the id counts: grants
 * saved by older builds also carry the agent that owned the knowledgebase.
 */
export const kbGrantKey = (kb: { kbId: string }): string => `kb:${kb.kbId}`

/**
 * The agents and knowledgebases @mentioned in a message, each in first-mention order. Both
 * are matched in one pass, so `@kb/Writer` is the knowledgebase only.
 */
export function parseMentions(
  text: string,
  agents: CustomAgent[],
  kbs: KnowledgeBaseListItem[],
  selfAgentId?: string
): ParsedMentions {
  const found = matchMentions(text, mentionCandidates(agents, kbs, selfAgentId))
  return {
    agents: found.flatMap((f) => (f.kind === 'agent' ? [f.agent] : [])),
    knowledgeBases: found.flatMap((f) => (f.kind === 'kb' ? [f.kb] : []))
  }
}

export const toGrants = (kbs: MentionableKnowledgeBase[]): KnowledgeBaseGrant[] =>
  kbs.map((kb) => ({ kbId: kb.kbId }))

const SEARCH_TOOL = 'searchKnowledgeBase'
const LIST_TOOL = 'listKnowledgeBases'
const SOURCES_TOOL = 'listKnowledgeBaseSources'

/** Whether a tool is one of the knowledgebase tools a mention reroutes. */
export const isMentionedKnowledgeBaseTool = (name: string): boolean =>
  name === SEARCH_TOOL || name === LIST_TOOL || name === SOURCES_TOOL

/**
 * `tools` with the knowledgebase tools set up for the mentioned knowledgebases: present even
 * when the agent has none of its own, and with `knowledgeBase` limited to the mentioned names.
 */
export function withMentionedKnowledgeBaseTools(
  tools: ToolState[],
  kbs: MentionableKnowledgeBase[]
): ToolState[] {
  if (kbs.length === 0) return tools
  const names = kbs.map((kb) => kb.kbName)
  const specs: ToolState[] = KNOWLEDGE_BASE_TOOL_DEFINITIONS.map((t) => {
    const schema = JSON.parse(JSON.stringify(t.inputSchema)) as {
      properties: Record<string, Record<string, unknown>>
    }
    if (schema.properties.knowledgeBase) {
      schema.properties.knowledgeBase = {
        type: 'string',
        enum: [...new Set(names)],
        description: 'Only this one of the knowledge bases the user turned on. Omit for all.'
      }
    }
    return {
      enabled: true,
      toolSpec: {
        name: t.name,
        description: t.description.replace(
          /this agent's local knowledge bases/i,
          'the local knowledge bases the user turned on for this chat'
        ),
        inputSchema: { json: schema }
      }
    } as ToolState
  })
  const replaced = new Set(specs.map((s) => s.toolSpec?.name))
  return [...tools.filter((t) => !replaced.has(t.toolSpec?.name)), ...specs]
}

/** The turn's extra system prompt naming the mentioned knowledgebases. */
export function buildMentionedKnowledgeBasesPrompt(kbs: MentionableKnowledgeBase[]): string {
  if (kbs.length === 0) return ''
  return [
    '<mentioned_knowledge_bases>',
    'The user turned on these local knowledge bases for this chat:',
    ...kbs.map((kb) => {
      const description = (kb.description ?? '').trim()
      return `- ${kb.kbName}${description ? `: ${description}` : ''}`
    }),
    'Search them with the searchKnowledgeBase tool before answering; the knowledge base tools reach only these. Cite inline: put a passage\'s "Cite as" link right after each sentence or bullet that uses a fact from it, not in a list at the end, and never state a fact from a knowledge base without its link.',
    '</mentioned_knowledge_bases>'
  ].join('\n')
}

/** `listKnowledgeBases` for a turn with mentions: the mentioned knowledgebases. */
export function listMentionedKnowledgeBases(kbs: MentionableKnowledgeBase[]): string {
  return kbs
    .map((kb) => {
      const description = (kb.description ?? '').trim()
      return `- ${kb.kbName} (id: ${kb.kbId})${description ? `. ${description}` : ''}`
    })
    .join('\n')
}

/** Run a knowledgebase tool over only the mentioned knowledgebases. */
export async function runMentionedKnowledgeBaseTool(
  name: string,
  input: Record<string, unknown>,
  kbs: MentionableKnowledgeBase[]
): Promise<string> {
  const grants = toGrants(kbs)
  const knowledgeBase = typeof input.knowledgeBase === 'string' ? input.knowledgeBase : undefined
  if (name === SEARCH_TOOL) {
    const topK = typeof input.topK === 'number' ? input.topK : undefined
    return window.api.knowledge.searchGranted(
      grants,
      String(input.query ?? ''),
      knowledgeBase,
      topK
    )
  }
  if (name === SOURCES_TOOL) return window.api.knowledge.listSourcesGranted(grants, knowledgeBase)
  return listMentionedKnowledgeBases(kbs)
}
