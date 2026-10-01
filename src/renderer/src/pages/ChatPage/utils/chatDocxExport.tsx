import { renderToStaticMarkup } from 'react-dom/server'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { IdentifiableMessage } from '@/types/chat/message'
import { buildChatSections, type ChatExportImage, type DrawioRasterizer } from './chatExport'
import {
  escapeHtml,
  formatDiagrams,
  groupSectionsBySpeaker,
  inlineImages,
  rasterizeGroupAvatars,
  type AvatarContext
} from './chatHtmlExport'

export interface BuildChatHtmlOptions {
  rasterizeDrawio?: DrawioRasterizer
  /** Role heading text per message (e.g. `Assistant (Claude Opus 5)`). */
  roleLabel: (message: IdentifiableMessage) => string
  /** When provided, each speaker's chat avatar is embedded next to its heading. */
  avatar?: AvatarContext
}

/**
 * Horizontal rule between speaker groups. html-to-docx ignores `<hr>` and CSS borders on
 * paragraphs/divs, so a full-width single-cell table with borders disabled except the cell's
 * bottom edge renders as a clean horizontal line (no surrounding box).
 */
const MESSAGE_RULE =
  '<table style="border:none;width:100%"><tbody><tr>' +
  '<td style="border:none;border-bottom:1px solid #cccccc"></td>' +
  '</tr></tbody></table>'

/**
 * Build a self-contained HTML document for a chat session, suitable for handing to a
 * markdown/HTML→docx writer. Role headings are constructed directly (so headings inside
 * message bodies don't interfere), body images/diagrams are inlined as data URLs, and each
 * speaker's chat avatar is embedded beside its heading.
 */
export async function buildChatHtml(
  title: string,
  messages: IdentifiableMessage[],
  options: BuildChatHtmlOptions
): Promise<{ html: string }> {
  const { sections, images } = await buildChatSections(messages, {
    rasterizeDrawio: options.rasterizeDrawio
  })

  const imageMap = new Map(images.map((img: ChatExportImage) => [img.filename, img.base64]))
  const groups = groupSectionsBySpeaker(sections, options.roleLabel)
  const avatarUrls = await rasterizeGroupAvatars(groups, options.avatar)

  const parts: string[] = [`<h1>${escapeHtml(title)}</h1>`]

  groups.forEach(({ label, bodies }, index) => {
    const avatar = avatarUrls[index]
    // The <img> must be a bare direct child of the heading (src only, no width/height/style,
    // not wrapped in a span): html-to-docx otherwise emits an invalid empty extent for an
    // image inside a heading. The size is baked into the PNG (DEFAULT_AVATAR_PX) instead.
    const avatarImg = avatar ? `<img src="${avatar}" />&nbsp;` : ''

    // A horizontal rule separates consecutive speakers.
    if (index > 0) parts.push(MESSAGE_RULE)
    // No inline font-size: the role heading takes its size from the document's Heading 2 style.
    parts.push(`<h2>${avatarImg}${escapeHtml(label)}</h2>`)

    for (const body of bodies) {
      const bodyHtml = renderToStaticMarkup(<Markdown remarkPlugins={[remarkGfm]}>{body}</Markdown>)
      parts.push(inlineImages(formatDiagrams(formatCodeBlocks(bodyHtml)), imageMap))
    }
  })

  return { html: parts.join('\n') }
}

/** Fixed-pitch styling for exported code blocks. */
const CODE_BLOCK_STYLE = 'font-family:Courier New;font-size:8pt'

/** A fenced code block as react-markdown renders it: `<pre><code class="language-x">…`. */
const CODE_BLOCK_RE = /<pre(?:\s[^>]*)?>(?:<code(?:\s[^>]*)?>)?([\s\S]*?)(?:<\/code>)?<\/pre>/g

/** Tab width used when expanding tabs in code blocks (a raw tab in `w:t` is not honored). */
const CODE_TAB_WIDTH = 4

/**
 * Rewrite code blocks into a single 8pt Courier New paragraph with one explicit `<br />` per
 * source line. html-to-docx otherwise renders a `<pre>` in its own hardcoded "Courier" font at
 * body size and folds every newline into a space, losing the code's layout entirely.
 * Indentation needs no escaping: html-to-docx always emits `w:t` with `xml:space="preserve"`.
 * The block's text is left as-is (already HTML-escaped by the markdown renderer).
 */
function formatCodeBlocks(html: string): string {
  return html.replace(CODE_BLOCK_RE, (_whole, code: string) => {
    const lines = code
      .replace(/\r\n?/g, '\n')
      .replace(/\n+$/, '') // the renderer emits a trailing newline inside <code>
      .replace(/\t/g, ' '.repeat(CODE_TAB_WIDTH))
      .split('\n')
    return `<p style="${CODE_BLOCK_STYLE}">${lines.join('<br />')}</p>`
  })
}
