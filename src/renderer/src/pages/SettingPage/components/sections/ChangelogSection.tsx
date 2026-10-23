import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import MD from '@renderer/components/Markdown/MD'
import { useAppUpdate } from '@renderer/contexts/AppUpdateContext'
import { SettingSection } from '../SettingSection'
import { parseChangelog, sectionsForVersion } from '../../utils/changelog'
// The changelog as of this build: its topmost dated section is this version's release notes.
import changelog from '@/../CHANGELOG.md?raw'

const ALL_SECTIONS = parseChangelog(changelog)

export const ChangelogSection = () => {
  const { t } = useTranslation()
  const { currentVersion } = useAppUpdate()
  const [showEarlier, setShowEarlier] = useState(false)

  const sections = useMemo(() => sectionsForVersion(ALL_SECTIONS, currentVersion), [currentVersion])
  if (sections.length === 0) return null
  const [latest, ...earlier] = sections

  return (
    <SettingSection
      title={
        currentVersion
          ? t('appUpdate.changelogTitle', { version: currentVersion })
          : t('appUpdate.changelogTitleNoVersion')
      }
      description={t('appUpdate.changelogDescription')}
    >
      <ChangelogEntry date={latest.date} body={latest.body} />
      {earlier.length > 0 && (
        <>
          <button
            onClick={() => setShowEarlier((value) => !value)}
            className="px-3 py-1 text-sm rounded-control border border-subtle text-ink hover:bg-raised"
          >
            {showEarlier ? t('appUpdate.hideEarlier') : t('appUpdate.showEarlier')}
          </button>
          {showEarlier &&
            earlier.map((section) => (
              <ChangelogEntry key={section.date} date={section.date} body={section.body} />
            ))}
        </>
      )}
    </SettingSection>
  )
}

const ChangelogEntry = ({ date, body }: { date: string; body: string }) => (
  <article className="rounded-control border border-subtle bg-surface px-4 py-3">
    <h3 className="m-0 mb-2 text-sm font-semibold text-ink">{date}</h3>
    <div className="text-sm text-ink">
      <MD>{body}</MD>
    </div>
  </article>
)
