import React, { useState, useRef, useEffect } from 'react'
import { FiSend, FiLoader, FiSettings } from 'react-icons/fi'

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: any[]
  timestamp: number
}

interface BackgroundChatAreaProps {
  sessionId: string | null
  messages: ChatMessage[]
  loading: boolean
  onSendMessage: (message: string) => void
  onOpenSettings: () => void
}

export const BackgroundChatArea: React.FC<BackgroundChatAreaProps> = ({
  sessionId,
  messages,
  loading,
  onSendMessage,
  onOpenSettings
}) => {
  const [inputMessage, setInputMessage] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // メッセージが更新されたら最下部にスクロール
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // テキストエリアの高さを自動調整
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`
    }
  }, [inputMessage])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!inputMessage.trim() || loading) return

    onSendMessage(inputMessage)
    setInputMessage('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit(e)
    }
  }

  const formatMessageContent = (content: any[]) => {
    return content
      .map((block) => {
        if (block.text) return block.text
        if (block.toolUse) return `[Tool: ${block.toolUse.name}]`
        if (block.toolResult) return `[Tool Result]`
        return JSON.stringify(block)
      })
      .join('\n')
  }

  if (!sessionId) {
    return (
      <div className="flex-1 flex items-center justify-center bg-surface-2">
        <div className="text-center">
          <div className="w-16 h-16 bg-raised rounded-full flex items-center justify-center mx-auto mb-4">
            <FiSettings className="w-8 h-8 text-ink-faint" />
          </div>
          <h3 className="text-heading font-medium text-ink mb-2">セッションを選択してください</h3>
          <p className="text-ink-muted">
            左側のリストからセッションを選択するか、新規セッションを作成してください
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col bg-surface">
      {/* ヘッダー */}
      <div className="px-3 py-2 border-b border-subtle flex items-center justify-between">
        <div>
          <h2 className="text-heading font-semibold text-ink">Background Agent Chat</h2>
          <p className="text-sm text-ink-muted">Session: {sessionId}</p>
        </div>
        <button
          onClick={onOpenSettings}
          className="p-2 text-ink-faint hover:text-ink-muted rounded-control hover:bg-raised"
          title="設定"
        >
          <FiSettings className="w-4 h-4" />
        </button>
      </div>

      {/* メッセージエリア */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <p className="text-ink-muted">
                このセッションにはまだメッセージがありません。
                <br />
                下のフォームからメッセージを送信してください。
              </p>
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            {messages.map((message, index) => (
              <div
                key={message.id || index}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[70%] rounded-container px-3 py-1.5 ${
                    message.role === 'user' ? 'bg-accent text-accent-fg' : 'bg-raised text-ink'
                  }`}
                >
                  <div className="whitespace-pre-wrap text-sm">
                    {formatMessageContent(message.content)}
                  </div>
                  <div
                    className={`text-xs mt-1 ${
                      message.role === 'user' ? 'text-accent' : 'text-ink-muted'
                    }`}
                  >
                    {new Date(message.timestamp).toLocaleTimeString()}
                  </div>
                </div>
              </div>
            ))}
            {loading && (
              <div className="flex justify-start">
                <div className="bg-raised rounded-container px-3 py-1.5">
                  <div className="flex items-center gap-2 text-ink-muted">
                    <FiLoader className="w-4 h-4 animate-spin" />
                    <span className="text-sm">AI が応答しています...</span>
                  </div>
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* 入力フォーム */}
      <div className="px-3 py-2 border-t border-subtle">
        <form onSubmit={handleSubmit} className="flex gap-3">
          <div className="flex-1">
            <textarea
              ref={textareaRef}
              value={inputMessage}
              onChange={(e) => setInputMessage(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="メッセージを入力..."
              className="w-full px-3 py-2 border border-strong rounded-control focus:outline-none focus:ring-2 focus:ring-accent bg-raised text-ink resize-none min-h-[40px] max-h-[120px]"
              disabled={loading}
              rows={1}
            />
          </div>
          <button
            type="submit"
            disabled={!inputMessage.trim() || loading}
            className="px-2.5 py-1 bg-accent text-accent-fg rounded-control hover:bg-accent-strong disabled:bg-sunken disabled:cursor-not-allowed flex items-center gap-2 self-end"
          >
            {loading ? (
              <FiLoader className="w-4 h-4 animate-spin" />
            ) : (
              <FiSend className="w-4 h-4" />
            )}
            送信
          </button>
        </form>
      </div>
    </div>
  )
}
