import { useCallback, useEffect, useState } from 'react'
import {
  BEDROCK_EMBEDDING_MODELS,
  DEFAULT_KNOWLEDGE_SETTINGS,
  EmbeddingSpec,
  KnowledgeSettings,
  LOCAL_EMBEDDING_MODEL
} from '@/types/knowledge'

/** The stored `knowledgebase` key over the defaults (a partial or malformed key keeps them). */
export function readKnowledgeSettings(): KnowledgeSettings {
  const stored = window.store.get('knowledgebase') as Partial<KnowledgeSettings> | undefined
  if (!stored || typeof stored !== 'object') return DEFAULT_KNOWLEDGE_SETTINGS
  return {
    ...DEFAULT_KNOWLEDGE_SETTINGS,
    ...stored,
    embedding: { ...DEFAULT_KNOWLEDGE_SETTINGS.embedding, ...(stored.embedding ?? {}) },
    crawler: { ...DEFAULT_KNOWLEDGE_SETTINGS.crawler, ...(stored.crawler ?? {}) }
  }
}

/** The `knowledgebase` settings with a setter that writes them back to the store. */
export function useKnowledgeSettings(): [KnowledgeSettings, (next: KnowledgeSettings) => void] {
  const [settings, setSettings] = useState<KnowledgeSettings>(readKnowledgeSettings)
  useEffect(() => setSettings(readKnowledgeSettings()), [])
  const update = useCallback((next: KnowledgeSettings) => {
    setSettings(next)
    window.store.set('knowledgebase', next)
  }, [])
  return [settings, update]
}

/** The model a knowledgebase created now would use (mirrors `EmbeddingSettings::spec`). */
export function currentEmbeddingSpec(settings: KnowledgeSettings): EmbeddingSpec {
  const e = settings.embedding
  if (e.provider === 'local') {
    return {
      provider: 'local',
      modelId: LOCAL_EMBEDDING_MODEL.id,
      dimensions: LOCAL_EMBEDDING_MODEL.dimensions
    }
  }
  const modelId = e.bedrockModelId.trim() || BEDROCK_EMBEDDING_MODELS[0].id
  const base = modelId.replace(/^(us|eu|apac|jp|au|ca|us-gov|global)\./, '')
  const known = BEDROCK_EMBEDDING_MODELS.find((m) => m.id === base)
  const dimensions =
    known && !known.dimensions.includes(e.dimensions) ? known.dimensions[0] : e.dimensions || 1024
  return { provider: 'bedrock', modelId, dimensions }
}

export const sameSpec = (a: EmbeddingSpec, b: EmbeddingSpec): boolean =>
  a.provider === b.provider && a.modelId === b.modelId && a.dimensions === b.dimensions

/** `Amazon Titan Text Embeddings V2 (1024)` / `multilingual-e5-small (on device)`. */
export function embeddingLabel(spec: EmbeddingSpec): string {
  if (spec.provider === 'local') return LOCAL_EMBEDDING_MODEL.label
  const base = spec.modelId.replace(/^(us|eu|apac|jp|au|ca|us-gov|global)\./, '')
  const known = BEDROCK_EMBEDDING_MODELS.find((m) => m.id === base)
  return `${known?.label ?? spec.modelId} (${spec.dimensions})`
}

export function formatBytes(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  let i = 0
  let v = bytes
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`
}
