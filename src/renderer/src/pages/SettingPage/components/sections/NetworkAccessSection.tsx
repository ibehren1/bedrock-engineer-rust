import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'

/**
 * `blockLocalNetworkAccess`: when on, the agent's web tools (fetchWebsite, the knowledge base
 * website crawler) may not reach this computer or private networks. Enforced in Rust by
 * `tools::web::net_guard`; cloud metadata addresses are refused either way.
 */
export const NetworkAccessSection: React.FC = () => {
  const { t } = useTranslation()
  const [blocked, setBlocked] = useState<boolean>(
    () => window.store.get('blockLocalNetworkAccess') ?? false
  )

  const toggle = (value: boolean) => {
    setBlocked(value)
    window.store.set('blockLocalNetworkAccess', value)
  }

  return (
    <SettingSection title={t('networkAccess.title')} description={t('networkAccess.description')}>
      <div className="flex items-center space-x-2">
        <input
          type="checkbox"
          id="block-local-network"
          checked={blocked}
          onChange={(e) => toggle(e.target.checked)}
          className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
        />
        <label htmlFor="block-local-network" className="text-sm text-ink">
          {t('networkAccess.block')}
        </label>
      </div>
      <div className="flex flex-col gap-2 text-xs text-ink-muted">
        <p>{t('networkAccess.why')}</p>
        <p>{t('networkAccess.when')}</p>
        <p>{t('networkAccess.always')}</p>
      </div>
    </SettingSection>
  )
}
