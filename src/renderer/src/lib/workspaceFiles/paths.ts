/**
 * Pure path helpers for the chat's Files panel: project-relative paths (forward slashes, ""
 * for the project folder itself), the files a tool call touched, and spotting file paths in
 * chat text.
 */

/** Backslashes to forward slashes. */
export const toForwardSlashes = (path: string): string => path.replace(/\\/g, '/')

const WINDOWS_ABSOLUTE = /^[A-Za-z]:\//

/** True for `/x`, `C:/x`, `C:\x` and UNC `\\server\x`. */
export function isAbsolutePath(path: string): boolean {
  const p = toForwardSlashes(path)
  return p.startsWith('/') || WINDOWS_ABSOLUTE.test(p)
}

/** Collapse `.`/`..`/empty segments. `null` when `..` climbs above the start. */
function normalizeSegments(path: string): string | null {
  const out: string[] = []
  for (const segment of path.split('/')) {
    if (segment === '' || segment === '.') continue
    if (segment === '..') {
      if (out.length === 0) return null
      out.pop()
    } else {
      out.push(segment)
    }
  }
  return out.join('/')
}

/**
 * `path` (absolute, or relative to the project folder) as a project-relative path, or `null`
 * when it is outside the project folder or there is no project folder.
 */
export function toProjectRelative(path: string, projectPath: string | undefined): string | null {
  if (!projectPath || !path) return null
  const p = toForwardSlashes(path.trim())
  if (!p) return null
  if (!isAbsolutePath(p)) return normalizeSegments(p)

  const root = normalizeSegments(toForwardSlashes(projectPath))
  const target = normalizeSegments(p)
  if (root === null || target === null) return null
  // Windows paths compare case-insensitively; POSIX ones exactly.
  const windows = WINDOWS_ABSOLUTE.test(toForwardSlashes(projectPath))
  const same = (a: string, b: string): boolean =>
    windows ? a.toLowerCase() === b.toLowerCase() : a === b
  if (same(target, root)) return ''
  const prefix = root === '' ? '' : `${root}/`
  if (target.length > prefix.length && same(target.slice(0, prefix.length), prefix)) {
    return target.slice(prefix.length)
  }
  return null
}

/** The absolute path of a project-relative path, in the project folder's own separator style. */
export function toAbsolutePath(projectPath: string, relative: string): string {
  if (!relative) return projectPath
  const windowsStyle = projectPath.includes('\\') && !projectPath.includes('/')
  const sep = windowsStyle ? '\\' : '/'
  const base = projectPath.replace(/[\\/]+$/, '')
  return `${base}${sep}${windowsStyle ? relative.replace(/\//g, '\\') : relative}`
}

/** Ancestor directories of a relative path, outermost first: `a/b/c.md` → `['a', 'a/b']`. */
export function ancestorDirs(path: string): string[] {
  const parts = path.split('/').filter(Boolean)
  const dirs: string[] = []
  for (let i = 1; i < parts.length; i++) dirs.push(parts.slice(0, i).join('/'))
  return dirs
}

/** Directory part of a relative path ("" for top-level entries). */
export const dirOf = (path: string): string => {
  const i = path.lastIndexOf('/')
  return i === -1 ? '' : path.slice(0, i)
}

/** File name part of a relative path. */
export const baseName = (path: string): string => path.slice(path.lastIndexOf('/') + 1)

/** Lower-case extension without the dot. */
export function extOf(path: string): string {
  const name = baseName(path)
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(i + 1).toLowerCase() : ''
}

/** Group relative paths by their folder; folders sorted, "" (top level) first. */
export function groupByFolder(paths: string[]): { folder: string; paths: string[] }[] {
  const groups = new Map<string, string[]>()
  for (const path of paths) {
    const folder = dirOf(path)
    const list = groups.get(folder) ?? []
    list.push(path)
    groups.set(folder, list)
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)))
    .map(([folder, list]) => ({
      folder,
      paths: list.sort((a, b) => baseName(a).localeCompare(baseName(b)))
    }))
}

