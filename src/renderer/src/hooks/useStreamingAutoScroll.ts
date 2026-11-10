import { RefObject, useLayoutEffect, useRef } from 'react'

// Distance (px) from the bottom within which we consider the view "pinned" to the bottom.
const BOTTOM_THRESHOLD = 32

// A scroll event whose position is within this many px of the value we last assigned is ours.
const PROGRAMMATIC_TOLERANCE = 2

// Selector for the element marking the first line of the main streamed answer.
// Only the last assistant message tags this (see MessageList / Message).
const ANSWER_ANCHOR_SELECTOR = '[data-answer-anchor="true"]'

type StreamedBlock = {
  text?: string
  reasoningContent?: { reasoningText?: { text?: string } }
  toolUse?: { input?: unknown }
}

type StreamedMessage = { content?: unknown[] } | undefined

/**
 * Keys that drive the auto-scroll from the chat's messages.
 * - `blockKey` changes when a message or content block is added (a new tool-use step).
 * - `signature` also changes whenever the last message grows: answer text, thinking text, or
 *   streamed tool input (a string until the tool call completes).
 */
export const streamScrollKeys = (
  messageCount: number,
  lastMessage: StreamedMessage
): { blockKey: string; signature: string } => {
  const blocks = (lastMessage?.content ?? []) as StreamedBlock[]
  const blockKey = `${messageCount}:${blocks.length}`
  let size = 0
  for (const block of blocks) {
    if (!block || typeof block !== 'object') continue
    if (typeof block.text === 'string') size += block.text.length
    const reasoning = block.reasoningContent?.reasoningText?.text
    if (typeof reasoning === 'string') size += reasoning.length
    const input = block.toolUse?.input
    if (typeof input === 'string') size += input.length
  }
  return { blockKey, signature: `${blockKey}:${size}` }
}

type Options = {
  /** Changes when a message or content block is added (see `streamScrollKeys`). */
  blockKey: string
  /** Changes whenever the streamed content grows (see `streamScrollKeys`). */
  signature: string
  /** Whether a response is currently streaming. */
  loading: boolean
}

/**
 * Auto-scroll that follows streaming output through the tool-use phase, then stops
 * once the first line of the main response reaches the top of the scroll container.
 *
 * Behavior:
 * - Each new turn (loading starts) scrolls to the bottom and resumes following, whatever the
 *   user did before.
 * - While streaming, the view is pinned to the bottom so thinking and tool-use steps stay visible.
 * - When a new message or content block appears, following resumes (so each tool step is followed).
 * - Once the answer's first line crosses the top edge, it is aligned to the top and scrolling
 *   stops for that block — the rest streams in below without moving the view.
 * - If the user manually scrolls up, auto-scroll pauses until they return to the bottom.
 *   Scrolls the hook makes itself never count as the user's.
 */
export const useStreamingAutoScroll = (
  containerRef: RefObject<HTMLElement | null>,
  { blockKey, signature, loading }: Options
) => {
  // The answer's first line reached the top — stop following for the current content block.
  const stoppedRef = useRef(false)
  // The user scrolled away from the bottom — pause until they return.
  const userPausedRef = useRef(false)
  // The scrollTop the hook last assigned, so its own scroll events are ignored.
  const programmaticTopRef = useRef<number | null>(null)
  const prevBlockKeyRef = useRef(blockKey)
  const prevLoadingRef = useRef(loading)

  // Track manual scrolling so we can pause/resume auto-scroll.
  useLayoutEffect(() => {
    const el = containerRef.current
    if (!el) return

    const onScroll = () => {
      const ours = programmaticTopRef.current
      if (ours !== null && Math.abs(el.scrollTop - ours) < PROGRAMMATIC_TOLERANCE) return
      programmaticTopRef.current = null
      const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      userPausedRef.current = distanceFromBottom > BOTTOM_THRESHOLD
    }

    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [containerRef])

  useLayoutEffect(() => {
    const el = containerRef.current
    if (!el) return

    const setTop = (top: number) => {
      el.scrollTop = top
      // Read back: the browser clamps to the scrollable range.
      programmaticTopRef.current = el.scrollTop
    }

    // A new turn started — follow it regardless of earlier scrolling.
    if (loading && !prevLoadingRef.current) {
      userPausedRef.current = false
      stoppedRef.current = false
    }
    prevLoadingRef.current = loading

    // New message / content block (e.g. a tool-use step) — resume following.
    if (blockKey !== prevBlockKeyRef.current) {
      prevBlockKeyRef.current = blockKey
      stoppedRef.current = false
    }

    if (!loading || userPausedRef.current || stoppedRef.current) return

    // Pin to the bottom so streaming output stays visible.
    setTop(el.scrollHeight)

    // Once the main answer's first line is pinned at/above the top edge, align it to the
    // top and stop following for this block. Reading layout here reflows synchronously.
    const anchor = el.querySelector<HTMLElement>(ANSWER_ANCHOR_SELECTOR)
    if (anchor) {
      const containerTop = el.getBoundingClientRect().top
      const anchorTop = anchor.getBoundingClientRect().top
      if (anchorTop <= containerTop) {
        setTop(el.scrollTop + anchorTop - containerTop)
        stoppedRef.current = true
      }
    }
  }, [containerRef, blockKey, signature, loading])
}
