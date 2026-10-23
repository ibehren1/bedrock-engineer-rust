import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'

export interface SandboxAvailability {
  dockerInstalled: boolean
  dockerVersion?: string
  daemonRunning: boolean
  compose: 'plugin' | 'standalone' | 'none'
  composeVersion?: string
  error?: string
  installGuidance?: string
  lastChecked?: Date | string
}

/**
 * Whether Docker and Compose are usable, with a re-check button and install guidance. Shared by
 * the Docker Sandbox tool settings and the Sandbox settings tab.
 */
export const DockerEnvironmentStatus: React.FC = () => {
  const { t } = useTranslation()
  const [availability, setAvailability] = useState<SandboxAvailability | null>(null)
  const [isChecking, setIsChecking] = useState(false)

  // Probe once when shown. Each probe spawns docker child processes, so the
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
  )
}
