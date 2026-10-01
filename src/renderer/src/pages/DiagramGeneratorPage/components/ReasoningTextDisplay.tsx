import { useState, useRef, memo } from 'react'
import { MdExpandMore } from 'react-icons/md'

type ReasoningTextDisplayProps = {
  text: string
}

// メモ化してpropsが変更されない限り再レンダリングしない
const ReasoningTextDisplayComponent = ({ text }: ReasoningTextDisplayProps) => {
  // 展開状態を管理するstate
  const [isExpanded, setIsExpanded] = useState(false)
  const textAreaRef = useRef<HTMLDivElement>(null)

  if (!text) return null

  return (
    <div>
      {/* クリック可能なヘッダー部分 */}
      <div
        className="flex justify-center items-center space-x-1 cursor-pointer mb-1"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <span className="text-xs font-medium text-accent">Reasoning</span>
        <MdExpandMore
          className={`transform transition-transform ${isExpanded ? 'rotate-180' : ''} text-ink-muted text-xs`}
          size={14}
        />
      </div>

      {/* 展開可能なコンテンツ部分 */}
      <div
        className={`transition-all duration-300 rounded-container overflow-hidden ${
          isExpanded
            ? 'max-h-[30rem] opacity-100 bg-surface-2/50 border border-subtle backdrop-blur-sm'
            : 'max-h-0 opacity-0'
        }`}
      >
        <div className="px-2.5 py-1.5">
          <div
            ref={textAreaRef}
            className="pb-5 text-sm text-ink leading-relaxed whitespace-pre-wrap font-light overflow-y-auto pr-2 max-h-[25rem] break-words overflow-wrap-anywhere"
          >
            {text || 'Thinking...'}
          </div>
        </div>
      </div>
    </div>
  )
}

// displayNameを設定してESLintエラーを解消
export const ReasoningTextDisplay = memo(ReasoningTextDisplayComponent)
ReasoningTextDisplay.displayName = 'ReasoningTextDisplay'
