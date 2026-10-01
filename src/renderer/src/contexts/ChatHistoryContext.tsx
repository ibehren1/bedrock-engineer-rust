import React, { createContext, useContext, useState, useEffect, useCallback } from 'react'
import { SessionMetadata, ChatSession, ChatMessage } from '@/types/chat/history'
import { clearHostApproval } from '@renderer/pages/ChatPage/lib/hostCommandApproval'

interface ChatHistoryContextType {
  sessions: SessionMetadata[]
  currentSessionId?: string
  getSession: (sessionId: string) => ChatSession | null
  createSession: (agentId: string, modelId: string, systemPrompt?: string) => Promise<string>
  addMessage: (sessionId: string, message: ChatMessage) => Promise<void>
  updateSessionTitle: (sessionId: string, title: string) => Promise<void>
  deleteSession: (sessionId: string, deleteSandboxData?: boolean) => Promise<void>
  deleteSessions: (sessionIds: string[], deleteSandboxData?: boolean) => Promise<void>
  deleteAllSessions: (deleteSandboxData?: boolean) => Promise<void>
  /** Session ids that currently have a Docker sandbox on disk. */
  getSessionsWithSandbox: (sessionIds: string[]) => Promise<string[]>
  setActiveSession: (sessionId: string) => void
  updateMessageContent: (
    sessionId: string,
    messageIndex: number,
    updatedMessage: ChatMessage
  ) => Promise<void>
  deleteMessage: (sessionId: string, messageIndex: number) => Promise<void>
}

const ChatHistoryContext = createContext<ChatHistoryContextType | undefined>(undefined)

