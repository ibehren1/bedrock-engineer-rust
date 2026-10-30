import React from 'react'
import { useKnowledgeSettings } from '@renderer/components/KnowledgeBase/knowledgeSettings'
import {
  KnowledgeAutoSyncSection,
  KnowledgeCrawlerSection,
  KnowledgeEmbeddingSection,
  KnowledgeRetrievalSection,
  KnowledgeStorageSection
} from '../sections'

export const KnowledgeBaseTab: React.FC = () => {
  // One copy of the settings for every section, so an edit in one doesn't overwrite another's.
  const [settings, setSettings] = useKnowledgeSettings()
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
