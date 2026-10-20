import React, { Suspense, lazy, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiCopy, FiEdit2, FiExternalLink, FiFolder, FiMaximize2 } from 'react-icons/fi'
import { VscCode, VscEye } from 'react-icons/vsc'
import type { WorkspaceEntry, WorkspaceTextFile } from '@/types/bridge/workspace-files'
import { WORKSPACE_WRITE_CONFLICT } from '@/types/bridge/workspace-files'
import { baseName, extOf, toAbsolutePath } from '@renderer/lib/workspaceFiles/paths'
import {
  formatSize,
  hasRenderedView,
  isTextKind,
  languageFor,
  previewKind,
  relativeTime
} from './fileTypes'
import { MarkdownViewer } from './viewers/MarkdownViewer'
import { TextViewer } from './viewers/TextViewer'
import { HtmlViewer } from './viewers/HtmlViewer'
import { ImageViewer } from './viewers/ImageViewer'
import { TextEditor } from './viewers/TextEditor'
import { NoPreview } from './viewers/NoPreview'

// pdf.js and docx-preview are large and only needed when such a file is opened.
// eslint-disable-next-line no-restricted-syntax -- lazy chunk: pdf.js
const PdfViewer = lazy(() => import('./viewers/PdfViewer'))
// eslint-disable-next-line no-restricted-syntax -- lazy chunk: docx-preview
const DocxViewer = lazy(() => import('./viewers/DocxViewer'))

type FilePreviewProps = {
  root: string
  path: string
  entry?: WorkspaceEntry
  /** Bumped (to the time of the change) when this file changed on disk. */
  changeToken: number
  revealLabel: string
  onDirtyChange: (dirty: boolean) => void
  /** Open the file in its own window ("Pop out"); no button without it. */
  onPopOut?: () => void
}

type TextState =
  | { status: 'loading' }
  | { status: 'ready'; file: WorkspaceTextFile }
  | { status: 'notText' }
  | { status: 'error'; message: string }

const errorMessage = (e: unknown): string => (e instanceof Error ? e.message : String(e))

const toolbarButton =
  'inline-flex items-center gap-1 px-2 py-0.5 rounded-control text-xs text-ink-muted border border-transparent hover:bg-raised hover:text-ink whitespace-nowrap'

