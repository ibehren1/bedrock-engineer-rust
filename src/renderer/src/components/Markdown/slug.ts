/**
 * GitHub-style heading anchors (what `github-slugger` produces), so `[…](#26-accounting-…)`
 * links written for GitHub also work in the app: lowercase, drop everything but letters,
 * numbers, `_`, `-` and spaces, then spaces become `-`.
 */
export function githubSlug(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc}\- ]/gu, '')
    .replace(/ /g, '-')
}

/** Slugs for headings in document order, de-duplicated like GitHub (`x`, `x-1`, `x-2`). */
export class Slugger {
  private seen = new Map<string, number>()

  slug(text: string): string {
    const base = githubSlug(text)
    let slug = base
    let n = this.seen.get(base)
    if (n !== undefined) {
      do {
        n += 1
        slug = `${base}-${n}`
      } while (this.seen.has(slug))
      this.seen.set(base, n)
    } else {
      this.seen.set(base, 0)
    }
    this.seen.set(slug, this.seen.get(slug) ?? 0)
    return slug
  }
}

export type GuideHeading = { level: number; text: string; slug: string }

/** `#`–`######` headings of a Markdown document (outside fenced code), with their slugs. */
export function markdownHeadings(markdown: string): GuideHeading[] {
  const slugger = new Slugger()
  const out: GuideHeading[] = []
  let fence: string | null = null
  for (const line of markdown.split('\n')) {
    const f = line.match(/^\s*(`{3,}|~{3,})/)
    if (f) {
      if (fence === null) fence = f[1][0]
      else if (f[1][0] === fence) fence = null
      continue
    }
    if (fence !== null) continue
    const m = line.match(/^(#{1,6})\s+(.+?)\s*#*\s*$/)
    if (!m) continue
    // Inline markup doesn't reach the anchor text.
    const text = m[2]
      .replace(/`([^`]*)`/g, '$1')
      .replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, '$1$2')
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    out.push({ level: m[1].length, text, slug: slugger.slug(text) })
  }
  return out
}

/** The parts of a hast node the plugin reads (kept local: no `hast` types dependency). */
type HastNode = {
  type: string
  tagName?: string
  value?: string
  properties?: Record<string, unknown>
  children?: HastNode[]
}

const HEADING_TAGS = new Set(['h1', 'h2', 'h3', 'h4', 'h5', 'h6'])

function hastText(node: HastNode): string {
  if (node.type === 'text') return node.value ?? ''
  return (node.children ?? []).map(hastText).join('')
}

/**
 * rehype plugin: GitHub-style `id`s on every heading, numbered in document order. It runs once
 * per parse on the tree, so ids don't depend on how often React renders (a slugger inside the
 * heading components numbered twice under StrictMode's double render).
 */
export function rehypeHeadingIds() {
  return (tree: HastNode) => {
    const slugger = new Slugger()
    const walk = (node: HastNode) => {
      if (node.type === 'element' && node.tagName && HEADING_TAGS.has(node.tagName)) {
        node.properties = { ...node.properties, id: slugger.slug(hastText(node)) }
      }
      node.children?.forEach(walk)
    }
    walk(tree)
  }
}
