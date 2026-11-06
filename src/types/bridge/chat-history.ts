// `window.chatHistory`, installed by src/renderer/src/lib/tauriBridge.ts. The synchronous getters
// read a cache hydrated at startup (docs/port/BRIDGE.md, "Startup and synchronous methods").
import type { ChatFolder, ChatMessage, ChatSession, SessionMetadata } from '../chat/history'

export type ChatHistoryApi = {
  createSession(agentId: string, modelId: string, systemPrompt?: string): Promise<string>
  addMessage(sessionId: string, message: ChatMessage): Promise<void>
  getSession(sessionId: string): ChatSession | null
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
  /** Merge project-relative paths into the session's `touchedFiles` (deduplicated). */
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
