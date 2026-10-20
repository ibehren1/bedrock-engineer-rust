import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiChevronRight, FiFile, FiFolder, FiRefreshCw } from 'react-icons/fi'
import type { WorkspaceEntry, WorkspaceEntryKind } from '@/types/bridge/workspace-files'
import {
  ancestorDirs,
  baseName,
  changedPathsRelative,
  dirOf,
  groupByFolder
} from '@renderer/lib/workspaceFiles/paths'
import { useTouchedFiles } from '@renderer/lib/workspaceFiles/touchedFiles'
import { detectPlatform } from '@renderer/lib/nativeMenus'
import { FilePreview } from './FilePreview'
import {
  dirsToReload,
  filterPaths,
  filterTree,
  noteUserToggle,
  rememberExpanded,
  rememberedExpanded,
  rememberUserCollapsed,
  touchedAncestors,
  touchedDirsToExpand,
  userCollapsedDirs,
  withExpanded,
  type LoadedDirs
} from './treeState'

export type FilesOpenRequest = { path: string; kind?: WorkspaceEntryKind; nonce: number }

type FilesTabProps = {
  projectPath: string
  sessionId?: string
  /** The panel is open on this tab (the watcher runs only then). */
  active: boolean
  openRequest?: FilesOpenRequest
}

type Scope = 'project' | 'chat'

const errorMessage = (e: unknown): string => (e instanceof Error ? e.message : String(e))

/** "Show in Finder" / "Show in Explorer" / "Show in folder", by OS. */
export function useRevealLabel(): string {
  const { t } = useTranslation()
  const platform = useMemo(() => detectPlatform(navigator.userAgent), [])
  return platform === 'mac'
    ? t('filesPanel.showInFinder')
    : platform === 'windows'
      ? t('filesPanel.showInExplorer')
      : t('filesPanel.showInFolder')
}

