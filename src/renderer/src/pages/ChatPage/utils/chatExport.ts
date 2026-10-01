import mermaid from 'mermaid'
// mermaid はシングルトン。配色の初期化はこのモジュールの読み込み時に走るので、
// エクスポート経路が Mermaid コンポーネントを経由しなくても図の色が揃う。
import '@renderer/lib/mermaidTheme'
import { IdentifiableMessage } from '@/types/chat/message'
import { extractDrawioXml, isDrawioXml } from '@renderer/lib/drawio/xmlParser'

/**
 * A rendered image to be written alongside the exported markdown file.
 * `base64` is the raw base64 payload (without the data URL prefix).
 */
export interface ChatExportImage {
  filename: string
  base64: string
}

export interface ChatExportResult {
  markdown: string
  images: ChatExportImage[]
}

/** One rendered conversation turn: the originating message and its markdown body. */
export interface ChatSection {
  message: IdentifiableMessage
  /** Markdown body (text + rasterized image/diagram refs), tool/reasoning blocks excluded. */
  body: string
}

export interface ChatSectionsResult {
  sections: ChatSection[]
  images: ChatExportImage[]
}

/** Optional rasterizer for DrawIO XML. Returns a PNG data URL, or null on failure. */
export type DrawioRasterizer = (xml: string) => Promise<string | null>

const FENCE_RE = /```([^\n`]*)\n([\s\S]*?)```/g
const MXFILE_RE = /<mxfile[\s\S]*?<\/mxfile>/gi

/**
 * Extract the exportable conversation turns from a chat session.
 *
 * Only user prompts and assistant answers are included (text + image content
 * blocks). Tool calls, tool results, reasoning, and metadata are intentionally
 * excluded. Mermaid diagrams, DrawIO diagrams, and embedded images are
 * rasterized to PNG and referenced via relative `images/` links. Turns with no
 * renderable body are skipped.
 */
export async function buildChatSections(
  messages: IdentifiableMessage[],
  options: {
    rasterizeDrawio?: DrawioRasterizer
    /** Resize rasterized diagrams by this factor before writing them (1 = as rendered). */
    diagramScale?: number
    /**
     * Display width for diagrams, as an HTML `width` attribute value (e.g. `50%`). Set only for
     * targets that render inline HTML: it makes the reference an `<img>` tag instead of a
     * markdown image, which is the only way to control how large a diagram is shown.
     */
    diagramWidth?: string
    /**
     * Leave mermaid blocks as ```mermaid source instead of rasterizing them. Set for targets
     * that render mermaid themselves (GitHub, VS Code, Obsidian, …), where source stays
     * readable, diffable, and editable; DrawIO diagrams are still rasterized either way.
     */
    keepMermaidSource?: boolean
  } = {}
): Promise<ChatSectionsResult> {
  const images: ChatExportImage[] = []
  const diagramScale = options.diagramScale ?? 1
  let diagramCount = 0
  let imageCount = 0

  const pushImage = async (dataUrl: string, prefix: 'diagram' | 'image'): Promise<string> => {
    const isDiagram = prefix === 'diagram'
    const scale = isDiagram ? diagramScale : 1
    const resized = scale === 1 ? dataUrl : (await scalePngDataUrl(dataUrl, scale)) ?? dataUrl
    const base64 = stripDataUrlPrefix(resized)
    if (!base64) return ''
    const filename = isDiagram ? `diagram-${++diagramCount}.png` : `image-${++imageCount}.png`
    images.push({ filename, base64 })
    return isDiagram && options.diagramWidth
      ? `<img src="images/${filename}" alt="diagram" width="${options.diagramWidth}" />`
      : `![${prefix}](images/${filename})`
  }

  const sections: ChatSection[] = []

  for (const message of messages) {
    const blocks = message.content ?? []

    const rendered: string[] = []
    for (const block of blocks) {
      if (block && 'text' in block && block.text) {
        rendered.push(
          await processText(block.text, pushImage, {
            rasterizeDrawio: options.rasterizeDrawio,
            keepMermaidSource: options.keepMermaidSource
          })
        )
      } else if (block && 'image' in block && block.image) {
        const dataUrl = await imageBlockToPngDataUrl(block.image)
        if (dataUrl) {
          rendered.push(await pushImage(dataUrl, 'image'))
        }
      }
      // All other block types (toolUse, toolResult, reasoningContent, ...) are skipped.
    }

    const body = rendered.join('\n\n').trim()
    if (!body) continue

    sections.push({ message, body })
  }

  return { sections, images }
}

