import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { DockerEnvironmentStatus } from '@renderer/components/DockerSandbox/DockerEnvironmentStatus'

export const SandboxEnvironmentSection = () => {
  const { t } = useTranslation()

  return (
    <SettingSection
      title={t('sandboxSettings.environment.title')}
      description={t('sandboxSettings.environment.description')}
    >
      <DockerEnvironmentStatus />
    </SettingSection>
  )
}
