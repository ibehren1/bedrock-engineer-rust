import { IdentifiableMessage } from '@/types/chat/message'
import React, { memo, useCallback, useRef } from 'react'
import { ChatMessage } from './Message'
import { SelectionCopyToolbar } from './SelectionCopyToolbar'
import AILogo from '@renderer/assets/images/icons/bedrock-color.png'

type MessageListProps = {
  messages: IdentifiableMessage[]
  loading: boolean
  reasoning: boolean
  waitingForResponse?: boolean
  timeoutCountdown?: number
  heartbeatCount?: number
  deleteMessage?: (index: number) => void
}

const LoadingMessage = memo(function LoadingMessage({
  waiting,
  countdown,
  heartbeats
}: {
  waiting?: boolean
  countdown?: number
  heartbeats?: number
}) {
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60)
    const secs = seconds % 60
    return `${mins}:${secs.toString().padStart(2, '0')}`
  }

  const dots = heartbeats ? '..'.repeat(Math.min(heartbeats, 10)) : ''

  return (
    <div className="flex gap-4">
      <div className="flex items-center justify-center w-10 h-10">
        <div className="h-4 w-4 animate-pulse">
          <img src={AILogo} className="h-full w-full object-contain" alt="assistant" />
        </div>
      </div>
      <div className="flex flex-col gap-2 w-full">
        <span className="animate-pulse h-2 w-12 bg-raised rounded-control"></span>
        {waiting && countdown !== undefined && (
          <div className="text-xs text-warning mb-2">
            Processing — {formatTime(countdown)} remaining{dots}
          </div>
        )}
        <div className="flex-1 space-y-3 py-1">
          <div className="space-y-3">
            <div className="grid grid-cols-3 gap-4">
              <div className="h-2 bg-raised rounded-control col-span-2"></div>
              <div className="h-2 bg-raised rounded-control col-span-1"></div>
            </div>
            <div className="h-2 bg-raised rounded-control"></div>
          </div>
        </div>
      </div>
    </div>
  )
})

// MessageListコンポーネントを定義
const MessageListBase: React.FC<MessageListProps> = ({
  messages,
  loading,
  reasoning,
  waitingForResponse,
  timeoutCountdown,
  heartbeatCount,
  deleteMessage
}) => {
  const handleDeleteMessage = useCallback(
    (messageIndex: number) => () => {
      if (deleteMessage) {
        deleteMessage(messageIndex)
      }
    },
    [deleteMessage]
  )

  const listRef = useRef<HTMLDivElement>(null)

  return (
    <div className="flex flex-col gap-4" ref={listRef}>
      <SelectionCopyToolbar containerRef={listRef} />
      {messages.map((message, index) => {
        // Divider at each user <-> assistant boundary (role changes from
        // the previous message).
        const showDivider = index > 0 && messages[index - 1].role !== message.role
        return (
          <React.Fragment key={message.id || index}>
            {showDivider && <hr className="hr-accent my-2" />}
            <ChatMessage
              message={message}
              reasoning={reasoning}
              isLast={index === messages.length - 1}
              onDeleteMessage={deleteMessage ? handleDeleteMessage(index) : undefined}
            />
          </React.Fragment>
        )
      })}
      {loading && (
        <LoadingMessage
          waiting={waitingForResponse}
          countdown={timeoutCountdown}
          heartbeats={heartbeatCount}
        />
      )}
    </div>
  )
}

// React.memoを使用してメモ化したコンポーネントをエクスポート
export const MessageList = memo(MessageListBase)
