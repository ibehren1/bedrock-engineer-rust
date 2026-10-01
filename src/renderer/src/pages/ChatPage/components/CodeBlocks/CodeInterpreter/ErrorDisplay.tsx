import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiCopy, FiCheck, FiAlertTriangle } from 'react-icons/fi'
import toast from 'react-hot-toast'

interface ErrorDisplayProps {
  error?: string
  stderr?: string
  exitCode?: number
}

export const ErrorDisplay: React.FC<ErrorDisplayProps> = ({ error, stderr, exitCode }) => {
  const { t } = useTranslation('tools')
  const [copied, setCopied] = useState(false)

  // 表示するエラー内容を決定
  const errorContent = error || stderr

  const handleCopy = async () => {
    if (!errorContent) return

    try {
      await navigator.clipboard.writeText(errorContent)
      setCopied(true)
      toast.success(
        t('code interpreter display.Error copied to clipboard', 'Error copied to clipboard')
      )
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy error:', err)
      toast.error(t('code interpreter display.Failed to copy error', 'Failed to copy error'))
    }
  }

  // Pythonエラーのスタックトレースを解析してハイライト
  const formatPythonError = (errorText: string) => {
    const lines = errorText.split('\n')

    return lines.map((line, index) => {
      // Traceback行の判定
      if (line.trim().startsWith('Traceback')) {
        return (
          <div key={index} className="text-danger font-semibold">
            {line}
          </div>
        )
      }

      // ファイル行の判定
      if (line.trim().startsWith('File ')) {
        return (
          <div key={index} className="text-accent">
            {line}
          </div>
        )
      }

      // エラータイプ行の判定（例: ValueError:, TypeError: など）
      if (line.match(/^\w+Error:/)) {
        return (
          <div key={index} className="text-danger font-medium">
            {line}
          </div>
        )
      }

      // その他の行
      return (
        <div key={index} className="text-ink-faint">
          {line}
        </div>
      )
    })
  }

  if (!errorContent && (!exitCode || exitCode === 0)) {
    return null
  }

  return (
    <div className="bg-danger-soft border border-danger rounded-container overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-2.5 py-1.5 bg-danger-soft border-b border-danger">
        <div className="flex items-center gap-2">
          <FiAlertTriangle className="text-danger" />
          <span className="text-sm font-medium text-danger">
            {t('code interpreter display.Execution Error', 'Execution Error')}
          </span>
          {exitCode !== undefined && exitCode !== 0 && (
            <span className="text-xs bg-danger-soft text-danger px-2 py-1 rounded-control">
              {t('code interpreter display.Exit Code', 'Exit Code')}: {exitCode}
            </span>
          )}
        </div>

        {errorContent && (
          <button
            onClick={handleCopy}
            className="flex items-center gap-1 px-2 py-1 text-xs text-danger hover:text-danger-strong hover:bg-danger-soft rounded-control transition-colors"
            title={t('code interpreter display.Copy error', 'Copy error')}
          >
            {copied ? (
              <>
                <FiCheck className="size-3" />
                <span>{t('code interpreter display.Copied', 'Copied')}</span>
              </>
            ) : (
              <>
                <FiCopy className="size-3" />
                <span>{t('code interpreter display.Copy', 'Copy')}</span>
              </>
            )}
          </button>
        )}
      </div>

      {/* Error content */}
      {errorContent && (
        <div className="p-2.5 max-h-[30vh] overflow-auto">
          <div className="bg-sunken text-ink p-2.5 rounded-control font-mono text-sm leading-relaxed">
            {formatPythonError(errorContent)}
          </div>
        </div>
      )}

      {/* Error hints */}
      <div className="px-2.5 py-1.5 bg-danger-soft border-t border-danger">
        <div className="text-xs text-danger">
          <span className="font-medium">{t('code interpreter display.Hint', 'Hint')}:</span>
          <span className="ml-1">
            {t(
              'code interpreter display.Check your code for syntax errors, undefined variables, or incorrect function calls.',
              'Check your code for syntax errors, undefined variables, or incorrect function calls.'
            )}
          </span>
        </div>
      </div>
    </div>
  )
}
