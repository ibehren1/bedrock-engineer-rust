import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import MD from '@renderer/components/Markdown/MD'
import { useAppUpdate } from '@renderer/contexts/AppUpdateContext'

export const UpdatesSection = () => {
  const { t } = useTranslation()
  const { status, currentVersion, autoUpdate, setAutoUpdate, checkNow, restart } = useAppUpdate()

  const message = (() => {
    switch (status.state) {
      case 'checking':
        return t('appUpdate.checking')
      case 'upToDate':
        return t('appUpdate.upToDate')
      case 'downloading':
        return t('appUpdate.downloading', { version: status.version })
      case 'ready':
        return t('appUpdate.ready', { version: status.version })
      case 'error':
        return t('appUpdate.failed', { error: status.message })
      default:
        return null
    }
  })()

  const busy = status.state === 'checking' || status.state === 'downloading'
  // The new version's release notes, while it downloads or waits for a restart.
  const pending =
    (status.state === 'downloading' || status.state === 'ready') && status.notes ? status : null

  return (
    <SettingSection title={t('appUpdate.title')} description={t('appUpdate.description')}>
      <div className="flex flex-col gap-3">
        {currentVersion && (
          <p className="m-0 text-sm text-ink">
            {t('appUpdate.currentVersion', { version: currentVersion })}
          </p>
        )}
        <div className="flex items-center space-x-2">
          <input
            type="checkbox"
            id="enable-auto-update"
            checked={autoUpdate}
            onChange={(e) => setAutoUpdate(e.target.checked)}
            className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
          />
          <label htmlFor="enable-auto-update" className="text-sm text-ink">
            {t('appUpdate.auto')}
          </label>
        </div>
        <div className="flex items-center gap-3">
          {status.state === 'ready' ? (
            <button
              onClick={() => void restart()}
              className="px-3 py-1 text-sm rounded-control bg-accent text-accent-fg hover:bg-accent-strong"
            >
              {t('appUpdate.restartNow')}
            </button>
          ) : (
            <button
              onClick={() => void checkNow()}
              disabled={busy}
              className="px-3 py-1 text-sm rounded-control border border-subtle text-ink hover:bg-raised disabled:opacity-50"
            >
              {t('appUpdate.checkNow')}
            </button>
          )}
          {message && (
            <span
              className={`text-xs ${status.state === 'error' ? 'text-danger' : 'text-ink-muted'}`}
            >
              {message}
            </span>
          )}
        </div>
        {pending && (
          <article className="rounded-control border border-subtle bg-surface px-4 py-3">
            <h3 className="m-0 mb-2 text-sm font-semibold text-ink">
              {t('appUpdate.pendingNotesTitle', { version: pending.version })}
            </h3>
            <div className="text-sm text-ink">
              <MD>{pending.notes}</MD>
            </div>
          </article>
        )}
      </div>
    </SettingSection>
  )
}
