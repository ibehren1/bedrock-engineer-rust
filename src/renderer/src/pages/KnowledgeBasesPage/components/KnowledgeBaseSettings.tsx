import React from 'react'
import type { KnowledgeSettings } from '@/types/knowledge'
import { KnowledgeAutoSyncSection } from './settings/KnowledgeAutoSyncSection'
import { KnowledgeCrawlerSection } from './settings/KnowledgeCrawlerSection'
import { KnowledgeEmbeddingSection } from './settings/KnowledgeEmbeddingSection'
import { KnowledgeRetrievalSection } from './settings/KnowledgeRetrievalSection'
import { KnowledgeStorageSection } from './settings/KnowledgeStorageSection'

/**
 * The Knowledge Bases page's Settings view: the overview, then the settings every knowledgebase
 * uses. The page holds one copy of the settings for every section, so an edit in one doesn't
 * overwrite another's and the list view sees it.
 */
export const KnowledgeBaseSettings: React.FC<{
  settings: KnowledgeSettings
  setSettings: (next: KnowledgeSettings) => void
}> = ({ settings, setSettings }) => {
  return (
    <div className="flex flex-col gap-4">
      <KnowledgeStorageSection />
      <KnowledgeEmbeddingSection settings={settings} onChange={setSettings} />
      <KnowledgeRetrievalSection settings={settings} onChange={setSettings} />
      <KnowledgeAutoSyncSection settings={settings} onChange={setSettings} />
      <KnowledgeCrawlerSection settings={settings} onChange={setSettings} />
    </div>
  )
}