/**
 * Diagrams are shown at half the width of the text column. Markdown has no image-sizing syntax,
 * so this is applied through an inline `<img width="50%">` tag; renderers that fit wide images
 * to the column ignore the PNG's own pixel size, which is why halving the raster alone is not
 * enough to make a diagram render smaller.
 */
const MARKDOWN_DIAGRAM_WIDTH = '50%'

/**
 * Diagram PNGs are also written at half the size they were rendered at, which keeps them sharp
 * for {@link MARKDOWN_DIAGRAM_WIDTH} while cutting the exported file size by roughly 4x.
 */
const MARKDOWN_DIAGRAM_SCALE = 0.5

/**
 * Build a markdown document from a chat session. See {@link buildChatSections} for the content
 * rules. The output is CommonMark apart from the sized `<img>` tag used for diagrams.
 *
 * Mermaid diagrams are kept as ```mermaid source: markdown readers that matter for this output
 * (GitHub, VS Code, Obsidian) render mermaid natively, and the source stays editable and
 * diffable. DrawIO diagrams have no such support, so they are still written as PNGs.
 */
export async function buildChatMarkdown(
  title: string,
  messages: IdentifiableMessage[],
  options: {
    rasterizeDrawio?: DrawioRasterizer
    /** Override the role heading text per message (e.g. `Assistant – <modelId>`). */
    roleLabel?: (message: IdentifiableMessage) => string
    /**
     * Avatar for a message as a PNG data URL; embedded as an image in the role heading.
     * Called once per participant (the user, and each distinct assistant model) rather than
     * once per turn, since all turns of a participant share one avatar file.
     */
    avatarDataUrl?: (message: IdentifiableMessage) => Promise<string | null>
  } = {}
): Promise<ChatExportResult> {
  const { sections, images } = await buildChatSections(messages, {
    rasterizeDrawio: options.rasterizeDrawio,
    diagramScale: MARKDOWN_DIAGRAM_SCALE,
    diagramWidth: MARKDOWN_DIAGRAM_WIDTH,
    keepMermaidSource: true
  })

  const lines: string[] = [`# ${title}`, '']

  // Avatars are shared per participant, not per turn: the user's avatar and each distinct
  // assistant model are rasterized and written once, then referenced from every heading that
  // uses them. Values are the raw base64 payload, or null when rasterization failed.
  const avatars = new Map<string, string | null>()

  // Consecutive turns from the same participant sit under a single heading; a new heading is
  // only emitted when the speaker changes (including a switch to a different assistant model).
  let previousSpeaker: string | null = null

  for (const { message, body } of sections) {
    const role =
      options.roleLabel?.(message) ?? (message.role === 'assistant' ? 'Assistant' : 'User')
    const speaker = `${participantKey(message)}|${role}`

    if (speaker === previousSpeaker) {
      lines.push(body, '')
      continue
    }
    previousSpeaker = speaker

    let heading = role
    if (options.avatarDataUrl) {
      const filename = avatarFilename(message)
      if (!avatars.has(filename)) {
        const dataUrl = await options.avatarDataUrl(message)
        const base64 = dataUrl ? stripDataUrlPrefix(dataUrl) : ''
        avatars.set(filename, base64 || null)
        if (base64) images.push({ filename, base64 })
      }
      if (avatars.get(filename)) {
        heading = `![${message.role === 'assistant' ? 'assistant' : 'user'} avatar](images/${filename}) ${role}`
      }
    }

    lines.push(`## ${heading}`, '', body, '')
  }

  return { markdown: lines.join('\n').trim() + '\n', images }
}

/**
 * Identity of the party speaking in a message: the user, or a specific assistant model.
 * Two messages with the same key share an avatar and a heading.
 */
export function participantKey(message: IdentifiableMessage): string {
  return message.role === 'assistant' ? `assistant:${message.metadata?.modelId ?? ''}` : 'user'
}

