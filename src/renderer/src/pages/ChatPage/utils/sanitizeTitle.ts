/** Hard ceiling for a generated title. Titles sit in a narrow sidebar. */
export const MAX_TITLE_LENGTH = 60

/**
 * Reduce a model-generated chat title to short, plain text.
 *
 * The prompt asks for plain text, but a prompt is a request, not a guarantee:
 * models summarising a technical conversation reach for `backticks`, **bold**,
 * "quotes", a `Title:` prefix, or a trailing full stop, and occasionally answer
 * on several lines. Titles are rendered as raw text in the sidebar and reused as
 * sandbox and attachment folder names, so any of that shows up verbatim.
 *
 * Returns null when nothing usable is left, so callers can fall back to the
 * default timestamp title rather than setting an empty one.
 */
export const sanitizeGeneratedTitle = (raw: string | null | undefined): string | null => {
  if (typeof raw !== 'string') return null

  let text = raw.replace(/\r\n?/g, '\n')

  // Drop fenced code blocks outright rather than keeping their contents.
  text = text.replace(/```[\s\S]*?```/g, ' ').replace(/```/g, ' ')

  // A title is one line. Take the first that has content.
  text =
    text
      .split('\n')
      .map((line) => line.trim())
      .find((line) => line.length > 0) ?? ''

  // Images before links: ![alt](url) -> alt, then [text](url) -> text.
  text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
  text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')

  // Leading block markers: heading, blockquote, list bullet, ordered list.
  text = text.replace(/^\s*(?:#{1,6}\s+|>\s+|[-*+]\s+|\d+[.)]\s+)/, '')

  // Emphasis and inline code. Paired markers only, so snake_case survives.
  text = text
    .replace(/\*\*\*([^*]+)\*\*\*/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/\*([^*]+)\*/g, '$1')
    .replace(/___([^_]+)___/g, '$1')
    .replace(/__([^_]+)__/g, '$1')
    .replace(/(^|\s)_([^_]+)_(?=\s|$)/g, '$1$2')
    .replace(/~~([^~]+)~~/g, '$1')
    .replace(/`([^`]*)`/g, '$1')

  // Stray HTML the model may have wrapped things in.
  text = text.replace(/<[^>]*>/g, ' ')

  // A label the model added rather than part of the title.
  text = text.replace(/^\s*(?:title|chat title|summary)\s*[:\-–]\s*/i, '')

  text = text.replace(/\s+/g, ' ').trim()

  // Wrapping quotes, straight or curly, and leftover emphasis characters at the
  // edges once the pairs above are gone.
  text = text.replace(/^["'`“”‘’*_#>\s]+/, '').replace(/["'`“”‘’*_\s]+$/, '')

  // Trailing sentence punctuation. A question mark can be meaningful, so keep it.
  text = text.replace(/[.,;:!]+$/, '').trim()

  if (!text) return null

  if (text.length > MAX_TITLE_LENGTH) {
    const clipped = text.slice(0, MAX_TITLE_LENGTH)
    // Prefer a word boundary, but only if it does not cost most of the title —
    // CJK has no spaces, so there may be none to find.
    const lastSpace = clipped.lastIndexOf(' ')
    text = (lastSpace > MAX_TITLE_LENGTH * 0.6 ? clipped.slice(0, lastSpace) : clipped).trim()
    text = text.replace(/[.,;:!\-–—]+$/, '').trim()
  }

  return text || null
}
