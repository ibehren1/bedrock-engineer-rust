import React, { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiCopy, FiFolder, FiMaximize2 } from 'react-icons/fi'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { tomorrow } from 'react-syntax-highlighter/dist/esm/styles/prism'
import toast from 'react-hot-toast'
import { Mermaid } from '../Code/Mermaid'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import { buildComposeDiagram } from './composeDiagram'

type ComposeTabProps = {
  sessionId?: string
  status: ChatSandboxStatus
  active: boolean
  onOpenFolder: () => void
}

interface ComposeFile {
  composeless: boolean
  path?: string
  contents?: string
  error?: string
}

export const ComposeTab: React.FC<ComposeTabProps> = ({
  sessionId,
  status,
  active,
  onOpenFolder
}) => {
  const { t } = useTranslation()
  const [file, setFile] = useState<ComposeFile | null>(null)

  useEffect(() => {
    if (!active || !sessionId) return
    let cancelled = false

    void window.api.dockerSandbox
      .compose(sessionId)
      .then((result) => {
        if (!cancelled) setFile(result)
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFile({
            composeless: false,
            error: error instanceof Error ? error.message : String(error)
          })
        }
      })

    return () => {
      cancelled = true
    }
    // Re-read when the container list changes: `docker compose up` rewrites nothing, but a
    // recreated sandbox does, and the diagram's states come from this same status.
  }, [active, sessionId, status.state, status.containers.length])

  // Cheap enough to rebuild every render. The <MermaidCore> below is keyed on this string,
  // so an unchanged diagram keeps the existing SVG and only a real change re-renders it.
  const diagram = buildComposeDiagram(status)

  const copy = async () => {
    if (!file?.contents) return
    await navigator.clipboard.writeText(file.contents)
    toast.success(t('dockerSandbox.compose.copied'))
  }

  if (file?.composeless) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2 p-7 text-center">
        <p className="m-0 text-sm font-medium text-ink">{t('dockerSandbox.compose.noneTitle')}</p>
        <p className="m-0 max-w-[26rem] text-xs text-ink-muted">
          {t('dockerSandbox.compose.noneBody')}
        </p>
      </div>
    )
  }

  return (
    <div className="text-sm">
      <section className="px-3 py-3 border-b border-subtle">
        <h4 className="m-0 mb-2 text-micro uppercase tracking-wide text-ink-faint font-medium">
          {t('dockerSandbox.compose.layout')}
        </h4>
        {/* <Mermaid> opens the diagram full window on click, with zoom controls and Esc to
            close — the panel is only 40rem wide, so a stack of several services needs it. */}
        <div
          className="bg-surface-2 border border-subtle rounded-container p-2 min-h-[14rem]"
          title={t('dockerSandbox.compose.expandHint')}
        >
          {/* Keyed on the source so a state change re-renders rather than reusing the SVG. */}
          <Mermaid key={diagram} chart={diagram} />
        </div>
        <p className="mt-2 mb-0 text-micro text-ink-faint flex items-center gap-1">
          <FiMaximize2 className="size-3 shrink-0" />
          {t('dockerSandbox.compose.expandHint')}
        </p>
        <p className="mt-1 mb-0 text-micro text-ink-faint">
          {t('dockerSandbox.compose.layoutHint')}
        </p>
      </section>

      <section className="px-3 py-3">
        <div className="flex items-center gap-2 mb-2">
          <h4 className="m-0 text-micro uppercase tracking-wide text-ink-faint font-medium">
            {t('dockerSandbox.compose.file')}
          </h4>
          <div className="ml-auto flex gap-1.5">
            <button
              onClick={() => void copy()}
              disabled={!file?.contents}
              className="inline-flex items-center gap-1 px-2 py-0.5 text-micro rounded-control border border-strong bg-surface-2 text-ink-muted hover:text-ink hover:bg-raised disabled:opacity-50"
            >
              <FiCopy className="size-3" />
              {t('dockerSandbox.compose.copy')}
            </button>
            <button
              onClick={onOpenFolder}
              className="inline-flex items-center gap-1 px-2 py-0.5 text-micro rounded-control border border-strong bg-surface-2 text-ink-muted hover:text-ink hover:bg-raised"
            >
              <FiFolder className="size-3" />
              {t('dockerSandbox.menu.openFolder')}
            </button>
          </div>
        </div>

        {file?.path && (
          <p className="mt-0 mb-1.5 font-mono text-micro text-ink-faint break-all">{file.path}</p>
        )}

        {file?.error ? (
          <p className="m-0 text-xs text-danger">{file.error}</p>
        ) : (
          <SyntaxHighlighter
            language="yaml"
            style={tomorrow}
            customStyle={{
              margin: 0,
              borderRadius: 'var(--radius-container)',
              fontSize: '11.5px',
              maxHeight: '28rem'
            }}
            codeTagProps={{ style: { fontFamily: 'var(--font-mono)' } }}
          >
            {file?.contents ?? ''}
          </SyntaxHighlighter>
        )}
      </section>
    </div>
  )
}
