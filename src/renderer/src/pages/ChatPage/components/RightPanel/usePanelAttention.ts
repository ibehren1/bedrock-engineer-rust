import { useEffect, useRef, useState } from 'react'
import { nextAttention, type AttentionSnapshot } from './panelAttention'

/**
 * A tab's attention state (see `nextAttention`). `pulseKey` changes each time a pulse
 * should start (0: none yet), so it can key the halo element to restart its animation.
 */
export function usePanelAttention(
  snapshot: AttentionSnapshot,
  panelOpen: boolean
): { pulseKey: number; badge: boolean } {
  const prev = useRef<AttentionSnapshot | undefined>(undefined)
  const [pulseKey, setPulseKey] = useState(0)
  const [badge, setBadge] = useState(false)
  const badgeRef = useRef(badge)
  badgeRef.current = badge
  const { sessionId, visible, hasSandbox, touchedCount } = snapshot

  useEffect(() => {
    const next = { sessionId, visible, hasSandbox, touchedCount }
    const attention = nextAttention(prev.current, next, panelOpen, badgeRef.current)
    prev.current = next
    if (attention.pulse) setPulseKey((k) => k + 1)
    setBadge(attention.badge)
  }, [sessionId, visible, hasSandbox, touchedCount, panelOpen])

  return { pulseKey, badge }
}