/**
 * Stable avatar filename for a message's participant: one file for the user, and one per
 * distinct assistant model (named after the model id so multiple models don't collide).
 */
function avatarFilename(message: IdentifiableMessage): string {
  if (message.role !== 'assistant') return 'user-avatar.png'
  const modelId = message.metadata?.modelId
  return modelId ? `assistant-avatar-${sanitizeForFilename(modelId)}.png` : 'assistant-avatar.png'
}

/**
 * Reduce a model id to filesystem-safe characters (model ids contain `:` and inference
 * profiles may be full ARNs). Over-long values keep their tail, which is the part that
 * distinguishes one ARN or model version from another.
 */
function sanitizeForFilename(value: string): string {
  const safe = value.replace(/[^A-Za-z0-9._-]+/g, '-')
  const capped = safe.length > 80 ? safe.slice(safe.length - 80) : safe
  return capped.replace(/^[-.]+|[-.]+$/g, '') || 'model'
}

interface ProcessTextOptions {
  rasterizeDrawio?: DrawioRasterizer
  keepMermaidSource?: boolean
}

/**
 * Process a single text block: rasterize any mermaid / DrawIO fenced blocks (and
 * bare `<mxfile>` XML) into PNG image references, leaving all other markdown
 * untouched so it remains portable. With `keepMermaidSource`, mermaid fences are
 * passed through unchanged.
 */
async function processText(
  text: string,
  pushImage: (dataUrl: string, prefix: 'diagram' | 'image') => Promise<string>,
  { rasterizeDrawio, keepMermaidSource }: ProcessTextOptions = {}
): Promise<string> {
  // Pass 1: fenced code blocks
  let out = ''
  let lastIndex = 0
  let match: RegExpExecArray | null
  FENCE_RE.lastIndex = 0
  while ((match = FENCE_RE.exec(text)) !== null) {
    out += text.slice(lastIndex, match.index)
    const lang = match[1].trim().toLowerCase()
    const code = match[2]
    const original = match[0]

    if (lang === 'mermaid') {
      if (keepMermaidSource) {
        out += original
      } else {
        const dataUrl = await rasterizeMermaid(code)
        out += dataUrl ? await pushImage(dataUrl, 'diagram') : original
      }
    } else if (lang === 'xml' || lang === 'drawio' || (lang === '' && isDrawioXml(code))) {
      const xml = extractDrawioXml(code)
      const dataUrl = xml && rasterizeDrawio ? await rasterizeDrawio(xml) : null
      out += dataUrl ? await pushImage(dataUrl, 'diagram') : original
    } else {
      out += original
    }

    lastIndex = match.index + original.length
  }
  out += text.slice(lastIndex)

  // Pass 2: bare <mxfile> XML outside of code fences
  if (rasterizeDrawio && MXFILE_RE.test(out)) {
    out = await replaceAsync(out, MXFILE_RE, async (xmlMatch) => {
      const xml = extractDrawioXml(xmlMatch)
      const dataUrl = xml ? await rasterizeDrawio(xml) : null
      return dataUrl ? await pushImage(dataUrl, 'diagram') : xmlMatch
    })
  }

  return out
}

/** Render mermaid source to a high-resolution PNG data URL. */
async function rasterizeMermaid(code: string): Promise<string | null> {
  try {
    const { svg } = await mermaid.render(`export-${crypto.randomUUID()}`, code)
    return await svgToPngDataUrl(svg)
  } catch (error) {
    console.error('Failed to rasterize mermaid diagram for export:', error)
    return null
  }
}

/**
 * Re-encode a PNG data URL at a fraction of its pixel size. Returns null on failure, so callers
 * can fall back to the unscaled image.
 */
function scalePngDataUrl(dataUrl: string, factor: number): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => {
      const width = Math.max(1, Math.round((img.naturalWidth || img.width) * factor))
      const height = Math.max(1, Math.round((img.naturalHeight || img.height) * factor))
      const canvas = document.createElement('canvas')
      canvas.width = width
      canvas.height = height
      const ctx = canvas.getContext('2d')
      if (!ctx) {
        resolve(null)
        return
      }
      ctx.fillStyle = 'white'
      ctx.fillRect(0, 0, width, height)
      ctx.imageSmoothingEnabled = true
      ctx.imageSmoothingQuality = 'high'
      ctx.drawImage(img, 0, 0, width, height)
      resolve(canvas.toDataURL('image/png'))
    }
    img.onerror = () => resolve(null)
    img.src = dataUrl
  })
}

