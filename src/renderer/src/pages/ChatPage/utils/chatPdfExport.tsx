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

export interface BuildChatPdfOptions {
  rasterizeDrawio?: DrawioRasterizer
  /** Role heading text per message (e.g. `Assistant – <modelId>`). */
  roleLabel: (message: IdentifiableMessage) => string
  /** When provided, each speaker's chat avatar is embedded next to its heading. */
  avatar?: AvatarContext
}

/**
 * Avatars are rasterized larger than they are shown (they render at 1em) so they stay sharp at
 * print resolution; the PDF sizes them with CSS rather than from the PNG's own dimensions.
 */
const PDF_AVATAR_PX = 48

/**
 * Print stylesheet for the PDF export. The typography deliberately matches the Word export —
 * Calibri at 10pt, 18/16/14pt headings, 8pt Courier New code, compact tables, half-width
 * centered diagrams — so the two documents read the same. Page size and margins are set by the
 * main process when printing, not here.
 */
const PDF_STYLES = `
  body {
    font-family: Calibri, Carlito, "Helvetica Neue", Arial, sans-serif;
    font-size: 10pt;
    line-height: 1.3;
    color: #000;
    margin: 0;
  }
  h1, h2, h3, h4, h5, h6 { font-weight: bold; page-break-after: avoid; break-after: avoid; }
  h1 { font-size: 18pt; margin: 12pt 0 6pt; }
  h2 { font-size: 16pt; margin: 9pt 0 4pt; }
  h3 { font-size: 14pt; margin: 7pt 0 4pt; }
  h4 { font-size: 12pt; margin: 6pt 0 2pt; }
  h5, h6 { font-size: 10pt; margin: 5pt 0 2pt; }
  p { margin: 0 0 6pt; }
  ul, ol { margin: 0 0 6pt; padding-left: 24pt; }
  li { margin: 0; }
  blockquote {
    margin: 0 0 6pt;
    padding-left: 8pt;
    border-left: 2pt solid #cccccc;
    color: #333;
  }
  pre, code, kbd, samp { font-family: "Courier New", Courier, monospace; font-size: 8pt; }
  pre {
    margin: 0 0 6pt;
    white-space: pre-wrap;
    word-break: break-word;
    page-break-inside: avoid;
    break-inside: avoid;
  }
  pre code { font-size: inherit; }
  table {
    border-collapse: collapse;
    width: 100%;
    margin: 0 0 6pt;
    page-break-inside: avoid;
    break-inside: avoid;
  }
  th, td { border: 1px solid #000; padding: 1pt 4pt; vertical-align: top; text-align: left; }
  th { font-weight: bold; }
  img { max-width: 100%; page-break-inside: avoid; break-inside: avoid; }
  img.avatar {
    height: 1em;
    width: auto;
    max-width: none;
    vertical-align: -0.12em;
    margin-right: 0.35em;
  }
  hr { border: none; border-top: 1px solid #cccccc; margin: 12pt 0; }
  a { color: #0000ee; }
`

/**
 * Build a self-contained HTML document for a chat session, ready to be printed to PDF by the
 * main process. The content follows the same rules as the markdown export (see
 * `buildChatSections`): user prompts and assistant answers only, one heading per speaker, images
 * and diagrams rasterized — here inlined as data URLs so the document needs no side files.
 */
export async function buildChatPdfHtml(
  title: string,
  messages: IdentifiableMessage[],
  options: BuildChatPdfOptions
): Promise<{ html: string }> {
  const { sections, images } = await buildChatSections(messages, {
    rasterizeDrawio: options.rasterizeDrawio
  })

  const imageMap = new Map(images.map((img: ChatExportImage) => [img.filename, img.base64]))
  const groups = groupSectionsBySpeaker(sections, options.roleLabel)
  const avatarUrls = await rasterizeGroupAvatars(groups, options.avatar, PDF_AVATAR_PX)

  const parts: string[] = [`<h1>${escapeHtml(title)}</h1>`]

  groups.forEach(({ label, bodies }, index) => {
    const avatar = avatarUrls[index]
    const avatarImg = avatar ? `<img class="avatar" src="${avatar}" alt="" />` : ''

    // A horizontal rule separates consecutive speakers.
    if (index > 0) parts.push('<hr />')
    parts.push(`<h2>${avatarImg}${escapeHtml(label)}</h2>`)

    for (const body of bodies) {
      const bodyHtml = renderToStaticMarkup(<Markdown remarkPlugins={[remarkGfm]}>{body}</Markdown>)
      parts.push(inlineImages(formatDiagrams(bodyHtml), imageMap))
    }
  })

  const html = [
    '<!DOCTYPE html>',
    '<html>',
    '<head>',
    '<meta charset="utf-8" />',
    `<title>${escapeHtml(title)}</title>`,
    `<style>${PDF_STYLES}</style>`,
    '</head>',
    '<body>',
    parts.join('\n'),
    '</body>',
    '</html>'
  ].join('\n')

  return { html }
}
