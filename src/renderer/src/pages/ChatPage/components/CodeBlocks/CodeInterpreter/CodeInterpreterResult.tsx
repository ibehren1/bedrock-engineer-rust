import React, { useState } from 'react'
import { FiFileText, FiBarChart2, FiFolder, FiAlertTriangle } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { ExecutedCodeBlock } from './ExecutedCodeBlock'
import { OutputDisplay } from './OutputDisplay'
import { FileDisplay } from './FileDisplay'
import { ErrorDisplay } from './ErrorDisplay'
import { ExecutionMetadata } from './ExecutionMetadata'
import { FaCode, FaCheckCircle, FaExclamationCircle } from 'react-icons/fa'

interface CodeInterpreterResult {
  success: boolean
  name: string
  code: string // The executed code
  message: string
  output: string
  error?: string
  executionTime: number
  result: {
    code: string // The executed code (also in result for backward compatibility)
    stdout: string
    stderr: string
    exitCode: number
    files: string[]
  }
}

interface CodeInterpreterResultProps {
  response: CodeInterpreterResult
}

export const CodeInterpreterResult: React.FC<CodeInterpreterResultProps> = ({ response }) => {
  const { t } = useTranslation('tools')
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({
    code: false,
    output: true, // Default to expanded
    files: true, // Always expanded
    errors: false
  })

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => ({
      ...prev,
      [section]: !prev[section]
    }))
  }

  const hasFiles = response.result.files && response.result.files.length > 0
  const hasError = !response.success || response.result.stderr
  const hasOutput = response.result.stdout || response.output

  return (
    <div className="bg-surface border border-subtle rounded-container shadow-sm overflow-hidden">
      {/* Header */}
      <div className="bg-surface-2 px-2.5 py-1.5 border-b border-subtle">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <FaCode className="text-success size-4" />
            <span className="font-semibold text-ink">
              {t('code interpreter display.Code Interpreter', 'Code Interpreter')}
            </span>
            <div className="flex items-center gap-2">
              {response.success ? (
                <FaCheckCircle className="text-success size-4" />
              ) : (
                <FaExclamationCircle className="text-danger size-4" />
              )}
              <span
                className={`text-xs px-2 py-1 rounded-full font-medium ${
                  response.success ? 'bg-success-soft text-success' : 'bg-danger-soft text-danger'
                }`}
              >
                {response.success
                  ? t('code interpreter display.Success', 'Success')
                  : t('code interpreter display.Failed', 'Failed')}
              </span>
            </div>
          </div>
          <ExecutionMetadata
            executionTime={response.executionTime}
            exitCode={response.result.exitCode}
            fileCount={response.result.files?.length || 0}
          />
        </div>
      </div>

      {/* Content */}
      <div className="divide-y divide-subtle">
        {/* Executed Code */}
        {response.code && (
          <div className="p-2.5">
            <button
              onClick={() => toggleSection('code')}
              className="flex items-center gap-2 w-full text-left text-sm font-medium text-ink hover:text-ink mb-3"
            >
              <FiFileText className="w-3.5 h-3.5" />
              <span>{t('code interpreter display.Executed Code', 'Executed Code')}</span>
              <span className="ml-auto text-xs text-ink-muted">
                {expandedSections.code ? '▼' : '▶'}
              </span>
            </button>
            {expandedSections.code && <ExecutedCodeBlock code={response.code} />}
          </div>
        )}

        {/* Output */}
        {hasOutput && (
          <div className="p-2.5">
            <button
              onClick={() => toggleSection('output')}
              className="flex items-center gap-2 w-full text-left text-sm font-medium text-ink hover:text-ink mb-3"
            >
              <FiBarChart2 className="w-3.5 h-3.5" />
              <span>{t('code interpreter display.Output', 'Output')}</span>
              <span className="ml-auto text-xs text-ink-muted">
                {expandedSections.output ? '▼' : '▶'}
              </span>
            </button>
            {expandedSections.output && (
              <OutputDisplay stdout={response.result.stdout} output={response.output} />
            )}
          </div>
        )}

        {/* Generated Files */}
        {hasFiles && (
          <div className="p-2.5">
            <div className="flex items-center gap-2 text-sm font-medium text-ink mb-3">
              <FiFolder className="w-3.5 h-3.5" />
              <span>
                {t('code interpreter display.Generated Files', 'Generated Files')} (
                {response.result.files.length})
              </span>
            </div>
            <FileDisplay files={response.result.files} />
          </div>
        )}

        {/* Errors */}
        {hasError && (
          <div className="p-2.5">
            <button
              onClick={() => toggleSection('errors')}
              className="flex items-center gap-2 w-full text-left text-sm font-medium text-ink hover:text-ink mb-3"
            >
              <FiAlertTriangle className="w-3.5 h-3.5" />
              <span>{t('code interpreter display.Errors', 'Errors')}</span>
              <span className="ml-auto text-xs text-ink-muted">
                {expandedSections.errors ? '▼' : '▶'}
              </span>
            </button>
            {expandedSections.errors && (
              <ErrorDisplay
                error={response.error}
                stderr={response.result.stderr}
                exitCode={response.result.exitCode}
              />
            )}
          </div>
        )}
      </div>
    </div>
  )
}
