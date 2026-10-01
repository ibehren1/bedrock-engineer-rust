/**
 * Helpers for the user-defined agent arrangement (drag & drop on the My Agents
 * page). The order is stored as a list of agent ids; ids that are not in the
 * stored order keep their natural position at the end of the list.
 */

/** Sort agents by a stored id order. Unknown ids keep their relative order, last. */
export const sortAgentsByOrder = <T extends { id?: string }>(agents: T[], order: string[]): T[] => {
  if (order.length === 0) return agents

  const rank = new Map(order.map((id, index) => [id, index]))
  return [...agents]
    .map((agent, index) => ({ agent, index }))
    .sort((a, b) => {
      const rankA = a.agent.id !== undefined ? rank.get(a.agent.id) : undefined
      const rankB = b.agent.id !== undefined ? rank.get(b.agent.id) : undefined

      if (rankA === undefined && rankB === undefined) return a.index - b.index
      if (rankA === undefined) return 1
      if (rankB === undefined) return -1
      return rankA - rankB
    })
    .map(({ agent }) => agent)
}

/**
 * Move `draggedId` to the position of `targetId` within the full id list.
 * Dropping onto the second half of the list moves the agent after the target,
 * which is what dragging downwards in a list is expected to do.
 */
export const reorderAgentIds = (ids: string[], draggedId: string, targetId: string): string[] => {
  if (draggedId === targetId) return ids

  const from = ids.indexOf(draggedId)
  const to = ids.indexOf(targetId)
  if (from === -1 || to === -1) return ids

  const result = ids.filter((id) => id !== draggedId)
  const insertAt = result.indexOf(targetId) + (from < to ? 1 : 0)
  result.splice(insertAt, 0, draggedId)
  return result
}
