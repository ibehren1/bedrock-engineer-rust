import React from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { FaDocker } from 'react-icons/fa'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { DockerEnvironmentStatus } from '@renderer/components/DockerSandbox/DockerEnvironmentStatus'
import {
  CPU_OPTIONS,
  MEMORY_OPTIONS,
  TIMEOUT_OPTIONS
} from '@renderer/components/DockerSandbox/sandboxOptions'
import { DEFAULT_SANDBOX_OS, SANDBOX_OS_OPTIONS } from '@/types/sandbox'

const labelOf = <T,>(options: readonly { value: T; label: string }[], value: T) =>
  options.find((option) => option.value === value)?.label ?? String(value)

interface DockerSandboxSettingFormProps {
  /** Called before leaving for the Sandbox settings tab, so the modal can close. */
  onOpenSettings?: () => void
}

/**
 * The Docker Sandbox tool's panel: environment status, a summary of the sandbox settings and a
 * link to the Settings page's Sandbox tab, where they are changed.
 */
export const DockerSandboxSettingForm: React.FC<DockerSandboxSettingFormProps> = ({
  onOpenSettings
}) => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { dockerSandboxConfig } = useSettings()

  const summary = t('sandboxSettings.toolSummary', {
    os: labelOf(SANDBOX_OS_OPTIONS, dockerSandboxConfig.os ?? DEFAULT_SANDBOX_OS),
    memory: labelOf(MEMORY_OPTIONS, dockerSandboxConfig.memoryLimit),
    cpu: labelOf(CPU_OPTIONS, dockerSandboxConfig.cpuLimit),
    timeout: labelOf(TIMEOUT_OPTIONS, dockerSandboxConfig.timeout)
  })

  const openSettings = () => {
    onOpenSettings?.()
    navigate('/setting/sandbox')
  }

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-base font-bold text-ink mb-2 flex items-center gap-2">
          <FaDocker className="text-[#2496ED]" />
          {t('Docker Sandbox')}
        </h3>
        <p className="text-sm text-ink-muted">{t('dockerSandbox.settings.intro')}</p>
      </div>

      <DockerEnvironmentStatus />

      {/* Settings live on the Settings page's Sandbox tab */}
      <div className="bg-accent-tint p-2.5 rounded-control space-y-2">
        <p className="text-sm text-ink">{summary}</p>
        <button
          onClick={openSettings}
          className="px-3 py-1 text-xs bg-surface text-accent border border-accent rounded-control hover:bg-accent-tint-strong"
        >
          {t('sandboxSettings.openSettings')}
        </button>
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
