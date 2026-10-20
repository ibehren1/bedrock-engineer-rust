import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router-dom'
import { FiAlertTriangle, FiFile } from 'react-icons/fi'
import type { WorkspaceEntry } from '@/types/bridge/workspace-files'
import { baseName, changedPathsRelative, dirOf } from '@renderer/lib/workspaceFiles/paths'
import { FilePreview } from '../ChatPage/components/RightPanel/FilesTab/FilePreview'
import { useRevealLabel } from '../ChatPage/components/RightPanel/FilesTab'

/**
 * The pop-out file viewer window (`#/file-viewer?path=<root-relative>`, opened by
 * `workspaceFiles.popOut`): the Files panel's preview and editor for one file, without the app
 * sidebar. Follows changes on disk like the panel does, says so when the file is deleted, and
 * asks before closing over unsaved edits.
 */
const FileViewerPage: React.FC = () => {
  const { t } = useTranslation()
  const [params] = useSearchParams()
  const path = params.get('path') ?? ''
  const revealLabel = useRevealLabel()
  // The project folder this window was opened for; the backend always reads the current one.
  const [projectPath] = useState<string>(() => window.store.get('projectPath') ?? '')
  const [projectChanged, setProjectChanged] = useState(false)
  const [root, setRoot] = useState(projectPath)
  const [entry, setEntry] = useState<WorkspaceEntry | undefined>()
  const [exists, setExists] = useState<boolean | null>(null)
  const [changeToken, setChangeToken] = useState(0)
  const [dirty, setDirty] = useState(false)
  const dirtyRef = useRef(false)

  const onDirtyChange = useCallback((value: boolean) => {
    dirtyRef.current = value
    setDirty(value)
  }, [])

  // Size, mtime and existence, from the parent folder's listing.
  const refreshEntry = useCallback(async () => {
    if (!path) return
    try {
      const listing = await window.workspaceFiles.list(dirOf(path))
      if (listing.root) setRoot(listing.root)
      const found = listing.entries.find((e) => e.path === path && e.kind === 'file')
      setEntry(found)
      setExists(!!found)
    } catch {
      // The folder itself is gone (or the listing hides it): ask directly.
      try {
        const result = await window.workspaceFiles.exists([path])
        setExists(result[path]?.kind === 'file')
      } catch {
        setExists(false)
      }
    }
  }, [path])

  useEffect(() => {
    void refreshEntry()
  }, [refreshEntry])

  // Watch while the window is open (Rust also drops this window's watch when it closes).
  useEffect(() => {
    if (!path) return
    let watching = false
    let disposed = false
    window.workspaceFiles
      .watch()
      .then(() => {
        watching = true
        if (disposed) void window.workspaceFiles.unwatch().catch(() => {})
      })
      .catch((e) => console.warn('[FileViewer] file watcher unavailable', e))
    const unlisten = window.workspaceFiles.onChanged((event) => {
      if (!changedPathsRelative(event, projectPath).includes(path)) return
      setChangeToken(Date.now())
      void refreshEntry()
    })
    return () => {
      disposed = true
      unlisten()
      if (watching) void window.workspaceFiles.unwatch().catch(() => {})
    }
  }, [path, projectPath, refreshEntry])

  // The project folder changed in the main window: this path now means another file. The store
  // cache follows `store-changed`, so look when the window regains focus or files change.
  useEffect(() => {
    const check = () => setProjectChanged((window.store.get('projectPath') ?? '') !== projectPath)
    window.addEventListener('focus', check)
    const unlisten = window.workspaceFiles.onChanged(check)
    return () => {
      window.removeEventListener('focus', check)
      unlisten()
    }
  }, [projectPath])

  // Unsaved edits: ask before the window closes.
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let disposed = false
    window.appWindow
      .onCloseRequested(() => !dirtyRef.current || window.confirm(t('filesPanel.discardChanges')))
      .then((off) => {
        if (disposed) off()
        else unlisten = off
      })
      .catch((e) => console.warn('[FileViewer] close handler unavailable', e))
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [t])

  const notice = (message: string) => (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs bg-warning-soft text-warning border-b border-subtle">
      <FiAlertTriangle className="size-3.5 shrink-0" />
      <span>{message}</span>
    </div>
  )

  const body = () => {
    if (!path) {
      return (
        <div className="h-full grid place-items-center text-sm text-ink-faint">
          {t('filesPanel.viewerNoPath')}
        </div>
      )
    }
    if (projectChanged && !dirty) {
      return (
        <div className="h-full grid place-items-center p-6 text-center text-sm text-ink-faint">
          {t('filesPanel.viewerProjectChanged')}
        </div>
      )
    }
    // Deleted with nothing unsaved: nothing left to show. With unsaved edits the editor stays,
    // so they can still be saved (which recreates the file).
    if (exists === false && !dirty) {
      return (
        <div className="h-full grid place-items-center p-6 text-center text-sm text-ink-faint">
          {t('filesPanel.viewerDeleted')}
        </div>
      )
    }
    return (
      <FilePreview
        root={root}
        path={path}
        entry={entry}
        changeToken={changeToken}
        revealLabel={revealLabel}
        onDirtyChange={onDirtyChange}
      />
    )
  }

  return (
    <div className="h-screen flex flex-col bg-surface text-ink">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-subtle bg-canvas">
        <FiFile className="size-3.5 shrink-0 text-ink-faint" />
        <span className="text-sm font-medium truncate" title={path}>
          {baseName(path) || t('filesPanel.title')}
        </span>
        <span className="text-xs text-ink-faint font-mono truncate min-w-0" title={path}>
          {path}
        </span>
        <span className="ml-auto text-xs text-ink-faint truncate min-w-0 max-w-[40%]" title={root}>
          {root}
        </span>
      </div>
      {exists === false && dirty && notice(t('filesPanel.viewerDeletedEditing'))}
      {projectChanged && dirty && notice(t('filesPanel.viewerProjectChanged'))}
      <div className="flex-1 min-h-0">{body()}</div>
    </div>
  )
}

export default FileViewerPage
