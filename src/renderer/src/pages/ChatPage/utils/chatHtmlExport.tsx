import { renderToStaticMarkup } from 'react-dom/server'
import { LiaUserCircleSolid } from 'react-icons/lia'
import { IdentifiableMessage } from '@/types/chat/message'
import { getModelIcon, isWideModelIcon, WIDE_ICON_ASPECT } from '@renderer/components/ModelIcon'
import AILogo from '@renderer/assets/images/icons/bedrock-color.png'
import { participantKey, svgToPngDataUrl, type ChatSection } from './chatExport'

/**
 * Pieces shared by the HTML-based chat exports (Word and PDF): speaker grouping, avatar
 * rasterization, and the rewrites applied to each rendered message body. The markdown export
 * needs none of this — it emits markdown directly.
 */

/** Context needed to reproduce each message's chat avatar as an image. */
export interface AvatarContext {
  /** Emoji chosen for the user avatar (empty = default user icon). */
  userEmoji?: string
  /** Resolve a modelId to its display info, mirroring the chat Avatar. */
  resolveModel: (
    modelId?: string
  ) => { modelName?: string; isInferenceProfile?: boolean } | undefined
}

/** Consecutive turns by one participant, sharing a single heading. */
export interface SpeakerGroup {
  /** The group's first message, used for the avatar and the participant identity. */
  message: IdentifiableMessage
  /** Heading text for the group. */
  label: string
  /** Markdown bodies of the grouped turns, in order. */
  bodies: string[]
}

const HTML_ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;'
}

export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (ch) => HTML_ESCAPES[ch])
}

/**
 * Collapse consecutive turns from the same participant into one group, so a heading is emitted
 * per speaker change rather than per turn. A switch to a different assistant model starts a new
 * group, as does any change in the heading text.
 */
export function groupSectionsBySpeaker(
  sections: ChatSection[],
  roleLabel: (message: IdentifiableMessage) => string
): SpeakerGroup[] {
  const groups: SpeakerGroup[] = []

  for (const { message, body } of sections) {
    const label = roleLabel(message)
    const previous = groups[groups.length - 1]
    if (
      previous &&
      previous.label === label &&
      participantKey(previous.message) === participantKey(message)
    ) {
      previous.bodies.push(body)
    } else {
      groups.push({ message, label, bodies: [body] })
    }
  }

  return groups
}

/**
 * Rasterize the avatar of each group's participant, once per participant, and return one data
 * URL per group (null where there is no avatar context or rasterization failed).
 */
export function rasterizeGroupAvatars(
  groups: SpeakerGroup[],
  ctx: AvatarContext | undefined,
  sizePx?: number
): Promise<(string | null)[]> {
  const pending = new Map<string, Promise<string | null>>()

  return Promise.all(
    groups.map(({ message }) => {
      if (!ctx) return Promise.resolve(null)
      const key = participantKey(message)
      let avatar = pending.get(key)
      if (!avatar) {
        avatar = avatarDataUrl(message, ctx, sizePx)
        pending.set(key, avatar)
      }
      return avatar
    })
  )
}

/**
 * Pixel size avatars are baked to unless a caller asks for another. The Word export depends on
 * this being the on-page size: html-to-docx emits an empty (invalid) `<wp:extent>` for an `<img>`
 * inside a heading that carries any width/height/style attribute, so the avatar `<img>` is
 * rendered bare and Word derives its size from the image's intrinsic dimensions. html-to-docx
 * maps pixels to EMU at 96 dpi (px * 9525), so 19px ≈ 0.20".
 */
export const DEFAULT_AVATAR_PX = 19

/**
 * Produce a PNG data URL for a message's chat avatar, mirroring `MessageList/Avatar.tsx`, baked
 * to a square `sizePx` canvas. Returns null on failure (the heading then renders without an
 * avatar image).
 */
export async function avatarDataUrl(
  message: IdentifiableMessage,
  ctx: AvatarContext,
  sizePx: number = DEFAULT_AVATAR_PX
): Promise<string | null> {
  try {
    let raw: string | null
    // A wide wordmark fitted into a square canvas ends up a fraction of the height every
    // other avatar gets, so it is baked to a canvas of its own proportions instead.
    let wide = false
    if (message.role === 'assistant') {
      const modelId = message.metadata?.modelId
      if (modelId) {
        const model = ctx.resolveModel(modelId)
        wide = !model?.isInferenceProfile && isWideModelIcon(modelId)
        raw = await svgToPngDataUrl(
          normalizeSvgSize(renderToStaticMarkup(getModelIcon(modelId, model?.isInferenceProfile)))
        )
      } else {
        // No modelId → the generic Bedrock logo (a bundled PNG).
        raw = await loadImageDataUrl(AILogo)
      }
    } else if (ctx.userEmoji) {
      raw = emojiToPngDataUrl(ctx.userEmoji)
    } else {
      raw = await svgToPngDataUrl(normalizeSvgSize(renderToStaticMarkup(<LiaUserCircleSolid />)))
    }

    // Bake all avatars to a uniform height so they render consistently: a square canvas,
    // except for wide wordmarks, which keep their proportions at the same height.
    return raw ? await normalizeToPng(raw, sizePx, wide ? WIDE_ICON_ASPECT : 1) : null
  } catch (error) {
    console.error('Failed to rasterize avatar for export:', error)
    return null
  }
}

