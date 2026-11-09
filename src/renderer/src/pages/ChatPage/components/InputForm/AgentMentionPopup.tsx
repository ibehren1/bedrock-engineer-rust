import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { TbRobot } from 'react-icons/tb'
import { FiDatabase } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import type { CaretCoordinates } from './caretCoordinates'
import { mentionItemKey, type MentionItem } from './useAgentMention'

type AgentMentionPopupProps = {
  items: MentionItem[]
  activeIndex: number
  coordinates: CaretCoordinates
  /** Anchor for the caret offsets, which are relative to the textarea's box. */
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  onSelect: (item: MentionItem) => void
}

/** Gap between the caret line and the popup. */
const CARET_GAP = 4
/** Keep the popup this far from the window edges. */
const VIEWPORT_MARGIN = 8
const POPUP_WIDTH = 288
const MAX_POPUP_HEIGHT = 320
/** Never shrink the list below this, even when neither side has room. */
const MIN_USABLE_HEIGHT = 96

type Placement = {
  top: number
  left: number
  maxHeight: number
}

const renderIcon = (agent: CustomAgent) => (
  <AgentIconView
    icon={agent.icon}
    iconColor={agent.iconColor}
    className="w-4 h-4"
    fallback={<TbRobot className="w-4 h-4" />}
  />
)

/**
 * Caret-anchored agent and knowledge base picker for `@` mentions.
 *
 * Rendered into `document.body` and positioned `fixed` against the viewport:
 * the chat input sits at the bottom of the window, so a popup placed inside the
 * textarea's container is clipped (or offset) by its ancestors. It opens above
 * the caret whenever the whole list fits there, and only drops below the caret
 * when that side has more room.
 *
 * Keyboard handling lives in the textarea (it owns the key events), so this is
 * presentational: it renders the list, keeps the active row in view, and
 * reports clicks.
 */
export const AgentMentionPopup: React.FC<AgentMentionPopupProps> = ({
  items,
  activeIndex,
  coordinates,
  textareaRef,
  onSelect
}) => {
  const { t } = useTranslation()
  const activeItemRef = useRef<HTMLButtonElement>(null)
  const popupRef = useRef<HTMLDivElement>(null)
  const [placement, setPlacement] = useState<Placement | null>(null)

  const measure = useCallback(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    const rect = textarea.getBoundingClientRect()
    const caretTop = rect.top + coordinates.top
    const caretBottom = caretTop + coordinates.lineHeight

    const spaceBelow = window.innerHeight - caretBottom - CARET_GAP - VIEWPORT_MARGIN
    const spaceAbove = caretTop - CARET_GAP - VIEWPORT_MARGIN

    // Full list height (scrollHeight ignores the current max-height)
    const contentHeight = popupRef.current?.scrollHeight ?? MAX_POPUP_HEIGHT
    const desired = Math.min(MAX_POPUP_HEIGHT, contentHeight)

    // The input is anchored to the bottom of the window, so open above the
    // caret when the list fits there; otherwise use whichever side is roomier.
    const placeBelow = spaceAbove < desired && spaceBelow > spaceAbove
    const available = placeBelow ? spaceBelow : spaceAbove
    const maxHeight = Math.max(MIN_USABLE_HEIGHT, Math.min(desired, available))

    const top = placeBelow ? caretBottom + CARET_GAP : caretTop - CARET_GAP - maxHeight

    const left = Math.min(
      Math.max(VIEWPORT_MARGIN, rect.left + coordinates.left),
      window.innerWidth - POPUP_WIDTH - VIEWPORT_MARGIN
    )

    setPlacement({
      top: Math.max(VIEWPORT_MARGIN, top),
      left: Math.max(VIEWPORT_MARGIN, left),
      maxHeight
    })
  }, [coordinates.left, coordinates.lineHeight, coordinates.top, textareaRef])

  // Measure before paint so the popup never renders at a stale position
  useLayoutEffect(measure, [measure, items.length])

  useEffect(() => {
    window.addEventListener('resize', measure)
    // Capture phase so scrolling in any ancestor re-anchors the popup
    window.addEventListener('scroll', measure, true)
    return () => {
      window.removeEventListener('resize', measure)
      window.removeEventListener('scroll', measure, true)
    }
  }, [measure])

  // Keep the highlighted row visible while arrowing through a long list
  useEffect(() => {
    activeItemRef.current?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex])

  return createPortal(
    <div
      ref={popupRef}
      className="fixed z-50 overflow-y-auto overscroll-contain rounded-container border border-subtle
        bg-surface py-1 shadow-lg border-subtle"
      style={{
        top: placement?.top ?? 0,
        left: placement?.left ?? 0,
        width: POPUP_WIDTH,
        maxHeight: placement?.maxHeight ?? MAX_POPUP_HEIGHT,
        // Avoid a one-frame flash at the wrong position before measuring
        visibility: placement ? 'visible' : 'hidden'
      }}
      role="listbox"
      aria-label={t('textarea.mention.ariaLabel')}
    >
      {items.map((item, index) => {
        const heading =
          index === 0 || items[index - 1].kind !== item.kind
            ? t(
                item.kind === 'agent'
                  ? 'textarea.mention.agents'
                  : 'textarea.mention.knowledgeBases'
              )
            : null
        const label = item.kind === 'agent' ? item.agent.name : item.kb.kbName
        const tooltip =
          item.kind === 'agent'
            ? item.agent.description || item.agent.name
            : [item.kb.mention, item.kb.description].filter(Boolean).join('\n')
        return (
          <React.Fragment key={mentionItemKey(item)}>
            {heading && (
              <div className="px-3 pt-1.5 pb-0.5 text-micro font-medium uppercase tracking-wide text-ink-faint">
                {heading}
              </div>
            )}
            <button
              ref={index === activeIndex ? activeItemRef : undefined}
              type="button"
              role="option"
              aria-selected={index === activeIndex}
              // Description lives in the native tooltip to keep rows single-line
              title={tooltip}
              // The textarea must keep focus, so don't let mousedown blur it
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => onSelect(item)}
              className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors ${
                index === activeIndex ? 'bg-accent-tint' : 'hover:bg-surface-2'
              }`}
            >
              <span className="shrink-0 text-ink-muted">
                {item.kind === 'agent' ? (
                  renderIcon(item.agent)
                ) : (
                  <FiDatabase className="w-4 h-4" />
                )}
              </span>
              <span className="truncate text-ink">{label}</span>
              {item.kind === 'kb' && (
                <span className="ml-auto shrink-0 max-w-[45%] truncate text-xs text-ink-muted">
                  {item.kb.isOwn ? t('textarea.mention.thisAgent') : item.kb.agentName}
                </span>
              )}
            </button>
          </React.Fragment>
        )
      })}
    </div>,
    document.body
  )
}
