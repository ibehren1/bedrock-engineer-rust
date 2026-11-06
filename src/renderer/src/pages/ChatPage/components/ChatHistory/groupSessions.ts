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

/** MIME type of the folder id a dragged folder header carries. */
export const FOLDER_DRAG_TYPE = 'application/x-bedrock-folder'

/** Whether a drag carries a folder from the history list. */
export const isFolderDrag = (e: { dataTransfer: DataTransfer | null }): boolean =>
  !!e.dataTransfer && Array.from(e.dataTransfer.types).includes(FOLDER_DRAG_TYPE)

/**
 * `ids` with `movedId` placed before or after `targetId`; `ids` unchanged when either is
 * unknown or they are the same.
 */
export function moveFolder(
  ids: string[],
  movedId: string,
  targetId: string,
  position: 'before' | 'after'
): string[] {
  if (movedId === targetId || !ids.includes(movedId) || !ids.includes(targetId)) return ids
  const rest = ids.filter((id) => id !== movedId)
  const at = rest.indexOf(targetId) + (position === 'after' ? 1 : 0)
  return [...rest.slice(0, at), movedId, ...rest.slice(at)]
}

/** `ids` with `id` moved one place up (`-1`) or down (`1`); unchanged at either end. */
export function shiftFolder(ids: string[], id: string, by: -1 | 1): string[] {
  const i = ids.indexOf(id)
  const j = i + by
  if (i < 0 || j < 0 || j >= ids.length) return ids
  const next = [...ids]
  ;[next[i], next[j]] = [next[j], next[i]]
  return next
}
