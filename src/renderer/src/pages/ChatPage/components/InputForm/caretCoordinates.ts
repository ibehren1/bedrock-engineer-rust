/**
 * Measure the pixel position of a textarea's caret.
 *
 * A textarea gives no API for this, so the standard trick is a hidden div that
 * mirrors the textarea's text metrics: put the text before the caret in it,
 * append a marker span, and read the span's offset.
 */

const MIRRORED_STYLES = [
  'fontFamily',
  'fontSize',
  'fontWeight',
  'fontStyle',
  'fontVariant',
  'letterSpacing',
  'lineHeight',
  'textTransform',
  'textIndent',
  'paddingTop',
  'paddingRight',
  'paddingBottom',
  'paddingLeft',
  'borderTopWidth',
  'borderRightWidth',
  'borderBottomWidth',
  'borderLeftWidth',
  'boxSizing'
] as const

export type CaretCoordinates = {
  /** Offset from the textarea's top edge, already adjusted for scroll. */
  top: number
  left: number
  /** Line height at the caret, so callers can place a popup below the line. */
  lineHeight: number
}

export function getCaretCoordinates(
  textarea: HTMLTextAreaElement,
  position: number
): CaretCoordinates {
  const computed = window.getComputedStyle(textarea)
  const mirror = document.createElement('div')

  mirror.style.position = 'absolute'
  mirror.style.visibility = 'hidden'
  mirror.style.whiteSpace = 'pre-wrap'
  mirror.style.wordWrap = 'break-word'
  mirror.style.overflowWrap = 'break-word'
  mirror.style.top = '0'
  mirror.style.left = '-9999px'
  mirror.style.width = `${textarea.clientWidth}px`

  for (const property of MIRRORED_STYLES) {
    mirror.style[property] = computed[property]
  }

  mirror.textContent = textarea.value.substring(0, position)

  const marker = document.createElement('span')
  // A zero-width space would collapse; a real character keeps the span measurable.
  marker.textContent = '.'
  mirror.appendChild(marker)

  document.body.appendChild(mirror)
  const top = marker.offsetTop - textarea.scrollTop
  const left = marker.offsetLeft
  document.body.removeChild(mirror)

  const lineHeight = parseInt(computed.lineHeight, 10) || parseInt(computed.fontSize, 10) * 1.4

  return { top, left, lineHeight }
}
