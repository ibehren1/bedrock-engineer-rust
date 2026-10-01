import { memo } from 'react'

type ProgressBarProps = {
  progress: number
  message?: string
  showPercentage?: boolean
  className?: string
}

/**
 * 進捗バーコンポーネント
 * XML生成の進捗を視覚的に表示する
 */
const ProgressBarComponent = ({
  progress,
  message,
  showPercentage = true,
  className = ''
}: ProgressBarProps) => {
  // 進捗値を0-100の範囲に制限
  const clampedProgress = Math.min(100, Math.max(0, progress))

  return (
    <div className={`w-full max-w-md ${className}`}>
      {/* メッセージ表示 */}
      {message && <div className="mb-2 text-sm text-ink-muted text-center">{message}</div>}

      {/* 進捗バー */}
      <div className="w-full bg-raised rounded-full h-2.5 relative overflow-hidden">
        <div
          className="bg-accent-tint h-2.5 rounded-full transition-all duration-300 ease-out"
          style={{ width: `${clampedProgress}%` }}
        />

        {/* 進捗バーのアニメーション効果 */}
        <div
          className="absolute top-0 left-0 h-full bg-gradient-to-r from-transparent via-white to-transparent opacity-30 animate-pulse"
          style={{
            width: '30%',
            transform: `translateX(${clampedProgress * 3.33 - 100}%)`,
            transition: 'transform 0.3s ease-out'
          }}
        />
      </div>

      {/* パーセンテージ表示 */}
      {showPercentage && (
        <div className="mt-2 text-center">
          <span className="text-sm font-medium text-ink">{Math.round(clampedProgress)}%</span>
        </div>
      )}
    </div>
  )
}

// メモ化してpropsが変更されない限り再レンダリングしない
export const ProgressBar = memo(ProgressBarComponent)
ProgressBar.displayName = 'ProgressBar'
