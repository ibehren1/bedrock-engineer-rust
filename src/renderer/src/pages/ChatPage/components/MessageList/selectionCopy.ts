import TurndownService from 'turndown'
import { gfm } from 'turndown-plugin-gfm'

// A chat message's text is rendered from markdown to HTML (react-markdown), so a user's
// text selection lives in the rendered DOM — not in the original markdown source. To offer
// "Copy (markdown)" / "Copy (rich text)" scoped to just the selection, we grab the selected
// HTML fragment and convert it back to markdown with turndown.
//
// Known limitation: KaTeX math (rehype-katex) and syntax-highlighted code blocks render as
// complex nested spans, so the markdown reconstructed from those regions may be imperfect.
// The rich-text (text/html) format and the plain-text fallback still paste sensibly.
const turndownService = new TurndownService({
  headingStyle: 'atx',
  codeBlockStyle: 'fenced',
  bulletListMarker: '-'
})
turndownService.use(gfm)

/** Serialize the contents of a Range to an HTML string. */
export function getSelectionHtml(range: Range): string {
  const container = document.createElement('div')
  container.appendChild(range.cloneContents())
  return container.innerHTML
}

/** Convert selected HTML to markdown. */
function selectionToMarkdown(range: Range): string {
  return turndownService.turndown(getSelectionHtml(range))
}

/** The current, non-collapsed selection range, or null. */
function getActiveRange(): Range | null {
  const selection = window.getSelection()
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null
  const text = selection.toString().trim()
  if (!text) return null
  return selection.getRangeAt(0)
}

/** Copy the current selection as markdown (plain text). Returns success. */
export async function copySelectionAsMarkdown(): Promise<boolean> {
  const range = getActiveRange()
  if (!range) return false
  try {
    await navigator.clipboard.writeText(selectionToMarkdown(range))
    return true
  } catch (err) {
    console.error('Failed to copy selection as markdown: ', err)
    return false
  }
}

/**
 * Copy the current selection as rich text. Writes both text/html (formatted) and
 * text/plain (markdown), mirroring the whole-message rich-text copy in Message.tsx.
 * Returns success.
 */
export async function copySelectionAsRichText(): Promise<boolean> {
  const range = getActiveRange()
  if (!range) return false
  try {
    const html = getSelectionHtml(range)
    const markdown = selectionToMarkdown(range)
    await navigator.clipboard.write([
      new ClipboardItem({
        'text/html': new Blob([html], { type: 'text/html' }),
        'text/plain': new Blob([markdown], { type: 'text/plain' })
      })
    ])
    return true
  } catch (err) {
    console.error('Failed to copy selection as rich text: ', err)
    return false
  }
}
