import React, { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useFilesPanel } from '@renderer/contexts/FilesPanelContext'
import { detectFilePath } from '@renderer/lib/workspaceFiles/paths'
import {
  cachedWorkspacePath,
  resolveWorkspacePath,
  type ResolvedPath
} from '@renderer/lib/workspaceFiles/pathResolver'

/**
 * The project file `text` names, if it does: `undefined` while unknown or when there is no
 * Files panel, `null` when it is not an existing project path.
 */
export function useWorkspacePath(text: string | undefined): ResolvedPath | undefined {
  const panel = useFilesPanel()
  const projectPath = panel?.projectPath ?? ''
  const candidate = text && panel ? detectFilePath(text, projectPath) : null
  const [resolved, setResolved] = useState<ResolvedPath | undefined>(() =>
    candidate ? cachedWorkspacePath(projectPath, candidate) : undefined
  )
  const filesVersion = panel?.filesVersion ?? 0

  useEffect(() => {
    if (!candidate) {
      setResolved(undefined)
      return
    }
    let cancelled = false
    const cached = cachedWorkspacePath(projectPath, candidate)
    if (cached !== undefined) setResolved(cached)
    if (cached) return
    resolveWorkspacePath(projectPath, candidate).then((value) => {
      if (!cancelled) setResolved(value)
    })
    return () => {
      cancelled = true
    }
  }, [candidate, projectPath, filesVersion])

  return candidate ? resolved : undefined
}

type FilePathCodeProps = {
  text: string
  className?: string
  children?: React.ReactNode
}

/** Inline code that opens the Files panel when it names an existing project file. */
export const FilePathCode: React.FC<FilePathCodeProps> = ({ text, className, children }) => {
  const { t } = useTranslation()
  const panel = useFilesPanel()
  const resolved = useWorkspacePath(text)
  if (!panel || !resolved) return <code className={className}>{children}</code>
  return (
    <code
      className={`${className ?? ''} cursor-pointer text-accent underline decoration-dotted underline-offset-2 hover:decoration-solid`}
      role="link"
      tabIndex={0}
      title={t('filesPanel.openInPanel')}
      onClick={() => panel.openFile(resolved.path, resolved.kind)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') panel.openFile(resolved.path, resolved.kind)
      }}
    >
      {children}
    </code>
  )
}

/** A chip naming a project file (tool targets); a button when the file still exists. */
export const FilePathChip: React.FC<{ path: string }> = ({ path }) => {
  const { t } = useTranslation()
  const panel = useFilesPanel()
  const resolved = useWorkspacePath(path)
  const base =
    'inline-flex max-w-full items-center rounded-control px-1.5 py-0.5 font-mono text-micro'
  if (!panel || !resolved) {
    return (
      <span className={`${base} bg-raised text-ink-faint truncate`} title={path}>
        {path}
      </span>
    )
  }
  return (
    <button
      type="button"
      onClick={() => panel.openFile(resolved.path, resolved.kind)}
      className={`${base} bg-accent-tint text-accent hover:bg-accent-tint-strong truncate`}
      title={t('filesPanel.openInPanel')}
    >
      {resolved.path || path}
    </button>
  )
}

type FilePathAnchorProps = {
  href?: string
  title?: string
  children?: React.ReactNode
}

/**
 * A Markdown link. One pointing at an existing project file opens it in the Files panel;
 * anything else opens the target externally, as before.
 */
export const FilePathAnchor: React.FC<FilePathAnchorProps> = ({ href, title, children }) => {
  const panel = useFilesPanel()
  let decoded = href
  try {
    decoded = href ? decodeURI(href) : href
  } catch {
    // keep as is
  }
  const resolved = useWorkspacePath(decoded)
  return (
    <a
      href={href}
      title={title}
      onClick={(e) => {
        e.preventDefault()
        if (panel && resolved) panel.openFile(resolved.path, resolved.kind)
        else if (href) open(href)
      }}
      style={{ cursor: 'pointer' }}
    >
      {children}
    </a>
  )
}
