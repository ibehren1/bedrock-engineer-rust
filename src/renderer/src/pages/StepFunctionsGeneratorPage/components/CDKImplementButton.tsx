import { memo } from 'react'
import { motion } from 'framer-motion'
import { FaAws } from 'react-icons/fa'
import { useTranslation } from 'react-i18next'

interface CDKImplementButtonProps {
  visible: boolean
  onImplement: () => void
  disabled?: boolean
}

/**
 * CDK実装ボタンコンポーネント
 *
 * ステートマシン定義が生成された場合に表示され、
 * クリックするとAgent ChatページでCDK実装プロンプトが実行される
 */
const CDKImplementButtonComponent = ({
  visible,
  onImplement,
  disabled = false
}: CDKImplementButtonProps) => {
  const { t } = useTranslation()

  if (!visible) return null

  return (
    <motion.button
      initial={{ opacity: 0, scale: 0 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0 }}
      transition={{ duration: 0.3 }}
      onClick={onImplement}
      disabled={disabled}
      className={`
        cursor-pointer
        rounded-full
        border
        p-2
        text-xs
        flex items-center gap-1
        transition-colors
        duration-200
        whitespace-nowrap
        flex-shrink-0
        ${
          disabled
            ? 'bg-raised text-ink-faint border-strong cursor-not-allowed'
            : 'text-accent border-accent bg-accent-tint hover:bg-accent-tint-strong'
        }
      `}
    >
      <FaAws className="text-sm" />
      <span>{t('Chat で CDK を実装する', 'Chat で CDK を実装する')}</span>
    </motion.button>
  )
}

// メモ化してpropsが変更されない限り再レンダリングしない
export const CDKImplementButton = memo(CDKImplementButtonComponent)
CDKImplementButton.displayName = 'CDKImplementButton'
