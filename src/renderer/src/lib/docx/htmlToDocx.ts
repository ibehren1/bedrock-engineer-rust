/**
 * Chat → Word conversion for the Tauri build. The Electron app ran html-to-docx in the main
 * process (`save-chat-to-docx` in `src/main/handlers/file-handlers.ts`); Tauri has no Node
 * process, so the same library, options and post-processing run here in the renderer and only
 * the finished bytes go to Rust to be written (`save_chat_to_docx`). The output is identical.
 *
 * html-to-docx is a Node bundle: its Node built-in imports resolve to `./nodeShims` (Vite
 * aliases in `vite.tauri.config.ts`), and it reads the free globals `global` and `Buffer`, which
 * are installed only for the duration of a conversion (see {@link withNodeGlobals}). Load this
 * module with a dynamic `import()` so the library stays out of the startup bundle.
 */
import { Buffer } from 'buffer'
import { postProcessDocx } from './postProcessDocx'

/** 1 inch in TWIP (1/1440 inch), the unit html-to-docx takes for page margins. */
const INCH_IN_TWIP = 1440

/**
 * Page margins for the exported docx: 1 inch on all four sides. Every key must be supplied —
 * html-to-docx only fills in defaults for the keys present in this object, so omitting
 * `header`/`footer`/`gutter` would emit an incomplete `<w:pgMar>`. The header/footer offsets
 * keep the library's own defaults (0.5 inch); no header or footer is generated anyway.
 */
const DOCX_MARGINS = {
  top: INCH_IN_TWIP,
  right: INCH_IN_TWIP,
  bottom: INCH_IN_TWIP,
  left: INCH_IN_TWIP,
  header: 720,
  footer: 720,
  gutter: 0
}

/** Body font of the exported docx. html-to-docx would otherwise default to Times New Roman. */
const DOCX_FONT = 'Calibri'

/** Body (Normal) font size in HIP — half-points, so 20 = 10pt. */
const DOCX_BODY_FONT_SIZE_HIP = 20

/** An `<img>` tag with a double-quoted `src`. */
const IMG_TAG_RE = /<img\b[^>]*?\ssrc="([^"]*)"[^>]*>/gi

/** A base64 data URL; group 1 is the payload. */
const BASE64_DATA_URL_RE = /^data:[^,;]*(?:;[^,;]*)*;base64,(.*)$/is

/**
 * The decoded payload starts with a PNG, JPEG, GIF, WebP or BMP signature. These are the
 * formats the image-size copy bundled in html-to-docx recognizes on its first-byte fast path,
 * so it never reaches its ICNS or HEIF parsers, which can loop forever on malformed input
 * (image-size ≤ 2.0.2 advisories; the copy is inlined in html-to-docx's bundle, so it can't
 * be upgraded).
 */
function isSafeImagePayload(base64: string): boolean {
  let head: string
  try {
    // 24 base64 chars decode to the first 18 bytes, enough for every signature below.
    head = atob(decodeURIComponent(base64).replace(/\s+/g, '').slice(0, 24))
  } catch {
    return false
  }
  return (
    head.startsWith('\x89PNG\r\n\x1a\n') ||
    head.startsWith('\xff\xd8\xff') ||
    head.startsWith('GIF87a') ||
    head.startsWith('GIF89a') ||
    (head.startsWith('RIFF') && head.startsWith('WEBPVP8', 8)) ||
    head.startsWith('BM')
  )
}

/**
 * Remove every `<img>` html-to-docx would have to size that isn't an inline PNG, JPEG, GIF,
 * WebP or BMP (see {@link isSafeImagePayload}). The chat export only inlines images as data
 * URLs, so in practice this drops attachments in other formats rather than hanging the export.
 */
export function dropUnsafeImages(html: string): string {
  return html.replace(IMG_TAG_RE, (tag, src: string) => {
    const payload = BASE64_DATA_URL_RE.exec(src.trim())?.[1]
    return payload !== undefined && isSafeImagePayload(payload) ? tag : ''
  })
}

/**
 * Run `fn` with `globalThis.global` and `globalThis.Buffer` defined, as html-to-docx expects,
 * then remove whichever of them were missing before, so the rest of the renderer never sees a
 * Node-like environment (some libraries switch code paths when `Buffer` exists).
 */
async function withNodeGlobals<T>(fn: () => Promise<T>): Promise<T> {
  const g = globalThis as any
  const added: string[] = []
  if (!('global' in g)) {
    g.global = globalThis
    added.push('global')
  }
  if (!('Buffer' in g)) {
    g.Buffer = Buffer
    added.push('Buffer')
  }
  try {
    return await fn()
  } finally {
    for (const key of added) delete g[key]
  }
}

/**
 * Convert the export HTML body (from `buildChatHtml`) to docx bytes: the same html-to-docx
 * options as the Electron main process, then {@link postProcessDocx}. Post-processing is
 * best-effort — if it fails, the untouched (still valid) document is returned.
 */
export async function convertChatHtmlToDocx(html: string): Promise<Uint8Array> {
  return withNodeGlobals(async () => {
    // Loaded on first use: html-to-docx is large and only the Word export needs it.
    // eslint-disable-next-line no-restricted-syntax
    const { default: HTMLtoDOCX } = await import('html-to-docx')

    // 自己完結した HTML（インライン画像付き）を docx に変換する
    const document = `<!DOCTYPE html><html><head><meta charset="utf-8" /></head><body>${dropUnsafeImages(html)}</body></html>`
    const generated = await HTMLtoDOCX(document, null, {
      font: DOCX_FONT,
      fontSize: DOCX_BODY_FONT_SIZE_HIP,
      complexScriptFontSize: DOCX_BODY_FONT_SIZE_HIP,
      margins: DOCX_MARGINS,
      table: { row: { cantSplit: true } },
      footer: false,
      pageNumber: false
    })
    const bytes = await toUint8Array(generated)

    // Apply the tweaks html-to-docx offers no options for: tighter message-separator rules
    // and the heading sizes. Best-effort — if anything fails, fall back to the untouched,
    // still-valid document.
    try {
      return await postProcessDocx(bytes)
    } catch (error) {
      console.warn('Failed to post-process docx; using untouched document', error)
      return bytes
    }
  })
}

async function toUint8Array(data: ArrayBuffer | Uint8Array | Blob): Promise<Uint8Array> {
  if (data instanceof Uint8Array)
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
  if (data instanceof ArrayBuffer) return new Uint8Array(data)
  return new Uint8Array(await data.arrayBuffer())
}

/** Base64 of `bytes`, for sending the document to Rust as one JSON string. */
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk))
  }
  return btoa(binary)
}
