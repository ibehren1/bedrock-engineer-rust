import { memo } from 'react'
import { AiOutlineCloseCircle } from 'react-icons/ai'
import { MdOutlineContentCopy } from 'react-icons/md'
import { toast } from 'react-hot-toast'
import MD from '@renderer/components/Markdown/MD'
import { AWSCDKConvertButton } from './AWSCDKConvertButton'
import { detectAWSElements } from '../utils/awsDetector'

type DiagramExplanationViewProps = {
  explanation: string
  isVisible: boolean
  isStreaming?: boolean
  onClose: () => void
  xml?: string
  onCDKConvert?: () => void
  hasMessages?: boolean
}

/**
 * ダイアグラムの説明表示コンポーネント
 *
 * ダイアグラムの説明文を表示し、閉じるボタンとコピーボタンを提供する
 */
const DiagramExplanationViewComponent = ({
  explanation,
  isVisible,
  isStreaming = false,
  onClose,
  xml = '',
  onCDKConvert,
  hasMessages = false
}: DiagramExplanationViewProps) => {
  if (!isVisible || !explanation) {
    return null
  }

  const handleCopyExplanation = () => {
    navigator.clipboard.writeText(explanation)
    toast.success('説明をクリップボードにコピーしました')
  }

  // AWS関連要素を検出（LLMとの会話がある場合のみ表示）
  const showCDKButton =
    detectAWSElements(xml, explanation) && onCDKConvert && !isStreaming && hasMessages

  return (
    <div className="h-full flex flex-col bg-surface rounded-container shadow-md overflow-hidden">
      {/* ヘッダー部分 */}
      <div className="flex justify-between items-center p-3 border-b border-subtle">
        <h3 className="text-heading font-medium text-ink">図の説明</h3>
        <div className="flex gap-2">
          <button
            onClick={handleCopyExplanation}
            className="p-1 hover:bg-raised rounded-control"
            title="説明をコピー"
          >
            <MdOutlineContentCopy className="text-ink-muted" size={18} />
          </button>
          <button onClick={onClose} className="p-1 hover:bg-raised rounded-control" title="閉じる">
            <AiOutlineCloseCircle className="text-ink-muted" size={18} />
          </button>
        </div>
      </div>

      {/* コンテンツ部分 */}
      <div className="flex-1 p-2.5 overflow-y-auto text-ink">
        <MD>{explanation}</MD>
        {isStreaming && (
          <div className="flex items-center gap-2 mt-2 text-ink-muted">
            <div className="flex space-x-1">
              <div className="w-2 h-2 bg-sunken rounded-full animate-bounce [animation-delay:-0.3s]"></div>
              <div className="w-2 h-2 bg-sunken rounded-full animate-bounce [animation-delay:-0.15s]"></div>
              <div className="w-2 h-2 bg-sunken rounded-full animate-bounce"></div>
            </div>
            <span className="text-sm">生成中...</span>
          </div>
        )}
      </div>

      {/* AWS CDK変換ボタン */}
      {showCDKButton && onCDKConvert && (
        <AWSCDKConvertButton visible={true} onConvert={onCDKConvert} />
      )}
    </div>
  )
}

// メモ化してpropsが変更されない限り再レンダリングしない
export const DiagramExplanationView = memo(DiagramExplanationViewComponent)
DiagramExplanationView.displayName = 'DiagramExplanationView'
