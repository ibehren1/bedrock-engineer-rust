/**
 * When a chat-page tab (Files, Sandbox) draws attention: a short pulse when it appears in a
 * chat, when the chat gets a sandbox, and when the agent touches new files while the tab is not
 * showing; plus a marker that stays until the tab is viewed.
 */

/** What the tab knows about the chat, as of one render. */
export type AttentionSnapshot = {
  sessionId?: string
  /** The tab is listed. */
  visible: boolean
  hasSandbox: boolean
  /** Files the agent created or changed in this chat. */
  touchedCount: number
}

export type Attention = {
  /** Start a pulse now. */
  pulse: boolean
  /** Show the dot. */
  badge: boolean
}

/**
 * The tab's attention after going from `prev` (undefined: first render) to `next`, given
 * whether the tab is showing (`panelOpen`) and whether the marker is.
 */
export function nextAttention(
  prev: AttentionSnapshot | undefined,
  next: AttentionSnapshot,
  panelOpen: boolean,
  badge: boolean
): Attention {
  if (panelOpen || !next.visible) return { pulse: false, badge: false }
  const hasContent = next.hasSandbox || next.touchedCount > 0
  // First shown in this chat (opening the page, switching chats, or the tab appearing).
  if (!prev || !prev.visible || prev.sessionId !== next.sessionId) {
    return { pulse: true, badge: hasContent }
  }
  const sandboxArrived = next.hasSandbox && !prev.hasSandbox
  const newFiles = next.touchedCount > prev.touchedCount
  if (sandboxArrived || newFiles) return { pulse: true, badge: true }
  return { pulse: false, badge }
}
