import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { DEFAULT_SANDBOX_OS, SANDBOX_OS_OPTIONS, SandboxOsId } from '@/types/sandbox'

/** i18n key of the note shown under the menu for each OS. */
const OS_NOTE_KEYS: Record<SandboxOsId, string> = {
  debian13: 'sandboxSettings.image.debian',
  ubuntu: 'sandboxSettings.image.ubuntu',
  amazonlinux2023: 'sandboxSettings.image.rhel',
  amazonlinux2027: 'sandboxSettings.image.rhel',
  almalinux9: 'sandboxSettings.image.rhel',
  rocky9: 'sandboxSettings.image.rhel',
  ubi9: 'sandboxSettings.image.ubi'
}

export const SandboxImageSection = () => {
  const { t } = useTranslation()
  const { dockerSandboxConfig, setDockerSandboxConfig } = useSettings()
  const os = dockerSandboxConfig.os ?? DEFAULT_SANDBOX_OS

  return (
    <SettingSection
      title={t('sandboxSettings.image.title')}
      description={t('sandboxSettings.image.description')}
    >
      <SettingSelect
        label={t('sandboxSettings.image.label')}
        value={os}
        options={SANDBOX_OS_OPTIONS.map((option) => ({ ...option }))}
        onChange={(e) =>
          setDockerSandboxConfig({ ...dockerSandboxConfig, os: e.target.value as SandboxOsId })
        }
      />
      <p className="text-xs text-ink-muted">{t(OS_NOTE_KEYS[os])}</p>
      <p className="text-xs text-ink-muted">{t('sandboxSettings.image.hint')}</p>
    </SettingSection>
  )
}
