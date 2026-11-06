/**
 * @mentions of local knowledgebases.
 *
 * Like agent mentions (`agentMentions.ts`), they are read from the submitted text, not kept as
 * state. Any agent's knowledgebase can be mentioned. A mention points the model at those
 * knowledgebases for that turn: the knowledgebase tools search only them, through
 * `knowledge_search_granted`, with the grants taken from the user's text (never the model's
 * tool input).
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

/** A knowledgebase that may be @mentioned. */
export type MentionableKnowledgeBase = {
  agentId: string
  agentName: string
  kbId: string
  kbName: string
  description?: string
  /** It belongs to the agent doing the mentioning. */
  isOwn: boolean
  /** The text after the `@`: the name, or `Agent/Name` when the name alone is ambiguous. */
  mention: string
}

/**
 * Every agent's local knowledgebases, own ones first. A knowledgebase is mentioned by its
 * name; when another knowledgebase or a mentionable agent has the same name, by
 * `Agent Name/Knowledgebase Name` (the current agent's own one keeps the bare name unless an
 * agent has it).
 */
export function getMentionableKnowledgeBases(
  agents: CustomAgent[],
  selfAgentId?: string
): MentionableKnowledgeBase[] {
  const agentNames = new Set(
    getMentionableAgents(agents, selfAgentId).map((a) => a.name.trim().toLowerCase())
  )
  const all = agents.flatMap((agent) =>
    agent.id
      ? (agent.localKnowledgeBases ?? [])
          .filter((kb) => kb.id && kb.name.trim())
          .map((kb) => ({
            agentId: agent.id!,
            agentName: agent.name,
            kbId: kb.id,
            kbName: kb.name.trim(),
            description: kb.description,
            isOwn: agent.id === selfAgentId
          }))
      : []
  )
  const count = new Map<string, number>()
  for (const kb of all) {
    const key = kb.kbName.toLowerCase()
    count.set(key, (count.get(key) ?? 0) + 1)
  }
  return all
    .map((kb) => {
      const key = kb.kbName.toLowerCase()
      const clashesWithAgent = agentNames.has(key)
      const shared = (count.get(key) ?? 0) > 1
      const bare = !clashesWithAgent && (!shared || kb.isOwn)
      // Two own knowledgebases with one name: the first keeps it, the rest are qualified.
      return { ...kb, mention: bare ? kb.kbName : `${kb.agentName}/${kb.kbName}` }
    })
    .map((kb, i, list) =>
      list.findIndex((other) => other.mention.toLowerCase() === kb.mention.toLowerCase()) < i
        ? { ...kb, mention: `${kb.agentName}/${kb.kbName}` }
        : kb
    )
    .sort((a, b) => Number(b.isOwn) - Number(a.isOwn))
}

export type ParsedMentions = {
  agents: CustomAgent[]
  knowledgeBases: MentionableKnowledgeBase[]
}

/**
 * The agents and knowledgebases @mentioned in a message, each in first-mention order. Both
 * are matched in one pass, so `@Agent/Notes` is the knowledgebase only.
 */
export function parseMentions(
  text: string,
  agents: CustomAgent[],
  selfAgentId?: string
): ParsedMentions {
  type Item = { kind: 'agent'; agent: CustomAgent } | { kind: 'kb'; kb: MentionableKnowledgeBase }
  const candidates: MentionCandidate<Item>[] = [
    ...agentMentionCandidates(agents, selfAgentId).map((c) => ({
      ...c,
      value: { kind: 'agent' as const, agent: c.value }
    })),
    ...getMentionableKnowledgeBases(agents, selfAgentId).map((kb) => ({
      mention: kb.mention,
      key: `kb:${kb.agentId}:${kb.kbId}`,
      value: { kind: 'kb' as const, kb }
    }))
  ]
  const found = matchMentions(text, candidates)
  return {
    agents: found.flatMap((f) => (f.kind === 'agent' ? [f.agent] : [])),
    knowledgeBases: found.flatMap((f) => (f.kind === 'kb' ? [f.kb] : []))
  }
}

export const toGrants = (kbs: MentionableKnowledgeBase[]): KnowledgeBaseGrant[] =>
  kbs.map((kb) => ({ agentId: kb.agentId, kbId: kb.kbId }))

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
        description: 'Only this one of the knowledge bases the user mentioned. Omit for all.'
      }
    }
    return {
      enabled: true,
      toolSpec: {
        name: t.name,
        description: t.description.replace(
          /this agent's local knowledge bases/i,
          'the local knowledge bases the user mentioned in this message'
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
    'The user pointed you at these local knowledge bases for this message:',
    ...kbs.map((kb) => {
      const owner = kb.isOwn ? '' : ` (from the agent "${kb.agentName}")`
      const description = (kb.description ?? '').trim()
      return `- ${kb.kbName}${owner}${description ? `: ${description}` : ''}`
    }),
    'Search them with the searchKnowledgeBase tool before answering; for this message the knowledge base tools reach only these. Cite inline: put a passage\'s "Cite as" link right after each sentence or bullet that uses a fact from it, not in a list at the end, and never state a fact from a knowledge base without its link.',
    '</mentioned_knowledge_bases>'
  ].join('\n')
}

/** `listKnowledgeBases` for a turn with mentions: the mentioned knowledgebases. */
export function listMentionedKnowledgeBases(kbs: MentionableKnowledgeBase[]): string {
  return kbs
    .map((kb) => {
      const owner = kb.isOwn ? '' : ` (agent "${kb.agentName}")`
      const description = (kb.description ?? '').trim()
      return `- ${kb.kbName} (id: ${kb.kbId})${owner}${description ? `. ${description}` : ''}`
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
