import React, { useState, useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { vscDarkPlus, prism } from 'react-syntax-highlighter/dist/esm/styles/prism'
import { FiCopy, FiCheck } from 'react-icons/fi'
import toast from 'react-hot-toast'
import { isDarkAppearance } from '@renderer/lib/appearance'

// Constants
const COPY_FEEDBACK_TIMEOUT = 2000
const LANGUAGE = 'python'

// Style constants
const createLineNumberStyle = (isDark: boolean) => ({
  minWidth: '3em',
  paddingRight: '1em',
  paddingLeft: '0.5em',
  color: isDark ? '#9ca3af' : '#6b7280',
  fontSize: '0.875rem'
})

const createCustomSyntaxStyle = (isDark: boolean) => ({
  margin: 0,
  padding: '1rem',
  background: isDark ? '#1f2937' : '#f9fafb',
  fontSize: '0.875rem',
  lineHeight: '1.5',
  borderRadius: '0'
})

const CODE_TAG_STYLE = {
  fontSize: '0.875rem',
  fontFamily:
    'ui-monospace, SFMono-Regular, "SF Mono", Monaco, Inconsolata, "Roboto Mono", monospace'
} as const

// CSS class constants
const CSS_CLASSES = {
  container: 'relative bg-surface rounded-container overflow-hidden border border-subtle',
  header: 'flex items-center justify-between px-2.5 py-1 bg-surface-2 border-b border-subtle',
  languageLabel: 'text-xs text-ink-muted font-medium',
  copyButton:
    'flex items-center gap-1 px-2 py-1 text-xs text-ink-muted hover:text-ink hover:bg-raised rounded-control transition-colors',
  icon: 'size-3',
  // Written literally, never interpolated. Tailwind scans source text, so
  // building this from a constant produced a class that was never generated and
  // left the block with no height cap at all.
  codeContent: 'relative max-h-[40vh] overflow-auto'
} as const

interface ExecutedCodeBlockProps {
  /** The Python code to display */
  code: string
}

/**
 * ExecutedCodeBlock component displays executed Python code with syntax highlighting
 * and copy functionality
 */
export const ExecutedCodeBlock: React.FC<ExecutedCodeBlockProps> = ({ code }) => {
  const { t } = useTranslation('tools')
  const [copied, setCopied] = useState<boolean>(false)

  /**
   * Whether this block should render on a dark canvas.
   *
   * Was `classList.contains('dark') || prefers-color-scheme`, which was wrong
   * twice over: no `dark` class is ever set, so the first test was always false,
   * and the OS fallback then made the block follow the desktop rather than the
   * appearance chosen in the app.
   */
  const isDarkMode = useMemo(() => isDarkAppearance(), [])

  /**
   * Handles copying code to clipboard with user feedback
   */
  const handleCopy = useCallback(async (): Promise<void> => {
    if (!navigator.clipboard) {
      toast.error(t('code interpreter display.Clipboard not available', 'Clipboard not available'))
      return
    }

    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      toast.success(
        t('code interpreter display.Code copied to clipboard', 'Code copied to clipboard')
      )

      // Reset copied state after timeout
      setTimeout(() => setCopied(false), COPY_FEEDBACK_TIMEOUT)
    } catch (error) {
      console.error('Failed to copy code:', error)
      toast.error(t('code interpreter display.Failed to copy code', 'Failed to copy code'))
    }
  }, [code, t])

  /**
   * Renders the copy button with appropriate icon and text
   */
  const renderCopyButton = (): JSX.Element => (
    <button
      onClick={handleCopy}
      className={CSS_CLASSES.copyButton}
      title={t('code interpreter display.Copy code', 'Copy code')}
      aria-label={t('code interpreter display.Copy code', 'Copy code')}
    >
      {copied ? (
        <>
          <FiCheck className={CSS_CLASSES.icon} />
          <span>{t('code interpreter display.Copied', 'Copied')}</span>
        </>
      ) : (
        <>
          <FiCopy className={CSS_CLASSES.icon} />
          <span>{t('code interpreter display.Copy', 'Copy')}</span>
        </>
      )}
    </button>
  )

  /**
   * Renders the header section with language label and copy button
   */
  const renderHeader = (): JSX.Element => (
    <div className={CSS_CLASSES.header}>
      <div className="flex items-center gap-2">
        <span className={CSS_CLASSES.languageLabel}>{LANGUAGE}</span>
      </div>
      {renderCopyButton()}
    </div>
  )

  /**
   * Renders the syntax highlighted code content
   */
  const renderCodeContent = (): JSX.Element => (
    <div className={CSS_CLASSES.codeContent}>
      <SyntaxHighlighter
        language={LANGUAGE}
        style={isDarkMode ? vscDarkPlus : prism}
        showLineNumbers
        lineNumberStyle={createLineNumberStyle(isDarkMode)}
        customStyle={createCustomSyntaxStyle(isDarkMode)}
        codeTagProps={{ style: CODE_TAG_STYLE }}
      >
        {code}
      </SyntaxHighlighter>
    </div>
  )

  return (
    <div className={CSS_CLASSES.container}>
      {renderHeader()}
      {renderCodeContent()}
    </div>
  )
}
