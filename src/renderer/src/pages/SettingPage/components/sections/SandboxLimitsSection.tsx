import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'
import { useSettings } from '@renderer/contexts/SettingsContext'
import {
  CPU_OPTIONS,
  MEMORY_OPTIONS,
  TIMEOUT_OPTIONS
} from '@renderer/components/DockerSandbox/sandboxOptions'

/** SettingSelect takes string values; the CPU and timeout settings are numbers. */
const asStrings = (options: { value: number; label: string }[]) =>
  options.map((option) => ({ value: String(option.value), label: option.label }))

export const SandboxLimitsSection = () => {
  const { t } = useTranslation()
  const { dockerSandboxConfig, setDockerSandboxConfig } = useSettings()

  return (
    <SettingSection
      title={t('sandboxSettings.limits.title')}
      description={t('dockerSandbox.settings.limitsHint')}
    >
      <SettingSelect
        label={t('sandboxSettings.limits.memory')}
        value={dockerSandboxConfig.memoryLimit}
        options={MEMORY_OPTIONS}
        onChange={(e) =>
          setDockerSandboxConfig({ ...dockerSandboxConfig, memoryLimit: e.target.value })
        }
      />
      <SettingSelect
        label={t('sandboxSettings.limits.cpu')}
        value={String(dockerSandboxConfig.cpuLimit)}
        options={asStrings(CPU_OPTIONS)}
        onChange={(e) =>
          setDockerSandboxConfig({ ...dockerSandboxConfig, cpuLimit: Number(e.target.value) })
        }
      />
      <SettingSelect
        label={t('sandboxSettings.limits.timeout')}
        description={t('dockerSandbox.settings.timeoutHint')}
        value={String(dockerSandboxConfig.timeout)}
        options={asStrings(TIMEOUT_OPTIONS)}
        onChange={(e) =>
          setDockerSandboxConfig({ ...dockerSandboxConfig, timeout: Number(e.target.value) })
        }
      />
    </SettingSection>
  )
}
