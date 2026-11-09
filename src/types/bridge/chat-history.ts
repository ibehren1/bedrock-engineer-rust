// `window.chatHistory`, installed by src/renderer/src/lib/tauriBridge.ts. The synchronous getters
// read a cache hydrated at startup (docs/port/BRIDGE.md, "Startup and synchronous methods").
import type {
  ChatFolder,
  ChatMessage,
  ChatSession,
  MentionGrants,
  SessionMetadata
} from '../chat/history'

export type ChatHistoryApi = {
  createSession(agentId: string, modelId: string, systemPrompt?: string): Promise<string>
  addMessage(sessionId: string, message: ChatMessage): Promise<void>
  /**
   * A copy of the whole session. Served from the cache; a session not loaded yet is read
   * synchronously (blocking the window), so call `loadSession` first where you can, and use
   * `getSessionField` / `getTouchedFiles` when you need one field.
   */
  getSession(sessionId: string): ChatSession | null
  /** Read a session into the cache without blocking; resolves to whether it exists. */
  loadSession(sessionId: string): Promise<boolean>
  /** Whether the session is in the cache (so `getSession` won't block). */
  isSessionLoaded(sessionId: string): boolean
  /**
   * One field of a session, without copying the rest. From the cache; for a session not loaded
   * yet only the list metadata's fields (title, agentId, modelId, ...) are known, the rest is
   * `undefined`. Never reads the session file.
   */
  getSessionField<K extends keyof ChatSession>(
    sessionId: string,
    key: K
  ): ChatSession[K] | undefined
  /** The session's saved `touchedFiles` (from the cache; `[]` if not loaded). */
  getTouchedFiles(sessionId: string): string[]
  updateSessionTitle(sessionId: string, title: string): Promise<void>
  deleteSession(sessionId: string): void
  deleteSessions(sessionIds: string[]): void
  deleteAllSessions(): void
  getRecentSessions(): SessionMetadata[]
  getAllSessionMetadata(): SessionMetadata[]
  setActiveSession(sessionId: string | undefined): void
  getActiveSessionId(): string | undefined
  updateMessageContent(
    sessionId: string,
    messageIndex: number,
    updatedMessage: ChatMessage
  ): Promise<void>
  deleteMessage(sessionId: string, messageIndex: number): Promise<void>
  setUseMemoryItems(sessionId: string, value: boolean): Promise<void>
  /** Save the agents and knowledge bases on for the chat. */
  setMentionGrants(sessionId: string, grants: MentionGrants): Promise<void>
  /**
   * Merge project-relative paths into the session's `touchedFiles` (deduplicated). Each call
   * rewrites the session file; `recordTouchedFiles` (lib/workspaceFiles) batches them.
   */
  addTouchedFiles(sessionId: string, paths: string[]): Promise<void>
  /** The chat history's folders, in display order (from the startup cache). */
  getFolders(): ChatFolder[]
  /** Adds a folder and returns its id. */
  createFolder(name: string): Promise<string>
  renameFolder(folderId: string, name: string): Promise<void>
  /** Puts the folders in this order (unknown ids ignored, unlisted folders last). */
  reorderFolders(folderIds: string[]): Promise<void>
  /** Removes a folder; its chats stay and move back out of any folder. */
  deleteFolder(folderId: string): Promise<void>
  /** Puts chats in a folder, or takes them out of any folder with `null`. */
  moveSessionsToFolder(sessionIds: string[], folderId: string | null): Promise<void>
}
