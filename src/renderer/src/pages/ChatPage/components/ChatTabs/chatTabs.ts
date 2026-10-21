/**
 * The chat page's tabs: the conversation plus the views that used to be a slide-out panel
 * (Files, Sandbox) or modals (Todo, Cost, System prompt), and the chat's local shells
 * (Terminal). A tab is listed only when it has something to show; Chat and System prompt
 * always are.
 */
export type ChatTab = 'chat' | 'files' | 'sandbox' | 'terminal' | 'todo' | 'cost' | 'systemPrompt'

export const CHAT_TAB_ORDER: readonly ChatTab[] = [
  'chat',
  'files',
  'sandbox',
  'terminal',
  'todo',
  'cost',
  'systemPrompt'
]

/** What decides which tabs are listed, as of one render. */
export type ChatTabConditions = {
  hasProject: boolean
  hasSandbox: boolean
  /** The chat has at least one local shell open. */
  hasTerminal: boolean
  /** The chat's todo list has at least one item. */
  hasTodo: boolean
  hasMessages: boolean
}

export function isTabVisible(tab: ChatTab, c: ChatTabConditions): boolean {
  switch (tab) {
    case 'files':
      return c.hasProject
    case 'sandbox':
      return c.hasSandbox
    case 'terminal':
      return c.hasTerminal
    case 'todo':
      return c.hasTodo
    case 'cost':
      return c.hasMessages
    default:
      return true
  }
}

export function visibleTabs(c: ChatTabConditions): ChatTab[] {
  return CHAT_TAB_ORDER.filter((tab) => isTabVisible(tab, c))
}

/** The tab to show: the chosen one while it is listed, otherwise Chat. */
export function effectiveTab(tab: ChatTab, c: ChatTabConditions): ChatTab {
  return isTabVisible(tab, c) ? tab : 'chat'
}

/**
 * The last tab chosen in each chat, so switching chats brings each one back where it was left.
 * Chats never visited (including new ones) open on Chat. In memory only.
 */
export class TabMemory {
  private readonly tabs = new Map<string, ChatTab>()

  get(sessionId: string | undefined): ChatTab {
    return (sessionId && this.tabs.get(sessionId)) || 'chat'
  }

  set(sessionId: string | undefined, tab: ChatTab): void {
    if (!sessionId) return
    if (tab === 'chat') this.tabs.delete(sessionId)
    else this.tabs.set(sessionId, tab)
  }
}
