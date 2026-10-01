import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcFolder } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'

export const ProjectSection: React.FC = () => {
  const { t } = useTranslation()
  const { projectPath, selectDirectory } = useSettings()

  return (
    <SettingSection title={t('Project Setting')}>
      <label
        onClick={selectDirectory}
        className="block text-md font-medium text-ink
          cursor-pointer hover:text-ink-muted transition-colors"
      >
        <div className="flex gap-2 items-center">
          <FcFolder className="text-base" />
          <span>{projectPath || t('Select Project Directory')}</span>
        </div>
      </label>
    </SettingSection>
  )
}
