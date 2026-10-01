import React from 'react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'
import { SettingSelect } from '../SettingSelect'

export const ProxySection: React.FC = () => {
  const { t } = useTranslation()
  const { proxySettings, setProxySettings } = useSettings()

  return (
    <SettingSection title={t('Proxy Settings')}>
      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <label className="inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              className="form-checkbox h-4 w-4 text-accent rounded-control border-strong
                focus:ring-accent ring-offset-surface
                focus:ring-2 bg-raised border-subtle"
              checked={proxySettings.enabled}
              onChange={(e) => setProxySettings({ enabled: e.target.checked })}
            />
            <span className="ml-2 text-sm text-ink">{t('Enable Proxy')}</span>
          </label>
        </div>

        {proxySettings.enabled && (
          <div className="space-y-2 p-2.5 border border-subtle rounded-control">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <SettingSelect
                label={t('Protocol')}
                value={proxySettings.protocol}
                options={[
                  { value: 'http', label: 'HTTP' },
                  { value: 'https', label: 'HTTPS' }
                ]}
                onChange={(e) => setProxySettings({ protocol: e.target.value as 'http' | 'https' })}
              />

              <SettingInput
                label={t('Port')}
                placeholder="8080"
                value={proxySettings.port.toString()}
                onChange={(e) => setProxySettings({ port: parseInt(e.target.value) || 8080 })}
              />
            </div>

            <SettingInput
              label={t('Proxy Host')}
              type="string"
              placeholder="proxy.example.com"
              value={proxySettings.host}
              onChange={(e) => setProxySettings({ host: e.target.value })}
            />

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <SettingInput
                label={t('Username (optional)')}
                type="string"
                placeholder={t('Enter username')}
                value={proxySettings.username}
                onChange={(e) => setProxySettings({ username: e.target.value })}
              />

              <SettingInput
                label={t('Password (optional)')}
                type="password"
                placeholder={t('Enter password')}
                value={proxySettings.password}
                onChange={(e) => setProxySettings({ password: e.target.value })}
              />
            </div>

            <div className="mt-4 p-3 bg-accent-tint rounded-control">
              <p className="text-sm text-accent">
                {t(
                  'Proxy settings will be applied to all AWS SDK connections. Please test your configuration to ensure connectivity.'
                )}{' '}
                {t(
                  'Proxy changes apply to AWS and tool connections right away. Images and embedded pages in the app (website previews, diagrams) use them after you restart the app.'
                )}
              </p>
            </div>
          </div>
        )}
      </div>
    </SettingSection>
  )
}
