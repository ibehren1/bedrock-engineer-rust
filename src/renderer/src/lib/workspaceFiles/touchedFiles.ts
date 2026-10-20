/**
 * The files the agent created or changed in each chat (the session's `touchedFiles`), as shown
 * by the Files panel's fuchsia dots and its "This chat" view.
 *
 * Writes go through `window.chatHistory.addTouchedFiles`, which also updates the session cache;
 * subscribers here are told so the panel can re-read without polling.
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
    set = new Set(window.chatHistory.getSession(sessionId)?.touchedFiles ?? [])
    known.set(sessionId, set)
  }
  return set
}

/** Record project-relative `paths` for `sessionId` (new ones only). Never throws. */
export async function recordTouchedFiles(sessionId: string, paths: string[]): Promise<void> {
  const set = knownFor(sessionId)
  const fresh = paths.filter((p) => p && !set.has(p))
  if (fresh.length === 0) return
  fresh.forEach((p) => set.add(p))
  listeners.forEach((listener) => listener(sessionId))
  try {
    await window.chatHistory.addTouchedFiles(sessionId, fresh)
  } catch (error) {
    console.warn('[touchedFiles] could not save touched files', error)
  }
}

/** Current touched files of a session (cache + this window's pending records). */
export function getTouchedFiles(sessionId: string | undefined): string[] {
  if (!sessionId) return []
  const fromSession = window.chatHistory.getSession(sessionId)?.touchedFiles ?? []
  return [...new Set([...fromSession, ...knownFor(sessionId)])]
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
 * which no tool input names. Returns the function that stops it.
 */
export function trackTurnFileChanges(sessionId: string, projectPath: string): () => void {
  let stopped = false
  const unlisten = window.workspaceFiles.onChanged((event) => {
    if (stopped) return
    // The project folder changed mid-turn: those files belong to another project.
    if (event.root && toProjectRelative(event.root, projectPath) !== '') return
    const paths = changedPathsRelative(event, projectPath)
    if (paths.length > 0) void recordTouchedFiles(sessionId, paths)
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
  }
}
