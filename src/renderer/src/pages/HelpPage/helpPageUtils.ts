import type { GuideHeading } from '@renderer/components/Markdown/slug'

/** Separator the knowledge base uses between heading levels (`crates/knowledge` `HEADING_SEP`). */
export const HEADING_SEP = ' › '

/**
 * The anchor of the deepest heading in a knowledge-base heading path
 * (`"…› 26. Accounting… › 26.4 A monthly budget"`). Walks the path so a heading name that
 * repeats (e.g. "Tab 1") resolves under the right parent.
 */
export function headingSlugForPath(headings: GuideHeading[], headingPath: string): string | null {
  const parts = headingPath
    .split(HEADING_SEP)
    .map((p) => p.trim())
    .filter(Boolean)
  let from = 0
  let found: GuideHeading | null = null
  for (const part of parts) {
    const i = headings.findIndex((h, idx) => idx >= from && h.text === part)
    if (i === -1) continue
    found = headings[i]
    from = i + 1
  }
  return found?.slug ?? null
}

/** A short plain-text preview of an excerpt. */
export function snippet(text: string, max = 180): string {
  const plain = text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[#*_`>|]/g, '')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim()
  return plain.length > max ? `${plain.slice(0, max - 1)}…` : plain
}
