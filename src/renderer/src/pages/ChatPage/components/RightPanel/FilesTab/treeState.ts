/** Pure state helpers for the Files panel's lazy tree. */
import type { WorkspaceEntry } from '@/types/bridge/workspace-files'
import { ancestorDirs, baseName, dirOf } from '@renderer/lib/workspaceFiles/paths'

/** Loaded directory listings, keyed by relative directory path ("" = project folder). */
export type LoadedDirs = Map<string, WorkspaceEntry[]>

export type TreeFilter = {
  /** Entries to show. */
  visible: Set<string>
  /** Directories to show expanded because they hold matches. */
  expand: Set<string>
}

/**
 * Which loaded entries match `filter` (case-insensitive, by name), plus their ancestors so the
 * matches stay reachable. `null` when there is no filter (show everything).
 */
export function filterTree(dirs: LoadedDirs, filter: string): TreeFilter | null {
  const needle = filter.trim().toLowerCase()
  if (!needle) return null
  const visible = new Set<string>()
  const expand = new Set<string>()
  for (const entries of dirs.values()) {
    for (const entry of entries) {
      if (!entry.name.toLowerCase().includes(needle)) continue
      visible.add(entry.path)
      for (const dir of ancestorDirs(entry.path)) {
        visible.add(dir)
        expand.add(dir)
      }
    }
  }
  return { visible, expand }
}

/** Loaded directories whose listing a change to `changed` paths can affect. */
export function dirsToReload(changed: string[], loaded: Iterable<string>): string[] {
  const loadedSet = new Set(loaded)
  const out = new Set<string>()
  for (const path of changed) {
    const parent = dirOf(path)
    if (loadedSet.has(parent)) out.add(parent)
    // A loaded directory that was itself replaced or removed.
    if (loadedSet.has(path)) out.add(path)
  }
  return [...out]
}

/** Touched files plus their ancestor directories (for the dimmer dot on folders). */
export function touchedAncestors(touched: string[]): Set<string> {
  const dirs = new Set<string>()
  for (const path of touched) for (const dir of ancestorDirs(path)) dirs.add(dir)
  return dirs
}

/** Filter "This chat" paths by name or folder. */
export function filterPaths(paths: string[], filter: string): string[] {
  const needle = filter.trim().toLowerCase()
  if (!needle) return paths
  return paths.filter(
    (p) => baseName(p).toLowerCase().includes(needle) || p.toLowerCase().includes(needle)
  )
}

/** Remember expanded folders per project folder for the lifetime of the window. */
const expandedByRoot = new Map<string, Set<string>>()

export function rememberedExpanded(root: string): Set<string> {
  return new Set(expandedByRoot.get(root) ?? [])
}

export function rememberExpanded(root: string, expanded: Set<string>): void {
  expandedByRoot.set(root, new Set(expanded))
}

/**
 * Folders to open so the given touched files (that exist) show in the project tree: their
 * ancestors, outermost first, minus the ones already open and the ones the user collapsed.
 */
export function touchedDirsToExpand(
  touchedFiles: string[],
  expanded: ReadonlySet<string>,
  userCollapsed: ReadonlySet<string>
): string[] {
  const out: string[] = []
  const seen = new Set<string>()
  for (const path of touchedFiles) {
    for (const dir of ancestorDirs(path)) {
      if (seen.has(dir)) continue
      seen.add(dir)
      if (!expanded.has(dir) && !userCollapsed.has(dir)) out.push(dir)
    }
  }
  // Outermost first, so parents load before their children.
  return out.sort((a, b) => a.split('/').length - b.split('/').length)
}

/** `expanded` plus `dirs`; the same set when nothing is new (so React skips the update). */
export function withExpanded(expanded: Set<string>, dirs: string[]): Set<string> {
  if (dirs.every((d) => expanded.has(d))) return expanded
  const next = new Set(expanded)
  dirs.forEach((d) => next.add(d))
  return next
}

/**
 * Folders the user collapsed in a project folder + chat, so auto-expanding the touched files'
 * folders never reopens them. Opening a folder again forgets it. For the window's lifetime.
 */
const collapsedByChat = new Map<string, Set<string>>()

const collapsedKey = (root: string, sessionId: string | undefined) =>
  `${root}\u0000${sessionId ?? ''}`

export function userCollapsedDirs(root: string, sessionId: string | undefined): Set<string> {
  return new Set(collapsedByChat.get(collapsedKey(root, sessionId)) ?? [])
}

/** Record the user opening (`opening`) or collapsing `dir`; the updated set. */
export function noteUserToggle(collapsed: Set<string>, dir: string, opening: boolean): Set<string> {
  const next = new Set(collapsed)
  if (opening) next.delete(dir)
  else next.add(dir)
  return next
}

export function rememberUserCollapsed(
  root: string,
  sessionId: string | undefined,
  collapsed: Set<string>
): void {
  collapsedByChat.set(collapsedKey(root, sessionId), new Set(collapsed))
}
