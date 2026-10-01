import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { FaDocker } from 'react-icons/fa'
import { useSettings } from '@renderer/contexts/SettingsContext'

interface SandboxAvailability {
  dockerInstalled: boolean
  dockerVersion?: string
  daemonRunning: boolean
  compose: 'plugin' | 'standalone' | 'none'
  composeVersion?: string
  error?: string
  installGuidance?: string
  lastChecked?: Date | string
}

const MEMORY_OPTIONS = [
  { value: '256m', label: '256 MB' },
  { value: '512m', label: '512 MB' },
  { value: '1g', label: '1 GB' },
  { value: '2g', label: '2 GB' },
  { value: '4g', label: '4 GB' },
  { value: '8g', label: '8 GB' }
]

const CPU_OPTIONS = [
  { value: 0.5, label: '0.5 CPU (50%)' },
  { value: 1.0, label: '1.0 CPU (100%)' },
  { value: 2.0, label: '2.0 CPU (200%)' },
  { value: 4.0, label: '4.0 CPU (400%)' }
]

const TIMEOUT_OPTIONS = [
  { value: 60, label: '60s' },
  { value: 300, label: '300s (5 min)' },
  { value: 600, label: '600s (10 min)' },
  { value: 1800, label: '1800s (30 min)' }
]

export const DockerSandboxSettingForm: React.FC = () => {
  const { t } = useTranslation()
  const { dockerSandboxConfig, setDockerSandboxConfig } = useSettings()
  const [availability, setAvailability] = useState<SandboxAvailability | null>(null)
  const [isChecking, setIsChecking] = useState(false)

  // Probe once when the form opens. Each probe spawns docker child processes, so the
  // re-check button covers Docker being started later rather than polling.
  useEffect(() => {
    void checkAvailability()
  }, [])

  const checkAvailability = async () => {
    if (isChecking) return

    setIsChecking(true)
    try {
      const result = await window.api.dockerSandbox.availability(true)
      setAvailability(result)
    } catch (error) {
      setAvailability({
        dockerInstalled: false,
        daemonRunning: false,
        compose: 'none',
        error: error instanceof Error ? error.message : String(error),
        lastChecked: new Date()
      })
    } finally {
      setIsChecking(false)
    }
  }

  const handleConfigChange = (
    field: 'memoryLimit' | 'cpuLimit' | 'timeout',
    value: string | number
  ) => {
    setDockerSandboxConfig({ ...dockerSandboxConfig, [field]: value })
  }

  const dockerReady = !!availability?.dockerInstalled && !!availability?.daemonRunning

  const dockerStatusColor = () => {
    if (isChecking) return 'bg-warning-soft'
    return dockerReady ? 'bg-success-soft' : 'bg-danger-soft'
  }

  const dockerStatusText = () => {
    if (isChecking) return t('Checking...')
    if (!availability) return t('Not checked yet')
    if (!availability.dockerInstalled) return t('Docker not installed')
    if (!availability.daemonRunning) return t('Docker daemon not running')
    return availability.dockerVersion ? `Docker ${availability.dockerVersion}` : t('Available')
  }

  const composeStatusColor = () => {
    if (isChecking) return 'bg-warning-soft'
    if (!availability) return 'bg-sunken'
    return availability.compose === 'none' ? 'bg-warning-soft' : 'bg-success-soft'
  }

  const composeStatusText = () => {
    if (isChecking) return t('Checking...')
    if (!availability) return t('Not checked yet')
    if (availability.compose === 'none') return t('Not installed (single container only)')
    const flavor = availability.compose === 'plugin' ? 'docker compose' : 'docker-compose'
    return availability.composeVersion ? `${flavor} ${availability.composeVersion}` : flavor
  }

  const lastChecked = availability?.lastChecked ? new Date(availability.lastChecked) : undefined

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-base font-bold text-ink mb-2 flex items-center gap-2">
          <FaDocker className="text-[#2496ED]" />
          {t('Docker Sandbox')}
        </h3>
        <p className="text-sm text-ink-muted">{t('dockerSandbox.settings.intro')}</p>
      </div>

      {/* Docker / Compose status */}
      <div className="bg-surface-2 p-2.5 rounded-control space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-ink">{t('Environment')}</span>
          <button
            onClick={checkAvailability}
            disabled={isChecking}
            className="px-3 py-1 text-xs bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong disabled:opacity-50"
          >
            {isChecking ? t('Checking...') : t('Re-check')}
          </button>
        </div>

        <div className="flex items-center space-x-2">
          <div
            className={`w-3 h-3 rounded-full ${dockerStatusColor()} ${isChecking ? 'animate-pulse' : ''}`}
          />
          <span className="text-sm text-ink-muted">Docker: {dockerStatusText()}</span>
        </div>

        <div className="flex items-center space-x-2">
          <div
            className={`w-3 h-3 rounded-full ${composeStatusColor()} ${isChecking ? 'animate-pulse' : ''}`}
          />
          <span className="text-sm text-ink-muted">Compose: {composeStatusText()}</span>
        </div>

        {lastChecked && (
          <p className="text-xs text-ink-muted">
            {t('Last checked')}: {lastChecked.toLocaleTimeString()}
          </p>
        )}

        {availability?.installGuidance && (
          <div className="mt-2 p-3 bg-warning-soft border border-warning rounded-control text-sm text-warning whitespace-pre-line">
            {availability.installGuidance}
          </div>
        )}
      </div>

      {/* Resource limits */}
      <div className="bg-accent-tint p-2.5 rounded-control">
        <h5 className="font-medium mb-4 text-accent">{t('Container Limits')}</h5>

        <div className="space-y-2">
          <div>
            <label className="block text-sm font-medium text-ink mb-1">{t('Memory limit')}</label>
            <select
              value={dockerSandboxConfig.memoryLimit}
              onChange={(e) => handleConfigChange('memoryLimit', e.target.value)}
              className="w-full p-2 text-sm border border-strong rounded-control bg-surface text-ink"
            >
              {MEMORY_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-ink mb-1">{t('CPU limit')}</label>
            <select
              value={dockerSandboxConfig.cpuLimit}
              onChange={(e) => handleConfigChange('cpuLimit', Number(e.target.value))}
              className="w-full p-2 text-sm border border-strong rounded-control bg-surface text-ink"
            >
              {CPU_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-ink mb-1">
              {t('Command timeout')}
            </label>
            <select
              value={dockerSandboxConfig.timeout}
              onChange={(e) => handleConfigChange('timeout', Number(e.target.value))}
              className="w-full p-2 text-sm border border-strong rounded-control bg-surface text-ink"
            >
              {TIMEOUT_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <p className="mt-1 text-xs text-ink-muted">{t('dockerSandbox.settings.timeoutHint')}</p>
          </div>
        </div>

        <p className="mt-4 text-xs text-accent">{t('dockerSandbox.settings.limitsHint')}</p>
      </div>

      {/* How it behaves */}
      <div className="bg-surface-2 p-2.5 rounded-control">
        <h5 className="font-medium mb-2 text-ink">{t('How it works')}</h5>
        <ul className="list-disc list-inside space-y-1 text-sm text-ink-muted">
          <li>{t('dockerSandbox.settings.behaviorPerChat')}</li>
          <li>{t('dockerSandbox.settings.behaviorWorkspace')}</li>
          <li>{t('dockerSandbox.settings.behaviorHost')}</li>
          <li>{t('dockerSandbox.settings.behaviorLifecycle')}</li>
          <li>{t('dockerSandbox.settings.behaviorFiles')}</li>
        </ul>
      </div>
    </div>
  )
}
