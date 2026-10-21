import mermaid from 'mermaid'
import { flipDirection, needsReorient, prefersFlipped, svgAspect } from './mermaidLayout'
import { parseSvg } from './svgMarkup'

export type MermaidRender = {
  svg: string
  /** Rendered in the other direction than written, to fit the column (see mermaidLayout). */
  reoriented: boolean
}

let counter = 0
const nextId = () => `mermaid-${Date.now().toString(36)}-${counter++}`

/**
 * Render `code`, re-oriented to fit the chat column when it comes out lopsided and `fit` is set
 * (see mermaidLayout). Throws on a syntax error, like `mermaid.render`.
 */
export async function renderMermaid(code: string, fit = true): Promise<MermaidRender> {
  const { svg } = await mermaid.render(nextId(), code)
  if (!fit) return { svg, reoriented: false }

  const flipped = flipDirection(code)
  const element = parseSvg(svg)
  const aspect = element ? svgAspect(element) : null
  if (!flipped || aspect === null || !needsReorient(aspect)) return { svg, reoriented: false }

  try {
    const alternative = await mermaid.render(nextId(), flipped)
    const altElement = parseSvg(alternative.svg)
    const altAspect = altElement ? svgAspect(altElement) : null
    if (altAspect !== null && prefersFlipped(aspect, altAspect)) {
      return { svg: alternative.svg, reoriented: true }
    }
  } catch {
    // The flipped text should parse whenever the original did; if not, keep the original.
  }
  return { svg, reoriented: false }
}
