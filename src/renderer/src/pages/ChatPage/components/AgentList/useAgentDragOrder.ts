import { useCallback, useState } from 'react'
import { CustomAgent } from '@/types/agent-chat'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { reorderAgentIds } from './agentOrder'

/**
 * Drag & drop reordering for the agent list. Dragging updates the persisted
 * agent order, so the arrangement survives restarts and is reflected everywhere
 * agents are listed (agent dropdown, @mentions).
 *
 * @param enabled false while a column sort is active — the sort would fight the
 *                manual arrangement.
 */
export const useAgentDragOrder = (agents: CustomAgent[], enabled: boolean) => {
  const { agentOrder, setAgentOrder } = useSettings()
  const [draggingId, setDraggingId] = useState<string | null>(null)
  const [dropTargetId, setDropTargetId] = useState<string | null>(null)

  const reset = useCallback(() => {
    setDraggingId(null)
    setDropTargetId(null)
  }, [])

  const handleDragStart = useCallback(
    (agentId: string) => (event: React.DragEvent) => {
      if (!enabled) return
      setDraggingId(agentId)
      event.dataTransfer.effectAllowed = 'move'
      // Firefox/Chromium need data set for the drag to start at all
      event.dataTransfer.setData('text/plain', agentId)
    },
    [enabled]
  )

  const handleDragOver = useCallback(
    (agentId: string) => (event: React.DragEvent) => {
      if (!enabled || !draggingId) return
      // Without preventDefault the drop event never fires
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'
      if (agentId !== dropTargetId) {
        setDropTargetId(agentId)
      }
    },
    [enabled, draggingId, dropTargetId]
  )

  const handleDrop = useCallback(
    (agentId: string) => (event: React.DragEvent) => {
      if (!enabled) return
      event.preventDefault()

      const droppedId = draggingId || event.dataTransfer.getData('text/plain')
      reset()
      if (!droppedId || droppedId === agentId) return

      // Start from the current arrangement so agents hidden by a search or tag
      // filter keep their place.
      const currentIds = agents.map((agent) => agent.id).filter((id): id is string => !!id)
      const knownIds = new Set(currentIds)
      const baseOrder = [
        ...agentOrder.filter((id) => knownIds.has(id)),
        ...currentIds.filter((id) => !agentOrder.includes(id))
      ]

      setAgentOrder(reorderAgentIds(baseOrder, droppedId, agentId))
    },
    [enabled, draggingId, reset, agents, agentOrder, setAgentOrder]
  )

  /** Props to spread on each draggable row/card. */
  const dragProps = useCallback(
    (agentId?: string) => {
      if (!enabled || !agentId) return {}
      return {
        draggable: true,
        onDragStart: handleDragStart(agentId),
        onDragOver: handleDragOver(agentId),
        onDrop: handleDrop(agentId),
        onDragEnd: reset,
        onDragLeave: () => setDropTargetId((current) => (current === agentId ? null : current))
      }
    },
    [enabled, handleDragStart, handleDragOver, handleDrop, reset]
  )

  /** Extra classes marking the dragged item and the current drop target. */
  const dragClassName = useCallback(
    (agentId?: string) => {
      if (!enabled || !agentId) return ''
      if (agentId === draggingId) return 'opacity-40'
      if (agentId === dropTargetId) return 'ring-2 ring-accent'
      return ''
    },
    [enabled, draggingId, dropTargetId]
  )

  return { dragProps, dragClassName, isDragging: !!draggingId }
}
