import type { ChatFolder, SessionMetadata } from '@/types/chat/history'

export type GroupedSessions = {
  /** Each folder's chats, keyed by folder id, in the order of `sessions`. Every folder has an entry. */
  byFolder: Map<string, SessionMetadata[]>
  /** Chats in no folder, or in a folder that no longer exists. */
  loose: SessionMetadata[]
}

/** Splits the chat list by folder, keeping the list's order inside each group. */
export function groupSessionsByFolder(
  sessions: SessionMetadata[],
  folders: ChatFolder[]
): GroupedSessions {
  const byFolder = new Map<string, SessionMetadata[]>(folders.map((f) => [f.id, []]))
  const loose: SessionMetadata[] = []
  for (const session of sessions) {
    const group = session.folderId ? byFolder.get(session.folderId) : undefined
    if (group) group.push(session)
    else loose.push(session)
  }
  return { byFolder, loose }
}

/** MIME type of the chat ids a dragged history row carries (a JSON array of session ids). */
export const CHAT_DRAG_TYPE = 'application/x-bedrock-chat'

/** Whether a drag carries chats from the history list. */
export const isChatDrag = (e: { dataTransfer: DataTransfer | null }): boolean =>
  !!e.dataTransfer && Array.from(e.dataTransfer.types).includes(CHAT_DRAG_TYPE)

/** The chat ids a drop carries; `[]` for anything else. */
export function draggedChatIds(dataTransfer: DataTransfer | null): string[] {
  try {
    const ids: unknown = JSON.parse(dataTransfer?.getData(CHAT_DRAG_TYPE) || '[]')
    return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === 'string') : []
  } catch {
    return []
  }
}
