/**
 * Batched, cached `workspaceFiles.exists` lookups for file paths spotted in chat text.
 *
 * Each rendered candidate asks once; requests made within a short window go to the backend as
 * one call. Results are cached per project folder. Misses are forgotten whenever files change
 * (`forgetMissing`), because the agent often names a file before it writes it.
 */
import type { WorkspaceEntryKind } from '@/types/bridge/workspace-files'

export type ResolvedPath = { path: string; kind: WorkspaceEntryKind } | null

const BATCH_DELAY_MS = 120
const MAX_BATCH = 200

type Pending = { resolve: (value: ResolvedPath) => void }

let cacheRoot: string | undefined
const cache = new Map<string, ResolvedPath>()
const inflight = new Map<string, Promise<ResolvedPath>>()
let queue = new Map<string, Pending[]>()
let timer: ReturnType<typeof setTimeout> | undefined

function switchRoot(projectPath: string): void {
  if (cacheRoot === projectPath) return
  cacheRoot = projectPath
  cache.clear()
  inflight.clear()
}

async function flush(root: string): Promise<void> {
  timer = undefined
  const batch = queue
  queue = new Map()
  const inputs = [...batch.keys()]
  for (let i = 0; i < inputs.length; i += MAX_BATCH) {
    const chunk = inputs.slice(i, i + MAX_BATCH)
    let result: Record<string, ResolvedPath> = {}
    try {
      result = await window.workspaceFiles.exists(chunk)
    } catch {
      // Treated as misses, which are retried after the next file change.
    }
    for (const input of chunk) {
      const value = result[input] ?? null
      if (cacheRoot === root) cache.set(input, value)
      inflight.delete(input)
      batch.get(input)?.forEach((p) => p.resolve(value))
    }
  }
}

/** Whether `candidate` (absolute or project-relative) exists under `projectPath`. */
export function resolveWorkspacePath(
  projectPath: string,
  candidate: string
): Promise<ResolvedPath> {
  switchRoot(projectPath)
  if (cache.has(candidate)) return Promise.resolve(cache.get(candidate) ?? null)
  const existing = inflight.get(candidate)
  if (existing) return existing
  const promise = new Promise<ResolvedPath>((resolve) => {
    const waiting = queue.get(candidate) ?? []
    waiting.push({ resolve })
    queue.set(candidate, waiting)
    if (!timer) timer = setTimeout(() => void flush(projectPath), BATCH_DELAY_MS)
  })
  inflight.set(candidate, promise)
  return promise
}

/** The cached answer, if there is one (`undefined` = not asked yet). */
export function cachedWorkspacePath(
  projectPath: string,
  candidate: string
): ResolvedPath | undefined {
  if (cacheRoot !== projectPath) return undefined
  return cache.get(candidate)
}

/** Drop cached misses so they are asked again (after files changed). */
export function forgetMissing(): void {
  for (const [key, value] of cache) if (value === null) cache.delete(key)
}
