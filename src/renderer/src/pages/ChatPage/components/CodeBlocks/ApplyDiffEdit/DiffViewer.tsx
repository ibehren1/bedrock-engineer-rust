import React, { useMemo, useState } from 'react'
import { DiffEditor } from '@monaco-editor/react'
import '@renderer/lib/monaco/setup'
import { useTranslation } from 'react-i18next'
import { FiCopy, FiMaximize2, FiMinimize2 } from 'react-icons/fi'
import toast from 'react-hot-toast'
import { DiffViewerProps } from './types'
import { useTheme } from '@renderer/hooks/useTheme'

/**
 * Get language from file extension
 */
const getLanguageFromPath = (filePath: string): string => {
  const extension = filePath.split('.').pop()?.toLowerCase()

  const languageMap: Record<string, string> = {
    ts: 'typescript',
    tsx: 'typescript',
    js: 'javascript',
    jsx: 'javascript',
    py: 'python',
    java: 'java',
    cpp: 'cpp',
    c: 'c',
    cs: 'csharp',
    php: 'php',
    rb: 'ruby',
    go: 'go',
    rs: 'rust',
    kt: 'kotlin',
    swift: 'swift',
    css: 'css',
    scss: 'scss',
    less: 'less',
    html: 'html',
    xml: 'xml',
    json: 'json',
    yaml: 'yaml',
    yml: 'yaml',
    md: 'markdown',
    sql: 'sql',
    sh: 'shell',
    bash: 'shell',
    zsh: 'shell',
    ps1: 'powershell',
    dockerfile: 'dockerfile'
  }

  return languageMap[extension || ''] || 'plaintext'
}

export const DiffViewer: React.FC<DiffViewerProps> = ({
  originalText,
  updatedText,
  filePath,
  language
}) => {
  const { t } = useTranslation()
  const { isDarkMode } = useTheme()
  const [isExpanded, setIsExpanded] = useState(false)

  const detectedLanguage = useMemo(() => {
    return language || getLanguageFromPath(filePath)
  }, [language, filePath])

  // Calculate diff statistics
  const diffStats = useMemo(() => {
    const originalLines = originalText.split('\n')
    const updatedLines = updatedText.split('\n')

    return {
      originalLines: originalLines.length,
      updatedLines: updatedLines.length,
      linesChanged: Math.abs(updatedLines.length - originalLines.length)
    }
  }, [originalText, updatedText])

  const handleCopyOriginal = () => {
    navigator.clipboard
      .writeText(originalText)
      .then(() => toast.success(t('Original text copied to clipboard')))
      .catch(() => toast.error(t('Failed to copy text')))
  }

  const handleCopyUpdated = () => {
    navigator.clipboard
      .writeText(updatedText)
      .then(() => toast.success(t('Updated text copied to clipboard')))
      .catch(() => toast.error(t('Failed to copy text')))
  }

  const handleCopyFilePath = () => {
    navigator.clipboard
      .writeText(filePath)
      .then(() => toast.success(t('File path copied to clipboard')))
      .catch(() => toast.error(t('Failed to copy file path')))
  }

  return (
    <div className="border rounded-container border-subtle bg-surface">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-subtle bg-surface-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-ink">{t('File Diff')}:</span>
          <button
            onClick={handleCopyFilePath}
            className="text-sm text-accent hover:underline font-mono truncate max-w-xs"
            title={filePath}
          >
            {filePath}
          </button>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-ink-muted bg-raised px-2 py-1 rounded-control">
            {detectedLanguage}
          </span>
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-1 hover:bg-raised rounded-control"
            title={isExpanded ? t('Collapse') : t('Expand')}
          >
            {isExpanded ? <FiMinimize2 className="w-4 h-4" /> : <FiMaximize2 className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="flex items-center gap-4 px-3 py-2 text-xs text-ink-muted bg-surface-2 border-b border-subtle">
        <span>
          {t('Original')}: {diffStats.originalLines} {t('lines')}
        </span>
        <span>
          {t('Updated')}: {diffStats.updatedLines} {t('lines')}
        </span>
        <span>
          {t('Changed')}: {diffStats.linesChanged} {t('lines')}
        </span>
      </div>

      {/* Copy buttons */}
      <div className="flex gap-2 px-3 py-2 bg-surface-2 border-b border-subtle">
        <button
          onClick={handleCopyOriginal}
          className="flex items-center gap-1 text-xs px-2 py-1 bg-raised hover:bg-sunken rounded-control"
        >
          <FiCopy className="w-3 h-3" />
          {t('Copy Original')}
        </button>
        <button
          onClick={handleCopyUpdated}
          className="flex items-center gap-1 text-xs px-2 py-1 bg-accent-tint hover:bg-accent-tint-strong rounded-control"
        >
          <FiCopy className="w-3 h-3" />
          {t('Copy Updated')}
        </button>
      </div>

      {/* Diff Editor */}
      <div className={`${isExpanded ? 'h-96' : 'h-64'} transition-all duration-200`}>
        <DiffEditor
          original={originalText}
          modified={updatedText}
          language={detectedLanguage}
          // Was pinned to "vs-dark", so a dark diff sat inside a light UI in
          // every appearance except dark. Matches the other Monaco editors now.
          theme={isDarkMode ? 'vs-dark' : 'light'}
          options={{
            readOnly: true,
            renderSideBySide: true,
            enableSplitViewResizing: false,
            renderOverviewRuler: false,
            folding: false,
            lineNumbers: 'on',
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            wordWrap: 'on',
            automaticLayout: true,
            fontSize: 12,
            diffCodeLens: true,
            ignoreTrimWhitespace: false
          }}
        />
      </div>
    </div>
  )
}
