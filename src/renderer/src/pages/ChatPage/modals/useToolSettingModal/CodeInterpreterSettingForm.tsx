import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'

interface DockerStatus {
  available: boolean
  version?: string
  error?: string
  lastChecked?: Date
}

export const CodeInterpreterSettingForm: React.FC = () => {
  const { t } = useTranslation()
  const { codeInterpreterConfig, setCodeInterpreterConfig } = useSettings()
  const [dockerStatus, setDockerStatus] = useState<DockerStatus>({ available: false })
  const [isCheckingDocker, setIsCheckingDocker] = useState(false)

  // Memory limit options
  const memoryOptions = [
    { value: '128m', label: '128 MB' },
    { value: '256m', label: '256 MB' },
    { value: '512m', label: '512 MB' },
    { value: '1g', label: '1 GB' },
    { value: '2g', label: '2 GB' }
  ]

  // CPU limit options
  const cpuOptions = [
    { value: 0.5, label: '0.5 CPU (50%)' },
    { value: 1.0, label: '1.0 CPU (100%)' },
    { value: 2.0, label: '2.0 CPU (200%)' }
  ]

  // Timeout options
  const timeoutOptions = [
    { value: 30, label: '30秒' },
    { value: 60, label: '60秒' },
    { value: 120, label: '120秒' },
    { value: 300, label: '300秒 (5分)' }
  ]

  // Check Docker status once when the form opens. This used to re-check every 30
  // seconds, and each check spawns a `docker --version` child process; the
  // "再チェック" button below covers the case where Docker starts later.
  useEffect(() => {
    checkDockerStatus()
  }, [])

  const checkDockerStatus = async () => {
    if (isCheckingDocker) return

    setIsCheckingDocker(true)
    try {
      if (window.api?.codeInterpreter?.checkDockerAvailability) {
        const status = await window.api.codeInterpreter.checkDockerAvailability()
        setDockerStatus(status)
      }
    } catch (error) {
      setDockerStatus({
        available: false,
        error: error instanceof Error ? error.message : String(error),
        lastChecked: new Date()
      })
    } finally {
      setIsCheckingDocker(false)
    }
  }

  const handleConfigChange = (
    field: 'memoryLimit' | 'cpuLimit' | 'timeout',
    value: string | number
  ) => {
    const newConfig = {
      ...codeInterpreterConfig,
      [field]: value
    }
    setCodeInterpreterConfig(newConfig)
  }

  const getStatusColor = () => {
    if (isCheckingDocker) return 'bg-warning-soft'
    return dockerStatus.available ? 'bg-success-soft' : 'bg-danger-soft'
  }

  const getStatusText = () => {
    if (isCheckingDocker) return t('チェック中...')
    if (dockerStatus.available) {
      return dockerStatus.version ? `Docker ${dockerStatus.version}` : t('利用可能')
    }
    return dockerStatus.error || t('利用不可')
  }

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-base font-bold text-ink mb-2">{t('Code Interpreter')}</h3>
        <p className="text-sm text-ink-muted">
          {t(
            '安全なDocker環境でPythonコードを実行し、データ分析、計算、コード実行のためのコンテナ設定を行います。'
          )}
        </p>
      </div>

      {/* Docker Status Section */}
      <div className="bg-surface-2 p-2.5 rounded-control">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-2">
              <div
                className={`w-3 h-3 rounded-full ${getStatusColor()} ${isCheckingDocker ? 'animate-pulse' : ''}`}
                title={getStatusText()}
              />
              <span className="text-sm font-medium text-ink">{t('Docker ステータス')}</span>
            </div>
            <span className="text-sm text-ink-muted">{getStatusText()}</span>
          </div>
          <button
            onClick={checkDockerStatus}
            disabled={isCheckingDocker}
            className="px-3 py-1 text-xs bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong disabled:opacity-50"
          >
            {isCheckingDocker ? t('チェック中...') : t('再チェック')}
          </button>
        </div>

        {dockerStatus.lastChecked && (
          <p className="text-xs text-ink-muted">
            {t('最終チェック')}: {dockerStatus.lastChecked.toLocaleTimeString()}
          </p>
        )}

        {!dockerStatus.available && dockerStatus.error && (
          <div className="mt-2 p-2 bg-danger-soft border border-danger rounded-control text-sm text-danger">
            {dockerStatus.error}
          </div>
        )}
      </div>

      {/* Container Configuration Section */}
      <div className="bg-accent-tint p-2.5 rounded-control">
        <h5 className="font-medium mb-4 text-accent">{t('コンテナ設定')}</h5>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Memory Limit */}
          <div>
            <label className="block text-sm font-medium text-ink mb-2">{t('メモリ制限')}</label>
            <select
              value={codeInterpreterConfig.memoryLimit}
              onChange={(e) => handleConfigChange('memoryLimit', e.target.value)}
              className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-1 focus:ring-accent focus:border-accent bg-surface text-ink"
            >
              {memoryOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>

          {/* CPU Limit */}
          <div>
            <label className="block text-sm font-medium text-ink mb-2">{t('CPU制限')}</label>
            <select
              value={codeInterpreterConfig.cpuLimit}
              onChange={(e) => handleConfigChange('cpuLimit', parseFloat(e.target.value))}
              className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-1 focus:ring-accent focus:border-accent bg-surface text-ink"
            >
              {cpuOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>

          {/* Timeout */}
          <div>
            <label className="block text-sm font-medium text-ink mb-2">{t('タイムアウト')}</label>
            <select
              value={codeInterpreterConfig.timeout}
              onChange={(e) => handleConfigChange('timeout', parseInt(e.target.value))}
              className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-1 focus:ring-accent focus:border-accent bg-surface text-ink"
            >
              {timeoutOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="mt-4 p-3 bg-accent-tint rounded-control text-sm text-accent">
          <div className="flex items-start space-x-2">
            <span className="font-medium"></span>
            <div>
              <p className="font-medium mb-1">{t('設定のガイダンス')}:</p>
              <ul className="space-y-1 text-xs">
                <li>• {t('データサイエンス作業には256MB以上のメモリを推奨')}</li>
                <li>• {t('重い計算処理には1.0 CPU以上を推奨')}</li>
                <li>• {t('複雑な処理には120秒以上のタイムアウトを設定')}</li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      {/* Features & Capabilities Section */}
      <div className="bg-success-soft p-2.5 rounded-control">
        <h5 className="font-medium mb-2 text-success">{t('Features & Capabilities')}</h5>
        <ul className="text-sm text-ink space-y-2">
          <li className="flex items-start">
            <span className="text-success mr-2">•</span>
            {t('Execute Python code in a secure Docker container')}
          </li>
          <li className="flex items-start">
            <span className="text-success mr-2">•</span>
            {t('No internet access for enhanced security')}
          </li>
          <li className="flex items-start">
            <span className="text-success mr-2">•</span>
            {t('Automatic file generation and detection')}
          </li>
          <li className="flex items-start">
            <span className="text-success mr-2">•</span>
            {t('Support for data analysis and visualization')}
          </li>
          <li className="flex items-start">
            <span className="text-success mr-2">•</span>
            {t('Mathematical calculations and scientific computing')}
          </li>
        </ul>
      </div>

      {/* Security & Limitations Section */}
      <div className="bg-warning-soft p-2.5 rounded-control border border-warning">
        <h5 className="font-medium mb-2 text-warning">{t('Security & Limitations')}</h5>
        <ul className="text-sm text-ink space-y-2">
          <li className="flex items-start">
            <span className="text-warning mr-2">•</span>
            {t('Code runs in an isolated Docker environment')}
          </li>
          <li className="flex items-start">
            <span className="text-warning mr-2">•</span>
            {t('No network access to external resources')}
          </li>
          <li className="flex items-start">
            <span className="text-warning mr-2">•</span>
            {t('Generated files are temporary and may be cleared')}
          </li>
          <li className="flex items-start">
            <span className="text-warning mr-2">•</span>
            {t('設定されたリソース制限内で実行')}
          </li>
        </ul>
      </div>

      {/* Usage Examples Section */}
      <div className="bg-surface-2 p-2.5 rounded-control">
        <h5 className="font-medium mb-2 text-ink">{t('Usage Examples')}</h5>
        <div className="text-sm text-ink space-y-2">
          <p>
            <strong>{t('Data Analysis:')}</strong>{' '}
            {t('Process CSV files, perform statistical analysis')}
          </p>
          <p>
            <strong>{t('Visualization:')}</strong>{' '}
            {t('Create charts and graphs using matplotlib or plotly')}
          </p>
          <p>
            <strong>{t('Math & Science:')}</strong>{' '}
            {t('Solve complex equations, numerical simulations')}
          </p>
          <p>
            <strong>{t('File Processing:')}</strong> {t('Generate reports, process text files')}
          </p>
        </div>
      </div>
    </div>
  )
}
