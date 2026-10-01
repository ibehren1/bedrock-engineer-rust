import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { TbRobot } from 'react-icons/tb'
import { useTranslation } from 'react-i18next'
import { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import type { CaretCoordinates } from './caretCoordinates'

type AgentMentionPopupProps = {
  items: CustomAgent[]
  activeIndex: number
  coordinates: CaretCoordinates
  /** Anchor for the caret offsets, which are relative to the textarea's box. */
  textareaRef: React.RefObject<HTMLTextAreaElement>
  onSelect: (agent: CustomAgent) => void
}

/** Gap between the caret line and the popup. */
const CARET_GAP = 4
/** Keep the popup this far from the window edges. */
const VIEWPORT_MARGIN = 8
const POPUP_WIDTH = 288
const MAX_POPUP_HEIGHT = 256
/** Below this the list is too cramped to be useful, so flip to the other side. */
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
 * Caret-anchored agent picker for `@` mentions.
 *
 * Positioned `fixed` against the viewport rather than absolutely inside the
 * textarea's container: the chat input sits at the bottom of the window, so an
 * absolutely-placed popup is clipped by ancestors and runs off screen. Fixed
 * positioning lets it flip above the caret and clamp to the viewport.
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
  const [placement, setPlacement] = useState<Placement | null>(null)

  const measure = useCallback(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    const rect = textarea.getBoundingClientRect()
    const caretTop = rect.top + coordinates.top
    const caretBottom = caretTop + coordinates.lineHeight

    const spaceBelow = window.innerHeight - caretBottom - CARET_GAP - VIEWPORT_MARGIN
    const spaceAbove = caretTop - CARET_GAP - VIEWPORT_MARGIN

    // Prefer below, but flip above when below is too cramped — which is the
    // normal case, since the chat input is anchored to the bottom of the window.
    const placeBelow = spaceBelow >= MIN_USABLE_HEIGHT || spaceBelow >= spaceAbove
    const available = placeBelow ? spaceBelow : spaceAbove
    const maxHeight = Math.max(MIN_USABLE_HEIGHT, Math.min(MAX_POPUP_HEIGHT, available))

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

  return (
    <div
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
      {items.map((agent, index) => (
        <button
          key={agent.id}
          ref={index === activeIndex ? activeItemRef : undefined}
          type="button"
          role="option"
          aria-selected={index === activeIndex}
          // Description lives in the native tooltip to keep rows single-line
          title={agent.description || agent.name}
          // The textarea must keep focus, so don't let mousedown blur it
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onSelect(agent)}
          className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors ${
            index === activeIndex ? 'bg-accent-tint' : 'hover:bg-surface-2'
          }`}
        >
          <span className="shrink-0 text-ink-muted">{renderIcon(agent)}</span>
          <span className="truncate text-ink">{agent.name}</span>
        </button>
      ))}
    </div>
  )
}
