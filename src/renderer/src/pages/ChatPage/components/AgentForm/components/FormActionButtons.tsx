import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiSave } from 'react-icons/fi'
import { formEventUtils } from '../utils/formEventUtils'

/**
 * フォームアクションボタンコンポーネント
 */
export const FormActionButtons: React.FC<{
  onSubmit?: (e: React.FormEvent) => void
  onCancel: () => void
  isGenerating: boolean
}> = ({ onCancel, isGenerating }) => {
  const { t } = useTranslation()

  return (
    <div className="flex justify-end space-x-3" onClick={formEventUtils.preventPropagation}>
      <button
        type="button"
        onClick={formEventUtils.createSafeHandler(onCancel)}
        className="px-2.5 py-1 text-sm font-medium text-ink bg-surface
          border border-strong rounded-control shadow-sm hover:bg-surface-2
          focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent
          focus:ring-offset-canvas transition-all duration-200 hover:border-strong"
      >
        {t('cancel')}
      </button>
      <button
        type="submit"
        onClick={(e) => {
          e.stopPropagation()
        }}
        disabled={isGenerating}
        className={`flex items-center gap-2 px-2.5 py-1 text-sm font-medium border rounded-control shadow-sm focus:outline-none focus:ring-2
          focus:ring-offset-2 focus:ring-accent focus:ring-offset-canvas transition-all duration-200
          ${
            isGenerating
              ? 'text-ink-faint bg-raised border-strong cursor-not-allowed opacity-70'
              : 'text-accent-fg bg-accent border-transparent hover:bg-accent-strong hover:shadow-md'
          }`}
      >
        {isGenerating ? (
          <>
            <svg
              className="w-4 h-4 mr-1 animate-spin"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              ></circle>
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              ></path>
            </svg>
            <p>{t('generating')}...</p>
          </>
        ) : (
          <>
            <FiSave />
            <p>{t('save')}</p>
          </>
        )}
      </button>
    </div>
  )
}