/** Built-in tools that create or change files, and the input fields holding their targets. */
const FILE_WRITING_TOOLS: Record<string, string[]> = {
  writeToFile: ['path'],
  applyDiffEdit: ['path'],
  createFolder: ['path'],
  // The source no longer exists after a move; only the destination is worth showing.
  moveFile: ['destination'],
  copyFile: ['destination']
}

export const isFileWritingTool = (name: string | undefined): boolean =>
  !!name && Object.prototype.hasOwnProperty.call(FILE_WRITING_TOOLS, name)

/**
 * Project-relative paths a file-writing tool call created or changed. Relative inputs are taken
 * as relative to the project folder, like the tools do; paths outside it are dropped.
 */
export function extractTouchedPaths(
  toolName: string | undefined,
  input: unknown,
  projectPath: string | undefined
): string[] {
  if (!toolName || !isFileWritingTool(toolName) || !input || typeof input !== 'object') return []
  const fields = FILE_WRITING_TOOLS[toolName]
  const paths: string[] = []
  for (const field of fields) {
    const value = (input as Record<string, unknown>)[field]
    if (typeof value !== 'string') continue
    const rel = toProjectRelative(value, projectPath)
    if (rel && !paths.includes(rel)) paths.push(rel)
  }
  return paths
}

/**
 * The file paths a file tool call names, as written by the model (for chips in the chat):
 * the targets of file-writing tools and the files `readFiles` read.
 */
export function fileToolTargets(toolName: string | undefined, input: unknown): string[] {
  if (!toolName || !input || typeof input !== 'object') return []
  const record = input as Record<string, unknown>
  if (toolName === 'readFiles') {
    return Array.isArray(record.paths)
      ? record.paths.filter((p): p is string => typeof p === 'string')
      : []
  }
  if (!isFileWritingTool(toolName)) return []
  return FILE_WRITING_TOOLS[toolName]
    .map((field) => record[field])
    .filter((v): v is string => typeof v === 'string' && v.length > 0)
}

/** Project-relative paths from a `workspace-files-changed` event (absolute or relative). */
export function changedPathsRelative(
  event: { root?: string; paths?: string[] },
  projectPath: string | undefined
): string[] {
  const root = event.root || projectPath
  const out: string[] = []
  for (const path of event.paths ?? []) {
    const rel = toProjectRelative(path, root)
    if (rel && !out.includes(rel)) out.push(rel)
  }
  return out
}

const HAS_EXTENSION = /\.[A-Za-z][A-Za-z0-9_-]{0,9}$/
const LINE_SUFFIX = /(?::\d+){1,2}$/
const NOT_A_PATH = /[\s<>|"'`*?{}()[\]$;,=!#&^]/

/**
 * If `text` (inline code or a link target in chat) looks like a file path, the string to check
 * with `workspaceFiles.exists` (an absolute path under the project folder, or a relative one);
 * otherwise `null`. Only a shape test: whether the file exists is up to the caller.
 */
export function detectFilePath(text: string, projectPath: string | undefined): string | null {
  if (!projectPath) return null
  let candidate = text.trim()
  if (!candidate || candidate.length > 400 || candidate.includes('\n')) return null
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(candidate) || /^(mailto|data|javascript):/i.test(candidate))
    return null
  // `src/app.ts:12` or `src/app.ts:12:4` → the file itself.
  candidate = candidate.replace(LINE_SUFFIX, '')
  if (NOT_A_PATH.test(candidate)) return null
  if (candidate.startsWith('-') || candidate.startsWith('~')) return null

  const normalized = toForwardSlashes(candidate)
  if (isAbsolutePath(normalized)) {
    return toProjectRelative(normalized, projectPath) !== null ? candidate : null
  }
  const relative = normalized.replace(/^(\.\/)+/, '')
  if (!relative || relative.startsWith('../') || relative === '..') return null
  // Needs a folder separator or an extension; `README` alone stays plain text.
  const hasSeparator = /[^/]\/[^/]/.test(relative)
  const name = baseName(relative.replace(/\/+$/, ''))
  if (!hasSeparator && !HAS_EXTENSION.test(name)) return null
  // Version numbers and the like: the name needs a letter before the extension or a separator.
  if (!hasSeparator && !/[A-Za-z_]/.test(name.slice(0, name.lastIndexOf('.')))) return null
  return relative.replace(/\/+$/, '')
}
