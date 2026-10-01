import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiClock, FiHash, FiFile } from 'react-icons/fi'

interface ExecutionMetadataProps {
  executionTime: number
  exitCode: number
  fileCount: number
}

export const ExecutionMetadata: React.FC<ExecutionMetadataProps> = ({
  executionTime,
  exitCode,
  fileCount
}) => {
  const { t } = useTranslation('tools')

  // 実行時間を適切な単位で表示
  const formatExecutionTime = (timeInMs: number) => {
    if (timeInMs < 1000) {
      return `${timeInMs}ms`
    } else {
      return `${(timeInMs / 1000).toFixed(2)}s`
    }
  }

  return (
    <div className="flex items-center gap-4 text-xs text-ink-muted">
      {/* Execution Time */}
      <div
        className="flex items-center gap-1"
        title={t('code interpreter display.Execution time', 'Execution time')}
      >
        <FiClock className="size-3" />
        <span>{formatExecutionTime(executionTime)}</span>
      </div>

      {/* Exit Code */}
      <div
        className="flex items-center gap-1"
        title={t('code interpreter display.Exit code', 'Exit code')}
      >
        <FiHash className="size-3" />
        <span className={exitCode === 0 ? 'text-success' : 'text-danger'}>{exitCode}</span>
      </div>

      {/* File Count */}
      {fileCount > 0 && (
        <div
          className="flex items-center gap-1"
          title={t('code interpreter display.Generated files', 'Generated files')}
        >
          <FiFile className="size-3" />
          <span>{fileCount}</span>
        </div>
      )}
    </div>
  )
}
