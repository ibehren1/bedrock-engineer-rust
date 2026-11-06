/**
 * @mention parsing for agent-to-agent delegation.
 *
 * Mentions are derived from the submitted text rather than tracked as separate
 * state: the text is the only thing the user sees and the only thing persisted,
 * so structured state would desync on backspace, mid-token edits, paste, or a
 * resend from history. The @ popup is purely a typing convenience.
 */

import type { Tool } from '@aws-sdk/client-bedrock-runtime'
import type { CustomAgent, ToolState } from '@/types/agent-chat'
import { buildInvokeAgentToolSpec, type DelegationTarget } from '@common/agents/delegation'
import { EXCLUDED_CHAT_AGENT_IDS } from '../components/AgentList/useAgentFilter'

export const INVOKE_AGENT_TOOL_NAME = 'invokeAgent'

/** Characters a mention may follow. Prevents matching inside e.g. an email address. */
const MENTION_BOUNDARY = '(?:^|[\\s(\\[{"\'>])'

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * Agents that may be @mentioned: everything selectable in chat, minus the
 * agent doing the mentioning.
 */
export function getMentionableAgents(agents: CustomAgent[], selfAgentId?: string): CustomAgent[] {
  return agents.filter(
    (agent) => !!agent.id && !EXCLUDED_CHAT_AGENT_IDS.includes(agent.id) && agent.id !== selfAgentId
  )
}

/** Something that can be @mentioned: the text after the `@`, a unique key and the item. */
export type MentionCandidate<T> = { mention: string; key: string; value: T }

/**
 * The candidates @mentioned in `text`, in first-mention order, each once.
 *
 * Names are matched longest-first so that `@Software Architect` wins over a hypothetical
 * `@Software`, and so that names containing spaces work. A span taken by a longer name is not
 * matched again, so `@Writer Pro` is only "Writer Pro" and `@Agent/Notes` is not also `@Agent`.
 */
export function matchMentions<T>(text: string, candidates: MentionCandidate<T>[]): T[] {
  if (!text || !text.includes('@')) return []

  const sorted = candidates
    .filter((c) => !!c.mention)
    .sort((a, b) => b.mention.length - a.mention.length)

  const matches: Array<{ index: number; value: T }> = []
  const claimedKeys = new Set<string>()
  const claimedSpans: Array<[number, number]> = []

  const overlapsClaimed = (start: number, end: number) =>
    claimedSpans.some(([from, to]) => start < to && end > from)

  for (const candidate of sorted) {
    // (?![\w-]) keeps `@Review` from matching the text `@Reviewer`
    const pattern = new RegExp(
      `${MENTION_BOUNDARY}@${escapeRegExp(candidate.mention)}(?![\\w-])`,
      'gi'
    )

    let match: RegExpExecArray | null
    while ((match = pattern.exec(text)) !== null) {
      // The boundary character is part of the match unless the mention starts the string
      const start = match.index + match[0].indexOf('@')
      const end = match.index + match[0].length

      if (overlapsClaimed(start, end)) continue

      claimedSpans.push([start, end])

      if (!claimedKeys.has(candidate.key)) {
        claimedKeys.add(candidate.key)
        matches.push({ index: start, value: candidate.value })
      }
    }
  }

  return matches.sort((a, b) => a.index - b.index).map((m) => m.value)
}

/** Agents as mention candidates. */
export const agentMentionCandidates = (
  agents: CustomAgent[],
  selfAgentId?: string
): MentionCandidate<CustomAgent>[] =>
  getMentionableAgents(agents, selfAgentId).map((agent) => ({
    mention: agent.name,
    key: `agent:${agent.id}`,
    value: agent
  }))

/** Find the agents @mentioned in a message, in first-mention order. */
export function parseAgentMentions(
  text: string,
  agents: CustomAgent[],
  selfAgentId?: string
): CustomAgent[] {
  return matchMentions(text, agentMentionCandidates(agents, selfAgentId))
}

/**
 * Constrain the `invokeAgent` tool to the agents mentioned in this turn.
 *
 * Returns the input array unchanged when the tool is not enabled, and drops the
 * tool entirely when nothing was mentioned — an empty enum is rejected by some
 * models and invites hallucinated ids.
 */
export function applyDelegationAllowlist(tools: ToolState[], allowed: CustomAgent[]): ToolState[] {
  const index = tools.findIndex((tool) => tool.toolSpec?.name === INVOKE_AGENT_TOOL_NAME)
  if (index === -1) return tools

  if (allowed.length === 0) {
    return tools.filter((_tool, i) => i !== index)
  }

  const targets: DelegationTarget[] = allowed.map((agent) => ({
    id: agent.id!,
    name: agent.name,
    description: agent.description
  }))

  const spec = buildInvokeAgentToolSpec(tools[index].toolSpec as Tool['toolSpec'], targets)
  if (!spec) {
    return tools.filter((_tool, i) => i !== index)
  }

  const next = [...tools]
  next[index] = { enabled: tools[index].enabled, toolSpec: spec }
  return next
}

export function hasInvokeAgentTool(tools: ToolState[]): boolean {
  return tools.some((tool) => tool.toolSpec?.name === INVOKE_AGENT_TOOL_NAME)
}
