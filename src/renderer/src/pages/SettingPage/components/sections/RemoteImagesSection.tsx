import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { REMOTE_IMAGES_SETTING } from '@renderer/components/RemoteImage'
import { SettingSection } from '../SettingSection'

/** `autoLoadRemoteImages` (off by default): see `RemoteImage`. */
export const RemoteImagesSection: React.FC = () => {
  const { t } = useTranslation()
  const [auto, setAuto] = useState<boolean>(() => !!window.store.get(REMOTE_IMAGES_SETTING))

  return (
    <SettingSection title={t('remoteImages.title')} description={t('remoteImages.description')}>
      <div className="flex items-center space-x-2">
        <input
          type="checkbox"
          id="auto-load-remote-images"
          checked={auto}
          onChange={(e) => {
            setAuto(e.target.checked)
            window.store.set(REMOTE_IMAGES_SETTING, e.target.checked)
          }}
          className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
        />
        <label htmlFor="auto-load-remote-images" className="text-sm text-ink">
          {t('remoteImages.autoLoad')}
        </label>
      </div>
      <p className="text-xs text-ink-muted">{t('remoteImages.why')}</p>
    </SettingSection>
  )
}
