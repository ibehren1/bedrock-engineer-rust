import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcFolder } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'

export const ConfigDirSection: React.FC = () => {
  const { t } = useTranslation()
  const { userDataPath } = useSettings()

  return (
    <SettingSection title={t('Config Directory')}>
      <label className="block text-md font-medium text-ink">
        <div>
          <span className="text-xs text-ink-muted">{t('Config Directory Description')}</span>
        </div>
        <div className="flex gap-2 items-center">
          <FcFolder className="text-base" />
          <span className="cursor-text">{userDataPath}</span>
        </div>
      </label>
    </SettingSection>
  )
}
