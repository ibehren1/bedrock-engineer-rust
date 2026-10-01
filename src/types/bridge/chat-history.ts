// `window.chatHistory`, installed by src/renderer/src/lib/tauriBridge.ts. The synchronous getters
// read a cache hydrated at startup (docs/port/BRIDGE.md, "Startup and synchronous methods").
import type { ChatMessage, ChatSession, SessionMetadata } from '../chat/history'

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
}