export const ChatHistoryProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [sessions, setSessions] = useState<SessionMetadata[]>([])
  const [currentSessionId, setCurrentSessionId] = useState<string | undefined>()

  // 初期化時にセッションをロード
  useEffect(() => {
    loadSessions()

    // アクティブセッションを取得
    const activeSessionId = window.chatHistory.getActiveSessionId()
    if (activeSessionId) {
      setCurrentSessionId(activeSessionId)
    }
  }, [])

  // セッション一覧を読み込む
  const loadSessions = useCallback(() => {
    const sessionMetadata = window.chatHistory.getAllSessionMetadata()
    setSessions(sessionMetadata)
  }, [])

  // セッション情報を取得
  const getSession = useCallback((sessionId: string) => {
    return window.chatHistory.getSession(sessionId)
  }, [])

  // 新規セッションを作成
  const createSession = useCallback(
    async (agentId: string, modelId: string, systemPrompt?: string): Promise<string> => {
      const newSessionId = await window.chatHistory.createSession(agentId, modelId, systemPrompt)
      loadSessions() // セッション一覧を更新
      return newSessionId
    },
    [loadSessions]
  )

  // メッセージを追加
  const addMessage = useCallback(
    async (sessionId: string, message: ChatMessage): Promise<void> => {
      await window.chatHistory.addMessage(sessionId, message)
      loadSessions() // セッション一覧を更新
    },
    [loadSessions]
  )

  // セッションタイトルを更新
  const updateSessionTitle = useCallback(
    async (sessionId: string, title: string): Promise<void> => {
      await window.chatHistory.updateSessionTitle(sessionId, title)
      loadSessions() // セッション一覧を更新

      // サンドボックスのフォルダ名をタイトルに追従させる。コンテナは作り直さないので
      // インストール済みパッケージは保たれる。サンドボックスが無い場合は何も起きない。
      try {
        await window.api.dockerSandbox.rename(sessionId)
      } catch {
        // 名前が読みやすくなるだけの処理なので、失敗してもリネーム自体は妨げない。
      }

      // 添付ファイルのフォルダもタイトルに追従させる（ファイルはそのまま残る）。
      try {
        await window.api.chatAttachments.rename(sessionId)
      } catch {
        // こちらも見た目だけの処理なので、失敗を伝播させない。
      }
    },
    [loadSessions]
  )

  // チャット削除に伴う後始末。Docker コンテナは常に削除し、そのデータフォルダは
  // 呼び出し側の指定に従う。添付ファイルのフォルダはユーザーが持っているファイルの
  // コピーなので、チェックボックスは設けず常に削除する。
  const teardownChatResources = useCallback(
    async (sessionIds: string[], deleteSandboxData: boolean): Promise<void> => {
      await Promise.all(
        sessionIds.map(async (sessionId) => {
          clearHostApproval(sessionId)
          try {
            await window.api.dockerSandbox.remove(sessionId, { deleteData: deleteSandboxData })
          } catch {
            // A chat without a sandbox is the common case, and a Docker daemon that is
            // down must not block deleting the chat itself.
          }
          try {
            await window.api.chatAttachments.removeAll(sessionId)
          } catch {
            // 添付が無いチャットが大半なので、失敗しても削除自体は続行する。
          }
        })
      )
    },
    []
  )

  // Docker サンドボックスを持つセッションを絞り込む（削除ダイアログの表示制御用）
  const getSessionsWithSandbox = useCallback(async (sessionIds: string[]): Promise<string[]> => {
    try {
      const { sessionIds: withSandbox } = await window.api.dockerSandbox.list()
      return sessionIds.filter((id) => withSandbox.includes(id))
    } catch {
      return []
    }
  }, [])

  // セッションを削除
  const deleteSession = useCallback(
    async (sessionId: string, deleteSandboxData = false): Promise<void> => {
      await teardownChatResources([sessionId], deleteSandboxData)
      window.chatHistory.deleteSession(sessionId)
      loadSessions() // セッション一覧を更新
    },
    [loadSessions, teardownChatResources]
  )

  // 選択した複数のセッションを削除
  const deleteSessions = useCallback(
    async (sessionIds: string[], deleteSandboxData = false): Promise<void> => {
      if (!sessionIds || sessionIds.length === 0) {
        return
      }
      await teardownChatResources(sessionIds, deleteSandboxData)
      window.chatHistory.deleteSessions(sessionIds)
      loadSessions() // セッション一覧を更新
      // 現在のセッションが削除対象に含まれる場合はクリア
      setCurrentSessionId((prev) => (prev && sessionIds.includes(prev) ? undefined : prev))
    },
    [loadSessions, teardownChatResources]
  )

  // 全セッションを削除
  const deleteAllSessions = useCallback(
    async (deleteSandboxData = false): Promise<void> => {
      // サイドバーはメッセージ 0 件のセッションを隠すので、一覧ではなくディスク上の
      // サンドボックス全件を対象にする。取り残しを防ぐため。
      let sandboxSessionIds: string[] = []
      try {
        sandboxSessionIds = (await window.api.dockerSandbox.list()).sessionIds
      } catch {
        sandboxSessionIds = []
      }
      await teardownChatResources(sandboxSessionIds, deleteSandboxData)

      // 添付フォルダはサイドバーに出ないチャットの分も残るので、ディスク上を丸ごと片付ける。
      try {
        await window.api.chatAttachments.removeEveryFolder()
      } catch {
        // プロジェクトフォルダ未設定などで失敗しても、チャットの削除は続行する。
      }

      window.chatHistory.deleteAllSessions()
      loadSessions() // セッション一覧を更新
      setCurrentSessionId(undefined)
    },
    [loadSessions, teardownChatResources]
  )

  // アクティブセッションを設定
  const setActiveSession = useCallback((sessionId: string): void => {
    window.chatHistory.setActiveSession(sessionId)
    setCurrentSessionId(sessionId)
  }, [])

  // メッセージ内容を更新
  const updateMessageContent = useCallback(
    async (sessionId: string, messageIndex: number, updatedMessage: ChatMessage): Promise<void> => {
      await window.chatHistory.updateMessageContent(sessionId, messageIndex, updatedMessage)
      loadSessions() // セッション一覧を更新
    },
    [loadSessions]
  )

  // メッセージを削除
  const deleteMessage = useCallback(
    async (sessionId: string, messageIndex: number): Promise<void> => {
      await window.chatHistory.deleteMessage(sessionId, messageIndex)
      loadSessions() // セッション一覧を更新
    },
    [loadSessions]
  )

  const value = {
    sessions,
    currentSessionId,
    getSession,
    createSession,
    addMessage,
    updateSessionTitle,
    deleteSession,
    deleteSessions,
    deleteAllSessions,
    getSessionsWithSandbox,
    setActiveSession,
    updateMessageContent,
    deleteMessage
  }

  return <ChatHistoryContext.Provider value={value}>{children}</ChatHistoryContext.Provider>
}

// Custom Hook
export const useChatHistory = () => {
  const context = useContext(ChatHistoryContext)
  if (context === undefined) {
    throw new Error('useChatHistory must be used within a ChatHistoryProvider')
  }
  return context
}
