/**
 * The files the agent created or changed in each chat (the session's `touchedFiles`), as shown
 * by the Files panel's fuchsia dots and its "This chat" view.
 *
 * Writes go through `window.chatHistory.addTouchedFiles`, which also updates the session cache;
 * subscribers here are told so the panel can re-read without polling. Each save rewrites the whole
 * session file, so new paths are batched: subscribers hear at once, the backend at most every
 * `TOUCHED_FILES_FLUSH_MS` and when the turn's tracking stops (`flushTouchedFiles`).
 */
import { useEffect, useState } from 'react'
import { changedPathsRelative, toProjectRelative } from './paths'

type Listener = (sessionId: string) => void
const listeners = new Set<Listener>()

/** Files already recorded per session in this window, so repeats don't hit the backend. */
const known = new Map<string, Set<string>>()

function knownFor(sessionId: string): Set<string> {
  let set = known.get(sessionId)
  if (!set) {
    set = new Set(window.chatHistory.getTouchedFiles(sessionId))
    known.set(sessionId, set)
  }
  return set
}

/** How long new paths wait before they are saved (unless the turn ends first). */
export const TOUCHED_FILES_FLUSH_MS = 2000

/** Paths recorded but not yet saved, per session, in record order. */
const pending = new Map<string, Set<string>>()
let flushTimer: ReturnType<typeof setTimeout> | undefined

/**
 * Save the paths waiting for `sessionId` (or for every session) now. Resolves once they are
 * saved; never throws.
 */
export async function flushTouchedFiles(sessionId?: string): Promise<void> {
  const ids = sessionId ? [sessionId] : [...pending.keys()]
  const saves = ids.flatMap((id) => {
    const paths = pending.get(id)
    pending.delete(id)
    if (!paths || paths.size === 0) return []
    return [
      window.chatHistory
        .addTouchedFiles(id, [...paths])
        .catch((error) => console.warn('[touchedFiles] could not save touched files', error))
    ]
  })
  if (pending.size === 0 && flushTimer !== undefined) {
    clearTimeout(flushTimer)
    flushTimer = undefined
  }
  await Promise.all(saves)
}

/**
 * Record project-relative `paths` for `sessionId` (new ones only). The Files panel hears about
 * them at once; the save is batched (see `flushTouchedFiles`). Never throws.
 */
export function recordTouchedFiles(sessionId: string, paths: string[]): void {
  const set = knownFor(sessionId)
  const fresh = paths.filter((p) => p && !set.has(p))
  if (fresh.length === 0) return
  fresh.forEach((p) => set.add(p))
  listeners.forEach((listener) => listener(sessionId))
  let queued = pending.get(sessionId)
  if (!queued) {
    queued = new Set()
    pending.set(sessionId, queued)
  }
  fresh.forEach((p) => queued.add(p))
  if (flushTimer === undefined) {
    flushTimer = setTimeout(() => {
      flushTimer = undefined
      void flushTouchedFiles()
    }, TOUCHED_FILES_FLUSH_MS)
  }
}

/** Current touched files of a session (cache + this window's pending records). */
export function getTouchedFiles(sessionId: string | undefined): string[] {
  if (!sessionId) return []
  const own = knownFor(sessionId)
  // After a tidy the session cache still has the old paths, so this window's list wins.
  if (tidied.has(sessionId)) return [...own]
  const fromSession = window.chatHistory.getTouchedFiles(sessionId)
  return [...new Set([...fromSession, ...own])]
}

/** Chats whose files this window has already moved into their folder. */
const tidied = new Set<string>()

/**
 * Move an existing chat's loose files into its folder (`chats/<chat>/`), once per chat per
 * window, and show them at their new paths. Chats from before the per-chat folder wrote files
 * all over the project folder; this is the clean-up. Never throws.
 */
export async function tidyChatFiles(sessionId: string | undefined): Promise<void> {
  if (!sessionId || tidied.has(sessionId)) return
  tidied.add(sessionId)
  try {
    // The tidy rewrites the saved paths, so the waiting ones must be in the file first.
    await flushTouchedFiles(sessionId)
    const result = await window.api.chatAttachments.tidy(sessionId)
    if (!result?.touched) return
    known.set(sessionId, new Set(result.touched))
    listeners.forEach((listener) => listener(sessionId))
  } catch (error) {
    tidied.delete(sessionId)
    console.warn('[touchedFiles] could not move the chat files into its folder', error)
  }
}

/**
 * Re-read a chat's files after its folder was renamed (the backend moved the folder and
 * rewrote the paths), so the Files panel shows them where they now are.
 */
export async function refreshTouchedFilesAfterRename(sessionId: string): Promise<void> {
  tidied.delete(sessionId)
  await tidyChatFiles(sessionId)
}

export function subscribeTouchedFiles(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** React view of a session's touched files. */
export function useTouchedFiles(sessionId: string | undefined): string[] {
  const [files, setFiles] = useState<string[]>(() => getTouchedFiles(sessionId))
  useEffect(() => {
    setFiles(getTouchedFiles(sessionId))
    return subscribeTouchedFiles((changed) => {
      if (changed === sessionId) setFiles(getTouchedFiles(sessionId))
    })
  }, [sessionId])
  return files
}

/**
 * While a chat turn runs, record every file that changes in the project folder as touched by
 * that chat. This catches files a command wrote (`executeCommand`, e.g. a pandoc export),
 * which no tool input names. Returns the function that stops it, which also saves the paths
 * still waiting.
 */
export function trackTurnFileChanges(sessionId: string, projectPath: string): () => void {
  let stopped = false
  const unlisten = window.workspaceFiles.onChanged((event) => {
    if (stopped) return
    // The project folder changed mid-turn: those files belong to another project.
    if (event.root && toProjectRelative(event.root, projectPath) !== '') return
    const paths = changedPathsRelative(event, projectPath)
    if (paths.length > 0) recordTouchedFiles(sessionId, paths)
  })
  let watching = false
  window.workspaceFiles
    .watch()
    .then(() => {
      watching = true
      if (stopped) void window.workspaceFiles.unwatch().catch(() => {})
    })
    .catch((error) => console.warn('[touchedFiles] file watcher unavailable', error))
  return () => {
    if (stopped) return
    stopped = true
    unlisten()
    if (watching) void window.workspaceFiles.unwatch().catch(() => {})
    void flushTouchedFiles(sessionId)
  }
}
