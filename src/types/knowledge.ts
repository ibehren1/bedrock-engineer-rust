// Local knowledgebases: the `knowledgebase` store key and the shapes of `window.api.knowledge`
// (Rust: src-tauri/crates/knowledge/src/types.rs).

export type EmbeddingProvider = 'bedrock' | 'local'

export type EmbeddingSpec = {
  provider: EmbeddingProvider
  modelId: string
  dimensions: number
}

export type KnowledgeSettings = {
  embedding: {
    provider: EmbeddingProvider
    bedrockModelId: string
    dimensions: number
    localModel: string
  }
  chunkSize: number
  chunkOverlap: number
  topK: number
  /** 0..1: weight of vector similarity against keyword matches in hybrid search. */
  hybridWeight: number
  crawler: {
    maxDepth: number
    maxPages: number
    delayMs: number
    respectRobots: boolean
    useSitemap: boolean
  }
}

export const DEFAULT_KNOWLEDGE_SETTINGS: KnowledgeSettings = {
  embedding: {
    provider: 'bedrock',
    bedrockModelId: 'amazon.titan-embed-text-v2:0',
    dimensions: 1024,
    localModel: 'multilingual-e5-small'
  },
  chunkSize: 400,
  chunkOverlap: 60,
  topK: 6,
  hybridWeight: 0.6,
  crawler: {
    maxDepth: 2,
    maxPages: 50,
    delayMs: 500,
    respectRobots: true,
    useSitemap: true
  }
}

/** Bedrock embedding models offered in Settings, with their allowed output sizes. */
export const BEDROCK_EMBEDDING_MODELS: { id: string; label: string; dimensions: number[] }[] = [
  {
    id: 'amazon.titan-embed-text-v2:0',
    label: 'Amazon Titan Text Embeddings V2',
    dimensions: [1024, 512, 256]
  },
  {
    id: 'cohere.embed-multilingual-v3',
    label: 'Cohere Embed Multilingual v3',
    dimensions: [1024]
  },
  { id: 'cohere.embed-english-v3', label: 'Cohere Embed English v3', dimensions: [1024] },
  { id: 'cohere.embed-v4:0', label: 'Cohere Embed v4', dimensions: [1536, 1024, 512, 256] }
]

export const LOCAL_EMBEDDING_MODEL = {
  id: 'multilingual-e5-small',
  label: 'multilingual-e5-small (on device)',
  dimensions: 384
}

export type KbInfo = {
  id: string
  agentId: string
  name: string
  description: string
  embedding: EmbeddingSpec
  chunkSize: number
  chunkOverlap: number
  createdAt: string
  sourceCount: number
  documentCount: number
  chunkCount: number
  bytes: number
}

export type UrlOptions = {
  maxDepth: number
  maxPages: number
  /** Only follow links under the start URL's directory. */
  pathPrefixOnly: boolean
}

export type SourceStatus = 'queued' | 'processing' | 'ready' | 'error' | 'cancelled'

export type SourceInfo = {
  id: string
  kind: 'file' | 'url'
  uri: string
  title: string
  options?: UrlOptions
  status: SourceStatus
  error?: string
  documentCount: number
  chunkCount: number
  bytes: number
  addedAt: string
  syncedAt?: string
}

export type JobPhase =
  'queued' | 'fetching' | 'extracting' | 'embedding' | 'indexing' | 'done' | 'error' | 'cancelled'

/** `knowledge:progress` payload. */
export type KnowledgeJob = {
  jobId: string
  agentId: string
  kbId: string
  /** Absent for a rebuild of the whole knowledgebase. */
  sourceId?: string
  phase: JobPhase
  current: number
  total: number
  message: string
}

export type KnowledgeSearchHit = {
  kbId: string
  kbName: string
  sourceId: string
  title: string
  uri: string
  headingPath: string
  text: string
  score: number
}

/** A finished ingestion / rebuild / optimize job. */
export type KnowledgeJobLog = {
  at: number
  kind: 'source' | 'rebuild' | 'optimize'
  sourceId: string | null
  durationMs: number
  documents: number
  chunks: number
  status: 'done' | 'error' | 'cancelled'
  message: string
}

