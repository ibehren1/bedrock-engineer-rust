import { ReactNode, memo } from 'react'
import { ReasoningTextDisplay } from './ReasoningTextDisplay'

type LoaderWithReasoningProps = {
  children: ReactNode
  reasoningText: string
}

const LoaderWithReasoningComponent = ({ children, reasoningText }: LoaderWithReasoningProps) => {
  return (
    <div className="flex flex-col items-center gap-2 w-full">
      {/* ローダーコンポーネント (WebLoader, RagLoader, etc.) */}
      {children}

      {/* ReasoningTextDisplay - 中央に配置 */}
      <div className="flex justify-center w-full">
        {reasoningText && <ReasoningTextDisplay text={reasoningText} />}
      </div>
    </div>
  )
}

// メモ化してpropsが変更されない限り再レンダリングしない
export const LoaderWithReasoning = memo(LoaderWithReasoningComponent)
LoaderWithReasoning.displayName = 'LoaderWithReasoning'
