import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import useSetting from '@renderer/hooks/useSetting'

export const NotificationSection = () => {
  const { t } = useTranslation()
  const { notification, setNotification } = useSetting()

  return (
    <SettingSection title={t('notification.title')} description={t('notification.description')}>
      <div className="flex items-center space-x-2">
        <input
          type="checkbox"
          id="enable-notification"
          checked={notification}
          onChange={(e) => setNotification(e.target.checked)}
          className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
        />
        <label htmlFor="enable-notification" className="text-sm text-ink">
          {t('notification.enable')}
        </label>
      </div>
    </SettingSection>
  )
}
