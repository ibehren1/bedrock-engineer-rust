import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { useSettings } from '@renderer/contexts/SettingsContext'
import type { DockerSandboxConfig } from '@/types/sandbox'

type RestrictionKey = 'lockDown' | 'disableNetwork'

/**
 * Optional restrictions for new sandbox containers (`dockerSandboxTool.lockDown` /
 * `.disableNetwork`), both off by default and enforced by the compose writer in Rust. Each
 * switch lists what it turns off, because both get in the way of ordinary sandbox work.
 */
export const SandboxRestrictionsSection = () => {
  const { t } = useTranslation()
  const { dockerSandboxConfig, setDockerSandboxConfig } = useSettings()

  const restriction = (name: RestrictionKey, i18nKey: string) => {
    const checked = dockerSandboxConfig[name] ?? false
    const turnsOff = t(`sandboxSettings.restrictions.${i18nKey}.turnsOff`, {
      returnObjects: true
    }) as string[]
    return (
      <div className="flex flex-col gap-2 rounded-control border border-subtle p-3">
        <div className="flex items-center space-x-2">
          <input
            type="checkbox"
            id={`sandbox-${name}`}
            checked={checked}
            onChange={(e) =>
              setDockerSandboxConfig({
                ...dockerSandboxConfig,
                [name]: e.target.checked
              } as DockerSandboxConfig)
            }
            className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
          />
          <label htmlFor={`sandbox-${name}`} className="text-sm font-medium text-ink">
            {t(`sandboxSettings.restrictions.${i18nKey}.label`)}
          </label>
        </div>
        <p className="text-xs text-ink-muted">
          {t(`sandboxSettings.restrictions.${i18nKey}.protects`)}
        </p>
        <p className="text-xs font-medium text-ink">{t('sandboxSettings.restrictions.turnsOff')}</p>
        <ul className="ml-4 list-disc text-xs text-ink-muted">
          {turnsOff.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
    )
  }

  return (
    <SettingSection
      title={t('sandboxSettings.restrictions.title')}
      description={t('sandboxSettings.restrictions.description')}
    >
      {restriction('lockDown', 'lockDown')}
      {restriction('disableNetwork', 'network')}
    </SettingSection>
  )
}