/** Convert an SVG string to a white-background PNG data URL (3x resolution). */
export function svgToPngDataUrl(svg: string, scale = 3): Promise<string | null> {
  return new Promise((resolve) => {
    let width = 800
    let height = 600

    try {
      const svgElement = new DOMParser().parseFromString(svg, 'image/svg+xml').querySelector('svg')
      if (svgElement) {
        const svgWidth = svgElement.getAttribute('width')
        const svgHeight = svgElement.getAttribute('height')
        if (svgWidth && svgHeight) {
          width = parseFloat(svgWidth.replace('px', '')) || width
          height = parseFloat(svgHeight.replace('px', '')) || height
        } else {
          const viewBox = svgElement.getAttribute('viewBox')
          if (viewBox) {
            const [, , vbWidth, vbHeight] = viewBox.split(/\s+/).map(Number)
            width = vbWidth || width
            height = vbHeight || height
          }
        }
      }
    } catch {
      // Fall back to default dimensions
    }

    const canvas = document.createElement('canvas')
    const ctx = canvas.getContext('2d')
    const img = new Image()

    img.onload = () => {
      canvas.width = width * scale
      canvas.height = height * scale
      if (ctx) {
        ctx.scale(scale, scale)
        ctx.fillStyle = 'white'
        ctx.fillRect(0, 0, width, height)
        ctx.imageSmoothingEnabled = true
        ctx.imageSmoothingQuality = 'high'
        ctx.drawImage(img, 0, 0, width, height)
      }
      resolve(canvas.toDataURL('image/png'))
    }
    img.onerror = () => resolve(null)
    img.src = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(svg)))}`
  })
}

/** Re-encode an embedded message image (any format) to a PNG data URL. */
function imageBlockToPngDataUrl(image: {
  format?: string
  source?: { bytes?: unknown }
}): Promise<string | null> {
  const sourceUrl = imageDataToDataUrl(image.source?.bytes, image.format || 'png')
  if (!sourceUrl) return Promise.resolve(null)

  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => {
      const canvas = document.createElement('canvas')
      canvas.width = img.naturalWidth || img.width
      canvas.height = img.naturalHeight || img.height
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.drawImage(img, 0, 0)
        resolve(canvas.toDataURL('image/png'))
      } else {
        resolve(sourceUrl)
      }
    }
    // If re-encoding fails, fall back to the original payload.
    img.onerror = () => resolve(sourceUrl)
    img.src = sourceUrl
  })
}

/** Convert various Bedrock image byte representations to a data URL. */
function imageDataToDataUrl(imageData: unknown, format: string): string | null {
  if (!imageData) return null

  if (typeof imageData === 'string') {
    return imageData.startsWith('data:') ? imageData : `data:image/${format};base64,${imageData}`
  }

  if (imageData instanceof Uint8Array) {
    let binary = ''
    for (let i = 0; i < imageData.length; i++) {
      binary += String.fromCharCode(imageData[i])
    }
    return `data:image/${format};base64,${btoa(binary)}`
  }

  if (typeof imageData === 'object' && imageData !== null && 'bytes' in imageData) {
    return imageDataToDataUrl((imageData as { bytes: unknown }).bytes, format)
  }

  return null
}

/** Strip a `data:...;base64,` prefix, returning the raw base64 payload. */
function stripDataUrlPrefix(dataUrl: string): string {
  const commaIndex = dataUrl.indexOf(',')
  return commaIndex >= 0 ? dataUrl.slice(commaIndex + 1) : dataUrl
}

/** Like String.prototype.replace, but supports an async replacer. */
async function replaceAsync(
  input: string,
  regex: RegExp,
  replacer: (match: string) => Promise<string>
): Promise<string> {
  const matches = [...input.matchAll(regex)]
  let result = ''
  let lastIndex = 0
  for (const match of matches) {
    const start = match.index ?? 0
    result += input.slice(lastIndex, start)
    result += await replacer(match[0])
    lastIndex = start + match[0].length
  }
  result += input.slice(lastIndex)
  return result
}
