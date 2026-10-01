/**
 * Shared policy for agent-to-agent delegation (the `invokeAgent` tool).
 *
 * Both the renderer (which builds the top-level turn's tool config) and the main
 * process (which builds a sub-agent's tool specs) must produce an identical
 * `agentId` enum and description, so that logic lives here.
 */

import type { Tool } from '@aws-sdk/client-bedrock-runtime'

/**
 * Maximum delegation depth. The top-level chat runs at depth 0, so a value of 2
 * permits chat -> agent A -> agent B and strips `invokeAgent` from B's tools.
 */
export const MAX_DELEGATION_DEPTH = 2

/** Cap on the sub-agent text handed back to the caller, in characters. */
export const MAX_DELEGATION_RESULT_CHARS = 60_000

export type DelegationTarget = {
  id: string
  name: string
  description?: string
}

export type DelegationContext = {
  /** Depth the *calling* agent runs at. 0 = top-level chat. */
  depth: number
  /** Agent ids from the root down to and including the calling agent. */
  lineage: string[]
  /** Ids the user permitted for this turn via @mention. */
  allowedAgentIds: string[]
}

/**
 * Narrow the turn allowlist to the agents a given caller may actually delegate
 * to: nothing already in its own ancestry (which would loop), and not itself.
 */
export function filterDelegationTargets(
  allowedAgentIds: string[],
  lineage: string[],
  selfAgentId?: string
): string[] {
  const seen = new Set<string>()
  return allowedAgentIds.filter((id) => {
    if (!id || seen.has(id)) return false
    if (lineage.includes(id)) return false
    if (selfAgentId && id === selfAgentId) return false
    seen.add(id)
    return true
  })
}

/** Whether an agent at `depth` is allowed to delegate at all. */
export function canDelegate(depth: number, remainingTargetCount: number): boolean {
  return depth < MAX_DELEGATION_DEPTH && remainingTargetCount > 0
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text
  return `${text.slice(0, max - 1).trimEnd()}…`
}

/**
 * Clone the static `invokeAgent` tool spec, constraining `agentId` to the
 * permitted ids and listing their names so the model can map the user's literal
 * `@Reviewer` onto the corresponding id.
 *
 * Returns null when nothing may be delegated to — callers must then drop the
 * tool entirely rather than ship an empty enum, which some models reject.
 */
export function buildInvokeAgentToolSpec(
  baseSpec: Tool['toolSpec'],
  allowed: DelegationTarget[]
): Tool['toolSpec'] | null {
  if (!baseSpec) return null
  if (allowed.length === 0) return null

  // Deep clone so the shared static spec is never mutated.
  const spec = JSON.parse(JSON.stringify(baseSpec)) as NonNullable<Tool['toolSpec']>
  const json = spec.inputSchema?.json as
    | { properties?: Record<string, any>; [key: string]: any }
    | undefined

  if (json?.properties?.agentId) {
    json.properties.agentId.enum = allowed.map((a) => a.id)
  }

  const table = allowed
    .map(
      (a) =>
        `- \`${a.id}\` — **${a.name}**${a.description ? ` — ${truncate(a.description, 300)}` : ''}`
    )
    .join('\n')

  spec.description = `${spec.description || ''}

## Agents you may delegate to in this request

${table}

The user permitted these agents by @mentioning them. You are not required to delegate — do the work yourself when that is simpler.`

  return spec
}