export const FilesTab: React.FC<FilesTabProps> = ({
  projectPath,
  sessionId,
  active,
  openRequest
}) => {
  const { t } = useTranslation()
  const revealLabel = useRevealLabel()
  const [root, setRoot] = useState(projectPath)
  const [dirs, setDirs] = useState<LoadedDirs>(new Map())
  const [dirErrors, setDirErrors] = useState<Map<string, string>>(new Map())
  const [loadingDirs, setLoadingDirs] = useState<Set<string>>(new Set())
  const [expanded, setExpanded] = useState<Set<string>>(() => rememberedExpanded(projectPath))
  const [selected, setSelected] = useState<{ path: string; kind: WorkspaceEntryKind } | null>(null)
  const [scope, setScope] = useState<Scope>('project')
  const [filter, setFilter] = useState('')
  const [changeTokens, setChangeTokens] = useState<Map<string, number>>(new Map())
  const [chatExisting, setChatExisting] = useState<string[] | null>(null)
  const dirtyRef = useRef(false)
  const dirsRef = useRef(dirs)
  dirsRef.current = dirs
  const expandedRef = useRef(expanded)
  expandedRef.current = expanded

  const touched = useTouchedFiles(sessionId)
  const touchedSet = useMemo(() => new Set(touched), [touched])
  const touchedDirs = useMemo(() => touchedAncestors(touched), [touched])
  // Folders the user collapsed in this project folder + chat: auto-expanding leaves them shut.
  const collapsedRef = useRef<Set<string>>(userCollapsedDirs(projectPath, sessionId))
  useEffect(() => {
    collapsedRef.current = userCollapsedDirs(projectPath, sessionId)
  }, [projectPath, sessionId])

  const loadDir = useCallback(async (dir: string): Promise<WorkspaceEntry[] | null> => {
    setLoadingDirs((prev) => new Set(prev).add(dir))
    try {
      const result = await window.workspaceFiles.list(dir)
      if (dir === '' && result.root) setRoot(result.root)
      setDirs((prev) => new Map(prev).set(dir, result.entries))
      setDirErrors((prev) => {
        if (!prev.has(dir)) return prev
        const next = new Map(prev)
        next.delete(dir)
        return next
      })
      return result.entries
    } catch (e) {
      if (dir === '') {
        setDirErrors((prev) => new Map(prev).set(dir, errorMessage(e)))
      } else {
        // Gone (deleted or renamed): forget it and collapse.
        setDirs((prev) => {
          const next = new Map(prev)
          next.delete(dir)
          return next
        })
        setExpanded((prev) => {
          const next = new Set(prev)
          next.delete(dir)
          return next
        })
      }
      return null
    } finally {
      setLoadingDirs((prev) => {
        const next = new Set(prev)
        next.delete(dir)
        return next
      })
    }
  }, [])

  // A new project folder: reload the root and the folders that were open there before.
  useEffect(() => {
    setRoot(projectPath)
    setDirs(new Map())
    setDirErrors(new Map())
    setSelected(null)
    const remembered = rememberedExpanded(projectPath)
    setExpanded(remembered)
    if (!projectPath) return
    void loadDir('')
    remembered.forEach((dir) => void loadDir(dir))
  }, [projectPath, loadDir])

  useEffect(() => {
    if (projectPath) rememberExpanded(projectPath, expanded)
  }, [projectPath, expanded])

  const reloadLoaded = useCallback(() => {
    for (const dir of dirsRef.current.keys()) void loadDir(dir)
  }, [loadDir])

  // Watch while showing. Coming back to the tab reloads what is open, since changes made while
  // it was hidden were not seen.
  useEffect(() => {
    if (!active || !projectPath) return
    reloadLoaded()
    setChangeTokens((prev) => {
      if (!selected) return prev
      return new Map(prev).set(selected.path, Date.now())
    })
    let watching = false
    let disposed = false
    window.workspaceFiles
      .watch()
      .then(() => {
        watching = true
        if (disposed) void window.workspaceFiles.unwatch().catch(() => {})
      })
      .catch((e) => console.warn('[FilesTab] file watcher unavailable', e))
    const unlisten = window.workspaceFiles.onChanged((event) => {
      const changed = changedPathsRelative(event, projectPath)
      if (changed.length === 0) return
      for (const dir of dirsToReload(changed, dirsRef.current.keys())) void loadDir(dir)
      const now = Date.now()
      setChangeTokens((prev) => {
        const next = new Map(prev)
        changed.forEach((p) => next.set(p, now))
        return next
      })
    })
    return () => {
      disposed = true
      unlisten()
      if (watching) void window.workspaceFiles.unwatch().catch(() => {})
    }
    // `selected` is read once on activation only.
  }, [active, projectPath, loadDir, reloadLoaded])

  const confirmLeave = (): boolean => {
    if (!dirtyRef.current) return true
    if (!window.confirm(t('filesPanel.discardChanges'))) return false
    dirtyRef.current = false
    return true
  }

  // Open the folders holding this chat's touched files (that still exist), when the tab opens,
  // the chat changes or new files are touched. Only ever adds; folders loaded lazily.
  useEffect(() => {
    if (!active || !projectPath || touched.length === 0) return
    let cancelled = false
    window.workspaceFiles
      .exists(touched)
      .then((result) => {
        if (cancelled) return
        const files = touched
          .map((p) => result[p])
          .filter((r): r is { path: string; kind: WorkspaceEntryKind } => !!r && r.kind === 'file')
          .map((r) => r.path)
        const dirs = touchedDirsToExpand(files, expandedRef.current, collapsedRef.current)
        if (dirs.length === 0) return
        setExpanded((prev) => withExpanded(prev, dirs))
        dirs.forEach((d) => {
          if (!dirsRef.current.has(d)) void loadDir(d)
        })
      })
      .catch((e) => console.warn('[FilesTab] could not check touched files', e))
    return () => {
      cancelled = true
    }
  }, [active, projectPath, sessionId, touched, loadDir])

  const popOut = (path: string) => {
    window.workspaceFiles
      .popOut(path)
      .catch((e) => toast.error(t('filesPanel.popOutFailed', { error: errorMessage(e) })))
  }

  const toggleDir = (dir: string) => {
    const opening = !expandedRef.current.has(dir)
    collapsedRef.current = noteUserToggle(collapsedRef.current, dir, opening)
    rememberUserCollapsed(projectPath, sessionId, collapsedRef.current)
    if (opening && !dirsRef.current.has(dir)) void loadDir(dir)
    setExpanded((prev) => {
      const next = new Set(prev)
      if (opening) next.add(dir)
      else next.delete(dir)
      return next
    })
  }

  const select = (path: string, kind: WorkspaceEntryKind) => {
    if (kind === 'dir') {
      toggleDir(path)
      return
    }
    if (selected?.path === path) return
    if (!confirmLeave()) return
    setSelected({ path, kind })
  }

  // Opened from a chat link: reveal it in the tree and select it.
  useEffect(() => {
    if (!openRequest || !projectPath) return
    const { path, kind } = openRequest
    if (!confirmLeave()) return
    if (scope === 'chat' && !touchedSet.has(path)) setScope('project')
    setFilter('')
    const parents = kind === 'dir' ? [...ancestorDirs(path), path] : ancestorDirs(path)
    setExpanded((prev) => {
      const next = new Set(prev)
      parents.forEach((d) => next.add(d))
      return next
    })
    parents.forEach((d) => {
      if (!dirsRef.current.has(d)) void loadDir(d)
    })
    if (kind !== 'dir') setSelected({ path, kind: 'file' })
    // Scroll the row into view once the tree has rendered it.
    setTimeout(() => {
      document
        .querySelector(`[data-files-path="${CSS.escape(path)}"]`)
        ?.scrollIntoView({ block: 'nearest' })
    }, 300)
  }, [openRequest?.nonce])

  // "This chat": touched files that still exist.
  useEffect(() => {
    if (scope !== 'chat') return
    if (touched.length === 0) {
      setChatExisting([])
      return
    }
    let cancelled = false
    window.workspaceFiles
      .exists(touched)
      .then((result) => {
        if (cancelled) return
        const files = touched
          .map((p) => result[p])
          .filter((r): r is { path: string; kind: WorkspaceEntryKind } => !!r && r.kind === 'file')
          .map((r) => r.path)
        setChatExisting([...new Set(files)])
      })
      .catch(() => !cancelled && setChatExisting(touched))
    return () => {
      cancelled = true
    }
  }, [scope, touched, changeTokens])

  const refresh = () => {
    reloadLoaded()
    if (selected) setChangeTokens((prev) => new Map(prev).set(selected.path, Date.now()))
  }

  const revealRoot = () => {
    window.workspaceFiles.reveal().catch((e) => toast.error(errorMessage(e)))
  }

  const treeFilter = useMemo(() => filterTree(dirs, filter), [dirs, filter])

  const selectedEntry = useMemo(() => {
    if (!selected) return undefined
    return dirs.get(dirOf(selected.path))?.find((e) => e.path === selected.path)
  }, [dirs, selected])

  const onDirtyChange = useCallback((dirty: boolean) => {
    dirtyRef.current = dirty
  }, [])

  const row = (
    entry: { path: string; name: string; kind: WorkspaceEntryKind },
    depth: number,
    extra?: React.ReactNode
  ) => {
    const isDir = entry.kind === 'dir'
    const isOpen = isDir && (expanded.has(entry.path) || !!treeFilter?.expand.has(entry.path))
    const isSelected = selected?.path === entry.path
    const touchedFile = !isDir && touchedSet.has(entry.path)
    const touchedDir = isDir && touchedDirs.has(entry.path)
    return (
      <button
        key={entry.path}
        data-files-path={entry.path}
        onClick={() => select(entry.path, entry.kind)}
        className={`w-full flex items-center gap-1.5 py-[3px] pr-1.5 rounded-control text-left text-xs whitespace-nowrap ${
          isSelected
            ? 'bg-accent-tint outline outline-1 outline-accent text-ink'
            : 'text-ink hover:bg-surface-2'
        }`}
        style={{ paddingLeft: 6 + depth * 14 }}
        title={entry.path}
      >
        {isDir ? (
          <FiChevronRight
            className={`size-3 shrink-0 text-ink-faint transition-transform ${isOpen ? 'rotate-90' : ''}`}
          />
        ) : (
          <span className="w-3 shrink-0" />
        )}
        {isDir ? (
          <FiFolder className="size-3.5 shrink-0 text-ink-muted" />
        ) : (
          <FiFile className="size-3.5 shrink-0 text-ink-faint" />
        )}
        <span className={`truncate ${isDir ? 'text-ink-muted' : ''}`}>{entry.name}</span>
        {extra}
        {(touchedFile || touchedDir) && (
          <span
            className={`ml-auto size-1.5 shrink-0 rounded-full bg-touched ${touchedDir ? 'opacity-50' : ''}`}
            title={t('filesPanel.touchedHint')}
          />
        )}
      </button>
    )
  }

  const renderDir = (dir: string, depth: number): React.ReactNode[] => {
    const entries = dirs.get(dir)
    if (!entries) {
      return loadingDirs.has(dir)
        ? [
            <div
              key={`${dir}/__loading`}
              className="py-[3px] text-xs text-ink-faint"
              style={{ paddingLeft: 6 + depth * 14 + 18 }}
            >
              {t('filesPanel.loading')}
            </div>
          ]
        : []
    }
    const out: React.ReactNode[] = []
    for (const entry of entries) {
      if (treeFilter && !treeFilter.visible.has(entry.path)) continue
      out.push(row(entry, depth))
      const open =
        entry.kind === 'dir' && (expanded.has(entry.path) || !!treeFilter?.expand.has(entry.path))
      if (open) out.push(...renderDir(entry.path, depth + 1))
    }
    return out
  }

  const renderProject = () => {
    const rootError = dirErrors.get('')
    if (rootError) {
      return (
        <div className="px-2 py-2 text-xs text-danger">
          {t('filesPanel.loadFailed', { error: rootError })}
        </div>
      )
    }
    const rows = renderDir('', 0)
    if (rows.length === 0 && dirs.has('')) {
      return (
        <div className="px-2 py-2 text-xs text-ink-faint">
          {treeFilter ? t('filesPanel.noMatches') : t('filesPanel.emptyFolder')}
        </div>
      )
    }
    return rows
  }

  const renderChat = () => {
    if (!sessionId) {
      return <div className="px-2 py-2 text-xs text-ink-faint">{t('filesPanel.noChatSession')}</div>
    }
    if (chatExisting === null) {
      return <div className="px-2 py-2 text-xs text-ink-faint">{t('filesPanel.loading')}</div>
    }
    const files = filterPaths(chatExisting, filter)
    if (files.length === 0) {
      return (
        <div className="px-2 py-2 text-xs text-ink-faint">
          {chatExisting.length === 0 ? t('filesPanel.noChatFiles') : t('filesPanel.noMatches')}
        </div>
      )
    }
    return groupByFolder(files).map((group) => (
      <div key={group.folder || '/'} className="mb-1.5">
        {group.folder && (
          <div
            className="flex items-center gap-1.5 px-1.5 py-[3px] text-micro font-normal tracking-normal text-ink-faint truncate"
            title={group.folder}
          >
            <FiFolder className="size-3 shrink-0" />
            <span className="truncate">{group.folder}</span>
          </div>
        )}
        {group.paths.map((path) =>
          row({ path, name: baseName(path), kind: 'file' }, group.folder ? 1 : 0)
        )}
      </div>
    ))
  }

  if (!projectPath) {
    return (
      <div className="h-full grid place-items-center p-6 text-center text-sm text-ink-faint">
        {t('filesPanel.noProject')}
      </div>
    )
  }

  const scopeButton = (value: Scope, label: string) => (
    <button
      onClick={() => setScope(value)}
      className={`flex-1 text-center text-micro font-normal tracking-normal py-1 rounded-control ${
        scope === value
          ? 'bg-accent-tint text-accent'
          : 'bg-surface-2 text-ink-muted hover:text-ink'
      }`}
    >
      {label}
    </button>
  )

  return (
    <div className="h-full flex flex-col min-h-0">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-subtle">
        <span className="text-xs text-ink-faint truncate min-w-0 mr-auto" title={root}>
          {root}
        </span>
        <button
          onClick={refresh}
          className="p-1 rounded-control text-ink-muted hover:bg-raised hover:text-ink"
          title={t('filesPanel.refresh')}
          aria-label={t('filesPanel.refresh')}
        >
          <FiRefreshCw className={`size-3.5 ${loadingDirs.size > 0 ? 'animate-spin' : ''}`} />
        </button>
        <button
          onClick={revealRoot}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-control border border-subtle text-xs text-ink-muted hover:bg-raised hover:text-ink whitespace-nowrap"
        >
          <FiFolder className="size-3.5" />
          {revealLabel}
        </button>
      </div>

      <div className="flex-1 flex min-h-0">
        <div className="w-56 shrink-0 border-r border-subtle flex flex-col min-h-0">
          <div className="flex gap-1 px-1.5 pt-1.5">
            {scopeButton('project', t('filesPanel.scopeProject'))}
            {scopeButton('chat', t('filesPanel.scopeChat'))}
          </div>
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t('filesPanel.filterPlaceholder')}
            className="mx-1.5 my-1.5 px-1.5 py-1 text-xs rounded-control bg-surface-2 border border-subtle text-ink placeholder:text-ink-faint focus:outline-none focus:border-accent"
          />
          <div className="flex-1 min-h-0 overflow-auto px-1 pb-2">
            {scope === 'project' ? renderProject() : renderChat()}
          </div>
        </div>

        <div className="flex-1 min-w-0 flex flex-col">
          <div className="flex-1 min-h-0">
            {selected ? (
              <FilePreview
                key={selected.path}
                root={root}
                path={selected.path}
                entry={selectedEntry}
                changeToken={changeTokens.get(selected.path) ?? 0}
                revealLabel={revealLabel}
                onDirtyChange={onDirtyChange}
                onPopOut={() => popOut(selected.path)}
              />
            ) : (
              <div className="h-full grid place-items-center text-sm text-ink-faint">
                {t('filesPanel.selectFile')}
              </div>
            )}
          </div>
          <div className="flex items-center gap-2 px-2.5 py-1 border-t border-subtle text-micro font-normal tracking-normal text-ink-faint">
            <span className="inline-flex items-center gap-1.5 truncate">
              <span className="size-1.5 rounded-full bg-touched shrink-0" />
              {t('filesPanel.footerTouched')}
            </span>
            <span className="ml-auto whitespace-nowrap">{t('filesPanel.footerLive')}</span>
          </div>
        </div>
      </div>
    </div>
  )
}
