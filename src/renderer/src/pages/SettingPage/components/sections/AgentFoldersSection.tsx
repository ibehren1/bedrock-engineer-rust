import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FcFolder } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'

/**
 * Folders outside the project that the agents' file tools may also read and write
 * (`agentFileFolders`, enforced in Rust by `tools::util::path_guard`).
 */
export const AgentFoldersSection: React.FC = () => {
  const { t } = useTranslation()
  const { projectPath } = useSettings()
  const [folders, setFolders] = useState<string[]>(() => window.store.get('agentFileFolders') ?? [])

  const save = (next: string[]) => {
    setFolders(next)
    window.store.set('agentFileFolders', next)
  }

  const addFolder = async () => {
    const path = await window.file.handleFolderOpen()
    if (path && path !== projectPath && !folders.includes(path)) {
      save([...folders, path])
    }
  }

  return (
    <SettingSection title={t('agentFolders.title')} description={t('agentFolders.description')}>
      <ul className="flex flex-col gap-2">
        <li className="flex gap-2 items-center text-sm text-ink">
          <FcFolder className="text-base shrink-0" />
          <span className="break-all">{projectPath || t('agentFolders.noProject')}</span>
          <span className="text-xs text-ink-muted">{t('agentFolders.projectFolder')}</span>
        </li>
        {folders.map((folder) => (
          <li key={folder} className="flex gap-2 items-center text-sm text-ink">
            <FcFolder className="text-base shrink-0" />
            <span className="break-all">{folder}</span>
            <button
              onClick={() => save(folders.filter((f) => f !== folder))}
              className="ml-auto px-2 py-0.5 text-xs rounded-control border border-subtle text-ink hover:bg-raised"
            >
              {t('agentFolders.remove')}
            </button>
          </li>
        ))}
      </ul>
      <div>
        <button
          onClick={addFolder}
          className="px-3 py-1 text-sm rounded-control border border-subtle text-ink hover:bg-raised"
        >
          {t('agentFolders.add')}
        </button>
      </div>
      <p className="text-xs text-ink-muted">{t('agentFolders.alwaysRefused')}</p>
    </SettingSection>
  )
}
