/** The parts of a hast node this plugin reads (kept local: no `hast` types dependency). */
type HastNode = {
  type: string
  tagName?: string
  value?: string
  children?: HastNode[]
}

/** Elements whose whitespace is content. */
const PRESERVE = new Set(['pre', 'code', 'textarea'])

/**
 * rehype plugin: soft line breaks in prose become plain spaces. The app's webview drew a
 * newline just before an inline element (a link, bold text) as next to nothing, so a source
 * line ending in "the" followed by "[Glossary](…)" rendered as "theGlossary". A space is what
 * a soft break means in HTML anyway; code keeps its newlines.
 */
export function rehypeSoftBreaks() {
  return (tree: HastNode) => {
    const walk = (node: HastNode) => {
      if (node.type === 'element' && node.tagName && PRESERVE.has(node.tagName)) return
      if (node.type === 'text' && node.value?.includes('\n')) {
        node.value = node.value.replace(/[ \t]*\n[ \t]*/g, ' ')
      }
      node.children?.forEach(walk)
    }
    walk(tree)
  }
}
