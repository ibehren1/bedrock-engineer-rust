import { memo } from 'react'
import { motion } from 'framer-motion'
import { FaAws } from 'react-icons/fa'
import { useTranslation } from 'react-i18next'

interface AWSCDKConvertButtonProps {
  visible: boolean
  onConvert: () => void
  disabled?: boolean
}

/**
 * AWS CDK変換ボタンコンポーネント
 *
 * AWS関連要素が検出された場合に表示され、
 * クリックするとAgent ChatページでCDK変換プロンプトが実行される
 */
const AWSCDKConvertButtonComponent = ({
  visible,
  onConvert,
  disabled = false
}: AWSCDKConvertButtonProps) => {
  const { t } = useTranslation()

  if (!visible) return null

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 10 }}
      transition={{ duration: 0.3 }}
      className="mt-4 p-3 border-t border-subtle"
    >
      <button
        onClick={onConvert}
        disabled={disabled}
        className={`
          w-full flex items-center justify-center gap-2 px-2.5 py-1 rounded-container
          font-medium text-accent-fg transition-all duration-200
          ${
            disabled
              ? 'bg-sunken cursor-not-allowed'
              : 'bg-accent hover:bg-accent-strong active:bg-accent hover:shadow-md'
          }
        `}
      >
        <FaAws className="text-base" />
        <span>{t('Convert to AWS CDK', 'Agent ChatでAWS CDKに変換')}</span>
      </button>

      <p className="text-xs text-ink-muted mt-2 text-center">
        {t('AWS architecture detected', 'AWS構成が検出されました')}
      </p>
    </motion.div>
  )
}

// メモ化してpropsが変更されない限り再レンダリングしない
export const AWSCDKConvertButton = memo(AWSCDKConvertButtonComponent)
AWSCDKConvertButton.displayName = 'AWSCDKConvertButton'
