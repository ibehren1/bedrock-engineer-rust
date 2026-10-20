/**
 * When the chat page's right-panel opener (the Files / Sandbox strip) draws attention: a short
 * pulse when it appears in a chat, when the chat gets a sandbox, and when the agent touches new
 * files while the panel is closed; plus a dot that stays until the panel is opened.
 */

/** What the opener knows about the chat, as of one render. */
export type AttentionSnapshot = {
  sessionId?: string
  /** The strip is shown (the chat has a project folder or a sandbox). */
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
 * The opener's attention after going from `prev` (undefined: first render) to `next`, given
 * whether the panel is open and whether the dot is showing.
 */
export function nextAttention(
  prev: AttentionSnapshot | undefined,
  next: AttentionSnapshot,
  panelOpen: boolean,
  badge: boolean
): Attention {
  if (panelOpen || !next.visible) return { pulse: false, badge: false }
  const hasContent = next.hasSandbox || next.touchedCount > 0
  // First shown in this chat (opening the page, switching chats, or the strip appearing).
  if (!prev || !prev.visible || prev.sessionId !== next.sessionId) {
    return { pulse: true, badge: hasContent }
  }
  const sandboxArrived = next.hasSandbox && !prev.hasSandbox
  const newFiles = next.touchedCount > prev.touchedCount
  if (sandboxArrived || newFiles) return { pulse: true, badge: true }
  return { pulse: false, badge }
}
