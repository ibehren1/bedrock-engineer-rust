// `window.workspaceFiles`, installed by src/renderer/src/lib/tauriBridge.ts. Browses, previews and
// edits the files in the project folder (store key `projectPath`) for the chat's Files panel.
// Every path is relative to that folder with forward slashes; "" is the folder itself.

export type WorkspaceEntryKind = 'file' | 'dir'

export type WorkspaceEntry = {
  name: string
  /** Relative to the project folder, forward slashes. */
  path: string
  kind: WorkspaceEntryKind
  size: number
  modifiedMs: number
  /** Lower-case extension without the dot ("" when there is none). */
  ext: string
}

export type WorkspaceListResult = {
  /** The project folder (absolute). */
  root: string
  /** One level, directories first, ignore rules applied. */
  entries: WorkspaceEntry[]
}

export type WorkspaceTextFile = {
  content: string
  /** True when the file was larger than the read cap and `content` is its beginning. */
  truncated: boolean
  size: number
  modifiedMs: number
}

export type WorkspaceExistsResult = Record<
  string,
  { path: string; kind: WorkspaceEntryKind } | null
>

/** `workspace-files-changed`: paths that changed under `root` (debounced). */
export type WorkspaceFilesChangedEvent = {
  root: string
  paths: string[]
}

/** Prefix of the error message `writeText` rejects with when the file changed on disk. */
export const WORKSPACE_WRITE_CONFLICT = 'CONFLICT:'

export type WorkspaceFilesApi = {
  /** Children of `dir` ("" = project folder). */
  list(dir: string): Promise<WorkspaceListResult>
  /** Rejects with "Not a text file" for binary files. */
  readText(path: string): Promise<WorkspaceTextFile>
  /**
   * Save `content`. With `expectedModifiedMs`, rejects with a message starting
   * `CONFLICT:` when the file changed on disk since it was read.
   */
  writeText(
    path: string,
    content: string,
    expectedModifiedMs?: number
  ): Promise<{ modifiedMs: number }>
  /** Open in the OS default app. */
  open(path: string): Promise<void>
  /** Show in Finder / Explorer; no path opens the project folder itself. */
  reveal(path?: string): Promise<void>
  /** Which of `paths` (absolute or relative) exist under the project folder, keyed by input. */
  exists(paths: string[]): Promise<WorkspaceExistsResult>
  /** Start the file watcher. Reference-counted: each `watch` needs an `unwatch`. */
  watch(): Promise<void>
  unwatch(): Promise<void>
  onChanged(callback: (event: WorkspaceFilesChangedEvent) => void): () => void
  /**
   * Open the file in its own window (one per file: an open one is focused). Rejects when the
   * path is not a file under the project folder.
   */
  popOut(path: string): Promise<{ windowId: string; reused: boolean }>
  /** URL of the file's bytes (`workspacefile` scheme), for images, PDF and Word previews. */
  fileUrl(path: string, modifiedMs?: number): string
}