export const FilePreview: React.FC<FilePreviewProps> = ({
  root,
  path,
  entry,
  changeToken,
  revealLabel,
  onDirtyChange,
  onPopOut
}) => {
  const { t, i18n } = useTranslation()
  const name = baseName(path)
  const ext = entry?.ext ?? extOf(path)
  const kind = previewKind(ext)
  const textLike = isTextKind(kind)

  const [text, setText] = useState<TextState>({ status: 'loading' })
  const [view, setView] = useState<'rendered' | 'source'>('rendered')
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState('')
  const [saved, setSaved] = useState('')
  const [saving, setSaving] = useState(false)
  const [conflict, setConflict] = useState(false)
  const [changedOnDisk, setChangedOnDisk] = useState(false)
  // mtime of what the editor's draft is based on (sent as expectedModifiedMs).
  const baseModified = useRef<number | undefined>(undefined)

  const dirty = editing && draft !== saved
  useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange])

  const load = useCallback(async (): Promise<WorkspaceTextFile | null> => {
    try {
      const file = await window.workspaceFiles.readText(path)
      setText({ status: 'ready', file })
      return file
    } catch (e) {
      const message = errorMessage(e)
      setText(
        /not a text file/i.test(message) ? { status: 'notText' } : { status: 'error', message }
      )
      return null
    }
  }, [path])

  // A different file: start over.
  useEffect(() => {
    setView('rendered')
    setEditing(false)
    setConflict(false)
    setChangedOnDisk(false)
    setText({ status: 'loading' })
    if (textLike) void load()
  }, [path, textLike, load])

  // The file changed on disk.
  const editingRef = useRef({ editing, dirty })
  editingRef.current = { editing, dirty }
  useEffect(() => {
    if (!changeToken || !textLike) return
    void (async () => {
      const { editing: isEditing, dirty: isDirty } = editingRef.current
      if (!isEditing) {
        await load()
        return
      }
      let file: WorkspaceTextFile
      try {
        file = await window.workspaceFiles.readText(path)
      } catch {
        return
      }
      // Our own save coming back from the watcher.
      if (file.modifiedMs === baseModified.current) return
      if (isDirty) {
        setChangedOnDisk(true)
      } else {
        setText({ status: 'ready', file })
        setDraft(file.content)
        setSaved(file.content)
        baseModified.current = file.modifiedMs
      }
    })()
  }, [changeToken])

  const file = text.status === 'ready' ? text.file : undefined
  const size = file?.size ?? entry?.size
  const modifiedMs = file?.modifiedMs ?? entry?.modifiedMs
  const version = Math.max(entry?.modifiedMs ?? 0, changeToken)
  const url = window.workspaceFiles.fileUrl(path, version || undefined)

  const startEdit = () => {
    if (!file) return
    setDraft(file.content)
    setSaved(file.content)
    baseModified.current = file.modifiedMs
    setConflict(false)
    setChangedOnDisk(false)
    setEditing(true)
  }

  const cancelEdit = () => {
    if (dirty && !window.confirm(t('filesPanel.discardChanges'))) return
    setEditing(false)
    setConflict(false)
    setChangedOnDisk(false)
  }

  const save = async (force = false) => {
    setSaving(true)
    try {
      const content = draft
      const result = await window.workspaceFiles.writeText(
        path,
        content,
        force ? undefined : baseModified.current
      )
      baseModified.current = result.modifiedMs
      setSaved(content)
      setConflict(false)
      setChangedOnDisk(false)
      setText({
        status: 'ready',
        file: {
          content,
          truncated: false,
          size: new TextEncoder().encode(content).length,
          modifiedMs: result.modifiedMs
        }
      })
      toast.success(t('filesPanel.saved'))
    } catch (e) {
      const message = errorMessage(e)
      if (message.startsWith(WORKSPACE_WRITE_CONFLICT)) setConflict(true)
      else toast.error(t('filesPanel.saveFailed', { error: message }))
    } finally {
      setSaving(false)
    }
  }

  const reloadFromDisk = async () => {
    const fresh = await load()
    if (fresh) {
      setDraft(fresh.content)
      setSaved(fresh.content)
      baseModified.current = fresh.modifiedMs
    }
    setConflict(false)
    setChangedOnDisk(false)
  }

  const openExternal = () =>
    window.workspaceFiles.open(path).catch((e) => toast.error(errorMessage(e)))
  const reveal = () => window.workspaceFiles.reveal(path).catch((e) => toast.error(errorMessage(e)))
  const copyPath = async () => {
    try {
      await navigator.clipboard.writeText(toAbsolutePath(root, path))
      toast.success(t('filesPanel.pathCopied'))
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  const metaParts: string[] = []
  if (size !== undefined) metaParts.push(formatSize(size))
  if (modifiedMs) {
    const when = relativeTime(modifiedMs, Date.now(), i18n.language) ?? t('filesPanel.justNow')
    metaParts.push(t('filesPanel.modified', { time: when }))
  }

  const canEdit = textLike && !!file && !file.truncated && !editing

  const body = (): React.ReactNode => {
    if (editing) {
      return (
        <TextEditor
          value={draft}
          onChange={setDraft}
          onSave={() => void save()}
          onCancel={cancelEdit}
          dirty={dirty}
          saving={saving}
        />
      )
    }
    if (kind === 'image') return <ImageViewer url={url} name={name} />
    if (kind === 'pdf') return <PdfViewer url={url} />
    if (kind === 'docx') return <DocxViewer url={url} />
    if (kind === 'binary' || text.status === 'notText') {
      return (
        <NoPreview
          message={text.status === 'notText' ? t('filesPanel.notText') : undefined}
          onOpen={() => void openExternal()}
          onReveal={() => void reveal()}
          revealLabel={revealLabel}
        />
      )
    }
    if (text.status === 'loading') {
      return <div className="p-4 text-sm text-ink-faint">{t('filesPanel.loading')}</div>
    }
    if (text.status === 'error') {
      return (
        <div className="p-4 text-sm text-danger">
          {t('filesPanel.readFailed', { error: text.message })}
        </div>
      )
    }
    const content = text.file.content
    if (kind === 'markdown' && view === 'rendered') return <MarkdownViewer content={content} />
    if (kind === 'html' && view === 'rendered') return <HtmlViewer content={content} />
    return <TextViewer content={content} language={languageFor(name, ext)} />
  }

  return (
    <div className="h-full flex flex-col min-w-0">
      <div className="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-subtle">
        <div className="min-w-0 flex items-baseline gap-2 mr-auto">
          <span className="text-sm font-medium text-ink truncate" title={path}>
            {name}
          </span>
          {dirty && (
            <span
              className="size-1.5 rounded-full bg-warning shrink-0 self-center"
              title={t('filesPanel.unsaved')}
            />
          )}
          <span className="text-micro font-normal tracking-normal text-ink-faint whitespace-nowrap truncate">
            {metaParts.join(' · ')}
          </span>
        </div>
        {hasRenderedView(kind) && !editing && text.status === 'ready' && (
          <button
            className={toolbarButton}
            onClick={() => setView(view === 'rendered' ? 'source' : 'rendered')}
            title={view === 'rendered' ? t('filesPanel.source') : t('filesPanel.rendered')}
          >
            {view === 'rendered' ? (
              <VscCode className="size-3.5" />
            ) : (
              <VscEye className="size-3.5" />
            )}
            {view === 'rendered' ? t('filesPanel.source') : t('filesPanel.rendered')}
          </button>
        )}
        {canEdit && (
          <button className={toolbarButton} onClick={startEdit} title={t('filesPanel.edit')}>
            <FiEdit2 className="size-3.5" />
            {t('filesPanel.edit')}
          </button>
        )}
        {onPopOut && (
          <button
            className={toolbarButton}
            onClick={onPopOut}
            title={t('filesPanel.popOutTooltip')}
          >
            <FiMaximize2 className="size-3.5" />
            {t('filesPanel.popOut')}
          </button>
        )}
        <button
          className={`${toolbarButton} !border-subtle`}
          onClick={() => void openExternal()}
          title={t('filesPanel.openTooltip')}
        >
          <FiExternalLink className="size-3.5" />
          {t('filesPanel.open')}
        </button>
        <button
          className={toolbarButton}
          onClick={() => void reveal()}
          title={t('filesPanel.revealTooltip')}
        >
          <FiFolder className="size-3.5" />
          {t('filesPanel.reveal')}
        </button>
        <button
          className={toolbarButton}
          onClick={() => void copyPath()}
          title={t('filesPanel.copyPath')}
          aria-label={t('filesPanel.copyPath')}
        >
          <FiCopy className="size-3.5" />
        </button>
      </div>

      {file?.truncated && !editing && (
        <div className="px-3 py-1.5 text-xs bg-warning-soft text-warning border-b border-subtle">
          {t('filesPanel.truncated', { size: formatSize(file.content.length) })}
        </div>
      )}
      {editing && (conflict || changedOnDisk) && (
        <div className="flex items-center gap-2 px-3 py-1.5 text-xs bg-warning-soft text-warning border-b border-subtle">
          <span className="mr-auto">
            {conflict ? t('filesPanel.conflict') : t('filesPanel.changedOnDisk')}
          </span>
          {conflict ? (
            <button
              className="px-2 py-0.5 rounded-control bg-warning text-accent-fg hover:opacity-90"
              onClick={() => void save(true)}
              disabled={saving}
            >
              {t('filesPanel.overwrite')}
            </button>
          ) : (
            <button
              className="px-2 py-0.5 rounded-control hover:bg-warning-soft-strong"
              onClick={() => setChangedOnDisk(false)}
            >
              {t('filesPanel.keepMine')}
            </button>
          )}
          <button
            className="px-2 py-0.5 rounded-control border border-current hover:bg-warning-soft-strong"
            onClick={() => void reloadFromDisk()}
          >
            {t('filesPanel.reloadFromDisk')}
          </button>
        </div>
      )}

      <div className={`flex-1 min-h-0 ${kind === 'pdf' || editing ? '' : 'overflow-auto'}`}>
        <Suspense
          fallback={<div className="p-4 text-sm text-ink-faint">{t('filesPanel.loading')}</div>}
        >
          {body()}
        </Suspense>
      </div>
    </div>
  )
}
