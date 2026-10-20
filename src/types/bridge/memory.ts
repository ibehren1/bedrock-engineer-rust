// `window.memory`, installed by src/renderer/src/lib/tauriBridge.ts. The global list of memory
// items (rules and facts) the model can add, update and remove with its memory tools.

export type MemoryItemKind = 'rule' | 'memory'
export type MemoryItemSource = 'chat' | 'manual'

export type MemoryItem = {
  id: string
  content: string
  kind: MemoryItemKind
  enabled: boolean
  source: MemoryItemSource
  createdAt: number
  updatedAt: number
}

export type MemoryAddParams = {
  content: string
  kind?: MemoryItemKind
  source?: MemoryItemSource
}

export type MemoryUpdateParams = {
  id: string
  content?: string
  kind?: MemoryItemKind
  enabled?: boolean
}

export type MemoryApi = {
  /** All items, newest first. */
  list(): Promise<MemoryItem[]>
  add(params: MemoryAddParams): Promise<MemoryItem>
  update(params: MemoryUpdateParams): Promise<MemoryItem>
  delete(id: string): Promise<boolean>
  /** `memory-changed`, fired after every write (including the model's tool calls). */
  onChanged(callback: (items: MemoryItem[]) => void): () => void
}
