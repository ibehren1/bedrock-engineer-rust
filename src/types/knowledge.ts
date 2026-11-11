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
  /** Background re-sync of file and folder sources whose files changed on disk. */
  autoSync: {
    enabled: boolean
    intervalMinutes: number
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
  },
  autoSync: {
    enabled: true,
    intervalMinutes: 60
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

/** Knowledgebases are central: any custom agent attaches one by listing its id. Names are unique. */
export type KbInfo = {
  id: string
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
  kind: 'file' | 'folder' | 'url'
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
  /** Position of the passage in its document, from 0. */
  ord: number
  headingPath: string
  text: string
  score: number
}

/**
 * A knowledgebase the user `@mentioned` for one chat turn. Grants saved by older builds also carry
 * the agent that owned the knowledgebase; only `kbId` decides the match.
 */
export type KnowledgeBaseGrant = { kbId: string; agentId?: string }

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
  kbId: string
  name: string
  bytes: number
  sourceCount: number
  chunkCount: number
  embedding: EmbeddingSpec | null
  /** Ids of the custom agents that attach this knowledgebase; empty when none does. */
  attachedTo: string[]
  stats: KbStats | null
}

export type KnowledgeStorageOverview = {
  root: string
  totalBytes: number
  knowledgeBases: KnowledgeStorageEntry[]
  /** Folders of the old per-agent layout that could not be moved yet. */
  legacyLeftovers?: string[]
}

/** `knowledge:changed` payload. */
export type KnowledgeChanged = { kbId: string }

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
  list: () => Promise<KbInfo[]>
  /** Rejects when another knowledgebase already has the name (case-insensitive). */
  create: (name: string, description?: string) => Promise<KbInfo>
  /** Rejects when another knowledgebase already has the name (case-insensitive). */
  update: (kbId: string, name: string, description?: string) => Promise<KbInfo>
  delete: (kbId: string) => Promise<void>
  /** Re-embed with the current global embedding model; resolves to the job id. */
  rebuild: (kbId: string) => Promise<string>
  listSources: (kbId: string) => Promise<SourceInfo[]>
  /** Pick files in the native dialog and add them; `[]` when canceled. */
  pickFiles: (kbId: string) => Promise<SourceInfo[]>
  /** Pick a folder and add it as one source that follows its files; `null` when canceled. */
  pickFolder: (kbId: string) => Promise<SourceInfo | null>
  addUrl: (kbId: string, url: string, options?: UrlOptions) => Promise<SourceInfo>
  /** New crawl options for a website source; they apply from its next sync. */
  updateSourceOptions: (kbId: string, sourceId: string, options: UrlOptions) => Promise<SourceInfo>
  resyncSource: (kbId: string, sourceId: string) => Promise<string>
  removeSource: (kbId: string, sourceId: string) => Promise<void>
  jobs: (kbId?: string) => Promise<KnowledgeJob[]>
  cancelJob: (jobId: string) => Promise<boolean>
  search: (query: string, kbId?: string, topK?: number) => Promise<KnowledgeSearchHit[]>
  /** `searchKnowledgeBase` over only the `@mentioned` knowledgebases: the tool's text result. */
  searchGranted: (
    grants: KnowledgeBaseGrant[],
    query: string,
    knowledgeBase?: string,
    topK?: number
  ) => Promise<string>
  /** `listKnowledgeBaseSources` over only the `@mentioned` knowledgebases. */
  listSourcesGranted: (grants: KnowledgeBaseGrant[], knowledgeBase?: string) => Promise<string>
  /** Open a cited local file or folder; refused unless it is a source of some knowledgebase. */
  openSource: (uri: string) => Promise<void>
  /** Usage over the last `periodDays` days and health checks. */
  stats: (kbId: string) => Promise<KbStats>
  /** Compact the file, merge the keyword index, check integrity. */
  optimize: (kbId: string) => Promise<KnowledgeOptimizeResult>
  storageOverview: () => Promise<KnowledgeStorageOverview>
  localModelStatus: () => Promise<LocalModelStatus>
  downloadLocalModel: () => Promise<void>
  deleteLocalModel: () => Promise<void>
  onProgress: (callback: (job: KnowledgeJob) => void) => () => void
  onChanged: (callback: (change: KnowledgeChanged) => void) => () => void
  onModelDownload: (callback: (progress: LocalModelDownload) => void) => () => void
}
