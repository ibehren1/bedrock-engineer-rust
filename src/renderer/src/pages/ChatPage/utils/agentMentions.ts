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

/**
 * Find the agents @mentioned in a message, in first-mention order.
 *
 * Candidate names are matched longest-first so that `@Software Architect` wins
 * over a hypothetical `@Software`, and so that names containing spaces work.
 */
export function parseAgentMentions(
  text: string,
  agents: CustomAgent[],
  selfAgentId?: string
): CustomAgent[] {
  if (!text || !text.includes('@')) return []

  const candidates = getMentionableAgents(agents, selfAgentId).sort(
    (a, b) => b.name.length - a.name.length
  )

  const matches: Array<{ index: number; agent: CustomAgent }> = []
  const claimedAgentIds = new Set<string>()
  // Character spans already consumed by a longer name, so that `@Writer Pro`
  // resolves to "Writer Pro" only and does not also count as "Writer".
  const claimedSpans: Array<[number, number]> = []

  const overlapsClaimed = (start: number, end: number) =>
    claimedSpans.some(([from, to]) => start < to && end > from)

  for (const agent of candidates) {
    if (!agent.name) continue

    // (?![\w-]) keeps `@Review` from matching the text `@Reviewer`
    const pattern = new RegExp(`${MENTION_BOUNDARY}@${escapeRegExp(agent.name)}(?![\\w-])`, 'gi')

    let match: RegExpExecArray | null
    while ((match = pattern.exec(text)) !== null) {
      // The boundary character is part of the match unless the mention starts the string
      const start = match.index + match[0].indexOf('@')
      const end = match.index + match[0].length

      if (overlapsClaimed(start, end)) continue

      claimedSpans.push([start, end])

      if (!claimedAgentIds.has(agent.id!)) {
        claimedAgentIds.add(agent.id!)
        matches.push({ index: start, agent })
      }
    }
  }

  return matches.sort((a, b) => a.index - b.index).map((m) => m.agent)
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