/** Rendered width of a diagram, as a fraction of the text column. */
const DIAGRAM_SCALE_STYLE = 'width:50%'

/** An `<img>` for a rasterized diagram, before {@link inlineImages} rewrites its src. */
const DIAGRAM_IMG_RE = /<img src="images\/diagram-[^"]*"[^>]*?\/?>/g

/** A paragraph whose entire content is diagram images. */
const DIAGRAM_PARAGRAPH_RE = /<p>((?:\s*<img src="images\/diagram-[^"]*"[^>]*?\/?>)+\s*)<\/p>/g

/**
 * Halve the width of diagram images and center them. Both consumers read these the same way:
 * a percentage `width` is relative to the text column (html-to-docx applies it after clamping
 * the image to the page width, keeping the aspect ratio), and `text-align` on the enclosing
 * paragraph centers the image. A diagram that shares a paragraph with text is still scaled, but
 * that paragraph keeps its normal alignment.
 */
export function formatDiagrams(html: string): string {
  return html
    .replace(DIAGRAM_PARAGRAPH_RE, '<p style="text-align:center">$1</p>')
    .replace(DIAGRAM_IMG_RE, (img) => img.replace(/\s*\/?>$/, ` style="${DIAGRAM_SCALE_STYLE}" />`))
}

/** Rewrite `src="images/<filename>"` refs to inline base64 data URLs. */
export function inlineImages(html: string, imageMap: Map<string, string>): string {
  return html.replace(/src="images\/([^"]+)"/g, (whole, rawName: string) => {
    const filename = decodeURIComponent(rawName)
    const base64 = imageMap.get(filename) ?? imageMap.get(rawName)
    return base64 ? `src="data:image/png;base64,${base64}"` : whole
  })
}

/**
 * Strip the `width`/`height` attributes from an `<svg>` opening tag so
 * {@link svgToPngDataUrl} sizes from the viewBox. react-icons emit `width="1em"` which
 * otherwise rasterizes to a 1px image.
 */
function normalizeSvgSize(svg: string): string {
  return svg.replace(/<svg\b[^>]*>/i, (tag) => tag.replace(/\s(width|height)="[^"]*"/gi, ''))
}

/**
 * Draw a source image (data URL) centered on a fixed canvas `size` tall and `size * aspect`
 * wide → PNG data URL. `aspect` is 1 (a square) for every avatar except wide wordmarks.
 */
function normalizeToPng(
  srcDataUrl: string,
  size: number,
  aspect: number = 1
): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => {
      const canvasW = Math.round(size * aspect)
      const canvas = document.createElement('canvas')
      canvas.width = canvasW
      canvas.height = size
      const context = canvas.getContext('2d')
      if (!context) {
        resolve(null)
        return
      }
      const srcW = img.naturalWidth || canvasW
      const srcH = img.naturalHeight || size
      const scale = Math.min(canvasW / srcW, size / srcH)
      const w = srcW * scale
      const h = srcH * scale
      context.drawImage(img, (canvasW - w) / 2, (size - h) / 2, w, h)
      resolve(canvas.toDataURL('image/png'))
    }
    img.onerror = () => resolve(null)
    img.src = srcDataUrl
  })
}

/** Load an image URL and return it as a PNG data URL (no resizing). */
function loadImageDataUrl(url: string): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => {
      const canvas = document.createElement('canvas')
      canvas.width = img.naturalWidth || 64
      canvas.height = img.naturalHeight || 64
      const context = canvas.getContext('2d')
      if (!context) {
        resolve(null)
        return
      }
      context.drawImage(img, 0, 0, canvas.width, canvas.height)
      resolve(canvas.toDataURL('image/png'))
    }
    img.onerror = () => resolve(null)
    img.src = url
  })
}

/** Render an emoji glyph onto a small canvas and return a PNG data URL. */
function emojiToPngDataUrl(emoji: string): string | null {
  const size = 64
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const context = canvas.getContext('2d')
  if (!context) return null
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  context.font = `${Math.round(size * 0.8)}px "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif`
  context.fillText(emoji, size / 2, size / 2 + 2)
  return canvas.toDataURL('image/png')
}
