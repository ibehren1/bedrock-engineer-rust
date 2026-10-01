import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiSun } from 'react-icons/fi'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'
import { useTheme, type AppTheme } from '@renderer/hooks/useTheme'
import { useFont, type AppFontSans, type AppFontMono } from '@renderer/hooks/useFont'

export const AppearanceSection: React.FC = () => {
  const { t } = useTranslation()
  const { appTheme, setAppTheme } = useTheme()
  const { fontSans, setFontSans, fontMono, setFontMono } = useFont()

  const themeOptions = [
    { value: 'light', label: t('appearance.themes.light') },
    { value: 'newspaper', label: t('appearance.themes.newspaper') },
    { value: 'dim', label: t('appearance.themes.dim') },
    { value: 'charcoal', label: t('appearance.themes.charcoal') },
    { value: 'dark', label: t('appearance.themes.dark') },
    { value: 'system', label: t('appearance.themes.system') }
  ]

  const fontSansOptions = [
    { value: 'inter', label: t('appearance.fontsSans.inter') },
    { value: 'geist', label: t('appearance.fontsSans.geist') },
    { value: 'system', label: t('appearance.fontsSans.system') }
  ]

  const fontMonoOptions = [
    { value: 'jetbrains', label: t('appearance.fontsMono.jetbrains') },
    { value: 'geist-mono', label: t('appearance.fontsMono.geistMono') },
    { value: 'system', label: t('appearance.fontsMono.system') }
  ]

  return (
    <SettingSection title={t('appearance.title')} icon={FiSun}>
      <SettingSelect
        label={t('appearance.theme')}
        description={t('appearance.description')}
        value={appTheme}
        options={themeOptions}
        onChange={(e) => setAppTheme(e.target.value as AppTheme)}
      />
      <SettingSelect
        label={t('appearance.fontSans')}
        description={t('appearance.fontSansDescription')}
        value={fontSans}
        options={fontSansOptions}
        onChange={(e) => setFontSans(e.target.value as AppFontSans)}
      />
      <SettingSelect
        label={t('appearance.fontMono')}
        description={t('appearance.fontMonoDescription')}
        value={fontMono}
        options={fontMonoOptions}
        onChange={(e) => setFontMono(e.target.value as AppFontMono)}
      />
    </SettingSection>
  )
}
