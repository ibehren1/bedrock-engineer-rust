import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiFolder, FiX } from 'react-icons/fi'
import { FaDocker } from 'react-icons/fa'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import { SandboxPanel } from '../SandboxPanel'
import { FilesTab, type FilesOpenRequest } from './FilesTab'

export type RightPanelTab = 'files' | 'sandbox'

export const RIGHT_PANEL_MIN_WIDTH = 360
export const RIGHT_PANEL_MAX_WIDTH = 1000
export const RIGHT_PANEL_DEFAULT_WIDTH = 640

export const clampPanelWidth = (width: unknown): number => {
  const n = typeof width === 'number' && Number.isFinite(width) ? width : RIGHT_PANEL_DEFAULT_WIDTH
  return Math.round(Math.min(RIGHT_PANEL_MAX_WIDTH, Math.max(RIGHT_PANEL_MIN_WIDTH, n)))
}

type RightPanelProps = {
  isOpen: boolean
  tab: RightPanelTab
  onTabChange: (tab: RightPanelTab) => void
  onClose: () => void
  projectPath: string
  sessionId?: string
  filesOpenRequest?: FilesOpenRequest
  sandbox: {
    status: ChatSandboxStatus
    isBusy: boolean
    onStart: () => void
    onStop: () => void
    onRemove: (deleteData: boolean) => void
    onOpenFolder: () => void
  }
}

/**
 * The chat page's right panel: Files (the project folder) and, for chats that have one, the
 * Docker sandbox. Resizable from its left edge; the width is saved in the store
 * (`rightPanelWidth`).
 */
export const RightPanel: React.FC<RightPanelProps> = ({
  isOpen,
  tab,
  onTabChange,
  onClose,
  projectPath,
  sessionId,
  filesOpenRequest,
  sandbox
}) => {
  const { t } = useTranslation()
  const [width, setWidth] = useState(() => clampPanelWidth(window.store.get('rightPanelWidth')))
  const [dragging, setDragging] = useState(false)
  const drag = useRef<{ startX: number; startWidth: number } | null>(null)
  const widthRef = useRef(width)
  widthRef.current = width

  const hasSandbox = sandbox.status.exists
  const activeTab: RightPanelTab = tab === 'sandbox' && hasSandbox ? 'sandbox' : 'files'

  // The Files tab mounts on first use, then stays mounted to keep its tree and editor state.
  const [filesMounted, setFilesMounted] = useState(false)
  useEffect(() => {
    if (isOpen && activeTab === 'files') setFilesMounted(true)
  }, [isOpen, activeTab])

  const onPointerMove = useCallback((e: PointerEvent) => {
    if (!drag.current) return
    // The handle is on the left edge: dragging left widens the panel.
    setWidth(clampPanelWidth(drag.current.startWidth + (drag.current.startX - e.clientX)))
  }, [])

  const endDrag = useCallback(() => {
    if (!drag.current) return
    drag.current = null
    setDragging(false)
    window.store.set('rightPanelWidth', widthRef.current)
  }, [])

  useEffect(() => {
    if (!dragging) return
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', endDrag)
    window.addEventListener('pointercancel', endDrag)
    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', endDrag)
      window.removeEventListener('pointercancel', endDrag)
    }
  }, [dragging, onPointerMove, endDrag])

  const startDrag = (e: React.PointerEvent) => {
    e.preventDefault()
    drag.current = { startX: e.clientX, startWidth: widthRef.current }
    setDragging(true)
  }

  const tabButton = (key: RightPanelTab, label: string, icon: React.ReactNode) => (
    <button
      key={key}
      onClick={() => onTabChange(key)}
      className={`inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs -mb-px border-b-2 ${
        activeTab === key
          ? 'text-ink border-accent'
          : 'text-ink-faint border-transparent hover:text-ink'
      }`}
    >
      {icon}
      {label}
    </button>
  )

  return (
    // Width animates open/closed; the inner box keeps the full width so the content does not
    // reflow during the animation. No transition while dragging, so the edge follows the
    // pointer. pb-52 keeps the panel clear of the fixed input box, as the message list does.
    // This wrapper must stay unpositioned: a positioned box later in the DOM paints over the
    // fixed input box, and its canvas-coloured pb-52 strip would hide the right of the input.
    <div
      className={`bg-canvas flex-shrink-0 overflow-hidden pb-52 ${
        dragging ? '' : 'transition-all duration-300 ease-in-out'
      }`}
      style={{ width: isOpen ? width : 0 }}
    >
      {/* While dragging, iframes and canvases in the panel must not swallow pointer events. */}
      {dragging && <div className="fixed inset-0 z-50 cursor-col-resize" />}
      <div
        className="relative h-full flex flex-col bg-surface border border-subtle rounded-container shadow-raised overflow-hidden"
        style={{ width }}
      >
        <div
          onPointerDown={startDrag}
          title={t('filesPanel.resize')}
          className="group absolute left-0 top-0 bottom-0 w-1.5 cursor-col-resize z-10 flex items-center justify-center"
        >
          <span
            className={`h-10 w-px rounded-full ${dragging ? 'bg-accent' : 'bg-raised group-hover:bg-accent'}`}
          />
        </div>

        <div className="flex items-center gap-0.5 pl-2 pr-1.5 border-b border-subtle">
          {tabButton('files', t('filesPanel.tabFiles'), <FiFolder className="size-3.5" />)}
          {hasSandbox &&
            tabButton(
              'sandbox',
              t('filesPanel.tabSandbox'),
              <FaDocker className="size-3.5 text-[#2496ED]" />
            )}
          <button
            onClick={onClose}
            className="ml-auto p-1 rounded-control text-ink-muted hover:bg-raised hover:text-ink"
            title={t('filesPanel.close')}
            aria-label={t('filesPanel.close')}
          >
            <FiX className="size-3.5" />
          </button>
        </div>

        <div className="flex-1 min-h-0">
          <div className={activeTab === 'files' ? 'h-full' : 'hidden'}>
            {filesMounted && (
              <FilesTab
                projectPath={projectPath}
                sessionId={sessionId}
                active={isOpen && activeTab === 'files'}
                openRequest={filesOpenRequest}
              />
            )}
          </div>
          {/* Kept mounted while the chat has a sandbox, so an open shell survives tab
              switches. */}
          {hasSandbox && (
            <div className={activeTab === 'sandbox' ? 'h-full' : 'hidden'}>
              <SandboxPanel
                embedded
                sessionId={sessionId}
                status={sandbox.status}
                isBusy={sandbox.isBusy}
                isOpen={isOpen && activeTab === 'sandbox'}
                onStart={sandbox.onStart}
                onStop={sandbox.onStop}
                onRemove={sandbox.onRemove}
                onOpenFolder={sandbox.onOpenFolder}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