export type KnowledgeUsage = {
  /** Agent searches per day, oldest first, today last. */
  dailySearches: number[]
  searchesInPeriod: number
  searchesTotal: number
  /** Searches from the editor's search box. */
  testSearchesInPeriod: number
  lastSearchAt: number | null
  latencyP50Ms: number | null
  latencyP95Ms: number | null
  avgHits: number | null
  lastJob: KnowledgeJobLog | null
  lastOptimizedAt: number | null
  avgChunkChars: number | null
}

export type KnowledgeHealthIssue = {
  kind:
    | 'sourceErrors'
    | 'staleWebsites'
    | 'modelMismatch'
    | 'localModelMissing'
    | 'reclaimable'
    | 'integrity'
    | 'empty'
  severity: 'info' | 'warning' | 'critical'
  /** Affected sources, or bytes for `reclaimable`. */
  count: number
}

export type KbStats = {
  info: KbInfo
  usage: KnowledgeUsage
  health: { reclaimableBytes: number; walBytes: number; issues: KnowledgeHealthIssue[] }
  fileSources: number
  websiteSources: number
  periodDays: number
}

export type KnowledgeOptimizeResult = { bytesBefore: number; bytesAfter: number; integrity: string }

export type KnowledgeStorageEntry = {
  agentId: string
  kbId: string
  name: string
  bytes: number
  sourceCount: number
  chunkCount: number
  embedding: EmbeddingSpec | null
  /** The owning agent no longer exists. */
  orphan: boolean
  stats: KbStats | null
}

export type KnowledgeStorageOverview = {
  root: string
  totalBytes: number
  knowledgeBases: KnowledgeStorageEntry[]
}

/** `knowledge:changed` payload. */
export type KnowledgeChanged = { agentId: string; kbId: string }

export type LocalModelStatus = {
  installed: boolean
  /** Bytes on disk when installed. */
  bytes: number
  downloading: boolean
  /** Download size, when known. */
  totalBytes: number
  supported: boolean
}

/** `knowledge:model-download` payload. */
export type LocalModelDownload = {
  state: 'downloading' | 'done' | 'error'
  received: number
  total: number
  file: string
  error?: string
}

export type KnowledgeApi = {
  list: (agentId: string) => Promise<KbInfo[]>
  create: (agentId: string, name: string, description?: string) => Promise<KbInfo>
  update: (agentId: string, kbId: string, name: string, description?: string) => Promise<KbInfo>
  delete: (agentId: string, kbId: string) => Promise<void>
  deleteAgent: (agentId: string) => Promise<void>
  copyAgent: (fromAgentId: string, toAgentId: string) => Promise<KbInfo[]>
  /** Re-embed with the current global embedding model; resolves to the job id. */
  rebuild: (agentId: string, kbId: string) => Promise<string>
  listSources: (agentId: string, kbId: string) => Promise<SourceInfo[]>
  /** Pick files in the native dialog and add them; `[]` when canceled. */
  pickFiles: (agentId: string, kbId: string) => Promise<SourceInfo[]>
  addUrl: (agentId: string, kbId: string, url: string, options?: UrlOptions) => Promise<SourceInfo>
  /** New crawl options for a website source; they apply from its next sync. */
  updateSourceOptions: (
    agentId: string,
    kbId: string,
    sourceId: string,
    options: UrlOptions
  ) => Promise<SourceInfo>
  resyncSource: (agentId: string, kbId: string, sourceId: string) => Promise<string>
  removeSource: (agentId: string, kbId: string, sourceId: string) => Promise<void>
  jobs: (agentId?: string) => Promise<KnowledgeJob[]>
  cancelJob: (jobId: string) => Promise<boolean>
  search: (
    agentId: string,
    query: string,
    kbId?: string,
    topK?: number
  ) => Promise<KnowledgeSearchHit[]>
  /** Usage over the last `periodDays` days and health checks. */
  stats: (agentId: string, kbId: string) => Promise<KbStats>
  /** Compact the file, merge the keyword index, check integrity. */
  optimize: (agentId: string, kbId: string) => Promise<KnowledgeOptimizeResult>
  storageOverview: () => Promise<KnowledgeStorageOverview>
  purgeOrphans: () => Promise<number>
  localModelStatus: () => Promise<LocalModelStatus>
  downloadLocalModel: () => Promise<void>
  deleteLocalModel: () => Promise<void>
  onProgress: (callback: (job: KnowledgeJob) => void) => () => void
  onChanged: (callback: (change: KnowledgeChanged) => void) => () => void
  onModelDownload: (callback: (progress: LocalModelDownload) => void) => () => void
}
