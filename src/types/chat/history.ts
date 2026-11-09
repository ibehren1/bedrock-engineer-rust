import { ConversationRole, ContentBlock } from '@aws-sdk/client-bedrock-runtime'
import { ToolState } from '../agent-chat'

type AttachedImage = {
  file: File
  preview: string
  base64: string
}

export interface ChatMessage {
  id: string
  role: ConversationRole
  content: ContentBlock[]
  timestamp: number
  metadata?: {
    modelId?: string
    tools?: ToolState[]
    images?: AttachedImage[]
    converseMetadata?: Record<string, any> // BedrockのConverseStreamMetadataEvent型のデータを保存するフィールド
  }
}

export interface SessionMetadata {
  id: string
  title: string
  createdAt: number
  updatedAt: number
  messageCount: number
  agentId: string
  modelId: string
  systemPrompt?: string
  /** The chat history folder this chat is in, if any. */
  folderId?: string
}

/** A folder in the chat history sidebar. Chats point at it through `folderId`. */
export interface ChatFolder {
  id: string
  name: string
  createdAt: number
}

export interface ChatSession {
  id: string
  title: string
  createdAt: number
  updatedAt: number
  messages: ChatMessage[]
  agentId: string
  systemPrompt?: string
  modelId: string
  /** Whether enabled memory items go into this chat's system prompt. Missing means true. */
  useMemoryItems?: boolean
  /** Project-relative paths the agent created or changed in this chat (Files panel). */
  touchedFiles?: string[]
  /**
   * Agents and knowledge bases on for this chat (from @mentions or the grant buttons). Missing
   * on chats from before it existed; those derive it from their messages.
   */
  mentionGrants?: MentionGrants
}

/** The agents a chat may delegate to and the knowledge bases it searches. */
export type MentionGrants = {
  agentIds: string[]
  knowledgeBases: { agentId: string; kbId: string }[]
}

export interface ChatHistoryStore {
  sessions: {
    [key: string]: ChatSession
  }
  activeSessionId?: string
  recentSessions: string[] // 最近使用したセッションID（最大10個）
}
