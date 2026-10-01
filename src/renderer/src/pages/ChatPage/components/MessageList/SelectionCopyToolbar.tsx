import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiCopy } from 'react-icons/fi'
import { LuFileText } from 'react-icons/lu'
import { copySelectionAsMarkdown, copySelectionAsRichText } from './selectionCopy'

type SelectionCopyToolbarProps = {
  /** The messages list container. Selections are only actioned when they fall inside it. */
  containerRef: RefObject<HTMLElement>
}

type ToolbarState = {
  visible: boolean
  /** Viewport coordinates (position: fixed) for the toolbar's horizontal center / bottom. */
  x: number
  y: number
}

const HIDDEN: ToolbarState = { visible: false, x: 0, y: 0 }

// Walk up from a node to check whether it sits inside a rendered message-text block.
function isWithinMessageText(node: Node | null, container: HTMLElement): boolean {
  let el = node instanceof Element ? node : node?.parentElement ?? null
  while (el && el !== container) {
    if (el instanceof HTMLElement && el.dataset.messageText === 'true') return true
    el = el.parentElement
  }
  return false
}

/**
 * A small floating toolbar that appears above a text selection made inside a chat message,
 * offering "Copy (markdown)" and "Copy (rich text)" scoped to just the highlighted text.
 * A single instance is mounted for the whole message list.
 */
export function SelectionCopyToolbar({ containerRef }: SelectionCopyToolbarProps) {
  const { t } = useTranslation()
  const [state, setState] = useState<ToolbarState>(HIDDEN)
  const toolbarRef = useRef<HTMLDivElement>(null)

  const hide = useCallback(() => setState((prev) => (prev.visible ? HIDDEN : prev)), [])

  const evaluateSelection = useCallback(() => {
    const container = containerRef.current
    if (!container) return
    const selection = window.getSelection()
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
      hide()
      return
    }
    if (!selection.toString().trim()) {
      hide()
      return
    }
    const range = selection.getRangeAt(0)
    const anchor = range.commonAncestorContainer
    if (!container.contains(anchor) || !isWithinMessageText(anchor, container)) {
      hide()
      return
    }
    const rect = range.getBoundingClientRect()
    if (rect.width === 0 && rect.height === 0) {
      hide()
      return
    }
    setState({
      visible: true,
      x: rect.left + rect.width / 2,
      y: rect.top // toolbar is rendered above this via a translate
    })
  }, [containerRef, hide])

  useEffect(() => {
    // mouseup covers finishing a drag-selection; selectionchange covers keyboard selection
    // and collapse. A click inside the toolbar preventing default (below) keeps the selection.
    const onMouseUp = () => evaluateSelection()
    const onSelectionChange = () => {
      const selection = window.getSelection()
      if (!selection || selection.isCollapsed) hide()
    }
    const onScroll = () => hide()
    const onResize = () => hide()

    document.addEventListener('mouseup', onMouseUp)
    document.addEventListener('selectionchange', onSelectionChange)
    // Hide when the messages scroll (position would otherwise drift) — capture phase to catch
    // the inner scroll container too.
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onResize)
    return () => {
      document.removeEventListener('mouseup', onMouseUp)
      document.removeEventListener('selectionchange', onSelectionChange)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onResize)
    }
  }, [evaluateSelection, hide])

  const handleCopy = useCallback(
    (copyFn: () => Promise<boolean>) => async () => {
      const ok = await copyFn()
      if (ok) {
        toast.success(t('Selection copied to clipboard'))
      } else {
        toast.error(t('Failed to copy message'))
      }
      hide()
      window.getSelection()?.removeAllRanges()
    },
    [t, hide]
  )

  if (!state.visible) return null

  return createPortal(
    <div
      ref={toolbarRef}
      // Center horizontally on the selection and sit just above it.
      style={{
        position: 'fixed',
        left: state.x,
        top: state.y,
        transform: 'translate(-50%, calc(-100% - 8px))',
        zIndex: 60
      }}
      // Prevent the toolbar itself from clearing the selection when interacted with.
      onMouseDown={(e) => e.preventDefault()}
      className="flex items-center gap-1 bg-surface rounded-container shadow-lg
        border border-subtle p-1 whitespace-nowrap"
    >
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={handleCopy(copySelectionAsMarkdown)}
        className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-control hover:bg-raised
          hover:bg-raised text-ink"
      >
        <FiCopy className="text-accent" />
        <span>{t('Copy (markdown)')}</span>
      </button>
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={handleCopy(copySelectionAsRichText)}
        className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-control hover:bg-raised
          hover:bg-raised text-ink"
      >
        <LuFileText className="text-accent" />
        <span>{t('Copy (rich text)')}</span>
      </button>
    </div>,
    document.body
  )
}
