import { currentEmbeddingSpec, embeddingLabel, formatBytes, sameSpec } from './knowledgeSettings'
import { DEFAULT_KNOWLEDGE_SETTINGS, KnowledgeSettings } from '@/types/knowledge'

const withEmbedding = (e: Partial<KnowledgeSettings['embedding']>): KnowledgeSettings => ({
  ...DEFAULT_KNOWLEDGE_SETTINGS,
  embedding: { ...DEFAULT_KNOWLEDGE_SETTINGS.embedding, ...e }
})

describe('currentEmbeddingSpec', () => {
  // Same cases as `embedding_spec_from_settings` in src-tauri/crates/knowledge/src/types.rs.
  it('follows the Rust spec rules', () => {
    expect(currentEmbeddingSpec(DEFAULT_KNOWLEDGE_SETTINGS)).toEqual({
      provider: 'bedrock',
      modelId: 'amazon.titan-embed-text-v2:0',
      dimensions: 1024
    })
    expect(currentEmbeddingSpec(withEmbedding({ dimensions: 300 })).dimensions).toBe(1024)
    expect(
      currentEmbeddingSpec(
        withEmbedding({ bedrockModelId: 'us.cohere.embed-v4:0', dimensions: 512 })
      ).dimensions
    ).toBe(512)
    expect(
      currentEmbeddingSpec(
        withEmbedding({ bedrockModelId: 'cohere.embed-multilingual-v3', dimensions: 512 })
      ).dimensions
    ).toBe(1024)
    expect(currentEmbeddingSpec(withEmbedding({ provider: 'local' }))).toEqual({
      provider: 'local',
      modelId: 'multilingual-e5-small',
      dimensions: 384
    })
  })

  it('labels and compares specs', () => {
    const spec = currentEmbeddingSpec(DEFAULT_KNOWLEDGE_SETTINGS)
    expect(embeddingLabel(spec)).toBe('Amazon Titan Text Embeddings V2 (1024)')
    expect(sameSpec(spec, { ...spec })).toBe(true)
    expect(sameSpec(spec, { ...spec, dimensions: 256 })).toBe(false)
  })
})

describe('formatBytes', () => {
  it('formats sizes', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(470_641_600)).toBe('449 MB')
  })
})
