import React from 'react'
import { useTranslation } from 'react-i18next'
import { FcGlobe } from 'react-icons/fc'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'

const languageOptions = [
  { value: 'en', label: 'English' },
  { value: 'ja', label: '日本語' }
]

export const LanguageSection: React.FC = () => {
  const { t, i18n } = useTranslation()

  // 言語は SettingsContext ではなく i18n と store が直接持っている
  const handleChangeLanguage = (language: 'ja' | 'en') => {
    i18n.changeLanguage(language)
    window.store.set('language', language)
  }

  return (
    <SettingSection title={t('Language')} icon={FcGlobe}>
      <SettingSelect
        label={t('Select Language')}
        value={i18n.language}
        options={languageOptions}
        onChange={(e) => handleChangeLanguage(e.target.value as 'ja' | 'en')}
      />
    </SettingSection>
  )
}
