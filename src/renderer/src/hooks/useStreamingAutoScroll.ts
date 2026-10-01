import { RefObject, useLayoutEffect, useRef } from 'react'

// Distance (px) from the bottom within which we consider the view "pinned" to the bottom.
const BOTTOM_THRESHOLD = 32

// Selector for the element marking the first line of the main streamed answer.
// Only the last assistant message tags this (see MessageList / Message).
const ANSWER_ANCHOR_SELECTOR = '[data-answer-anchor="true"]'

type Options = {
  /** A value that changes whenever the streamed message content changes (length + last block count). */
  signature: string
  /** Whether a response is currently streaming. */
  loading: boolean
}

/**
 * Auto-scroll that follows streaming output through the tool-use phase, then stops
 * once the first line of the main response reaches the top of the scroll container.
 *
 * Behavior:
 * - While streaming, the view is pinned to the bottom so new tool-use steps stay visible.
 * - When a new message or content block appears, following resumes (so each tool step is followed).
 * - Once the answer's first line crosses the top edge, it is aligned to the top and scrolling
 *   stops completely for that block — the rest streams in below without moving the view.
 * - If the user manually scrolls up, auto-scroll pauses until they return to the bottom.
 */
export const useStreamingAutoScroll = (
  containerRef: RefObject<HTMLElement>,
  { signature, loading }: Options
) => {
  // The answer's first line reached the top — stop following for the current content block.
  const stoppedRef = useRef(false)
  // The user scrolled away from the bottom — pause until they return.
  const userPausedRef = useRef(false)
  const prevSignatureRef = useRef(signature)

  // Track manual scrolling so we can pause/resume auto-scroll.
  useLayoutEffect(() => {
    const el = containerRef.current
    if (!el) return

    const onScroll = () => {
      const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      userPausedRef.current = distanceFromBottom > BOTTOM_THRESHOLD
    }

    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [containerRef])

  useLayoutEffect(() => {
    const el = containerRef.current
    if (!el) return

    // New message / content block (e.g. a tool-use step) — resume following.
    if (signature !== prevSignatureRef.current) {
      prevSignatureRef.current = signature
      stoppedRef.current = false
    }

    if (!loading || userPausedRef.current || stoppedRef.current) return

    // Pin to the bottom so streaming tool-use output stays visible.
    el.scrollTop = el.scrollHeight

    // Once the main answer's first line is pinned at/above the top edge, align it to the
    // top and stop following for this block. Reading layout here reflows synchronously.
    const anchor = el.querySelector<HTMLElement>(ANSWER_ANCHOR_SELECTOR)
    if (anchor) {
      const containerTop = el.getBoundingClientRect().top
      const anchorTop = anchor.getBoundingClientRect().top
      if (anchorTop <= containerTop) {
        el.scrollTop += anchorTop - containerTop
        stoppedRef.current = true
      }
    }
  }, [containerRef, signature, loading])
}
