/**
 * Fitting Mermaid flowcharts to the chat column.
 *
 * Mermaid has no maximum width: a flowchart's shape comes from its direction. A long chain
 * written `flowchart LR` renders as a strip several times wider than it is tall, which then
 * either shrinks to unreadable text or scrolls sideways. When a flowchart comes out that
 * lopsided we also render it in the other direction and keep whichever shape is closer to the
 * column's, so the diagram can be read without scrolling.
 *
 * Only the diagram's top-level direction changes. Nodes, edges, labels and styling are as
 * written, and `direction` lines inside subgraphs are left alone.
 */

/** The shape we fit to: a little wider than tall, like the chat column. */
export const TARGET_ASPECT = 1.2

/** Re-orient only past these width:height ratios; anything between is left as written. */
export const MAX_ASPECT = 2.5
export const MIN_ASPECT = 0.4

const HEADER = /^(\s*(?:%%.*\n\s*)*(?:flowchart|graph)[ \t]+)(LR|RL|TB|TD|BT)\b/

/**
 * `code` with its top-level flowchart direction turned 90°: left-right becomes top-bottom and
 * the other way round. Null when it is not a flowchart, or names no direction.
 */
export function flipDirection(code: string): string | null {
  const match = HEADER.exec(code)
  if (!match) return null
  const horizontal = match[2] === 'LR' || match[2] === 'RL'
  return code.replace(HEADER, `$1${horizontal ? 'TB' : 'LR'}`)
}

/** Width:height of an SVG from its viewBox (or width/height). Null when it has neither. */
export function svgAspect(svg: SVGSVGElement): number | null {
  const viewBox = svg
    .getAttribute('viewBox')
    ?.trim()
    .split(/[\s,]+/)
    .map(Number)
  let width = viewBox?.[2]
  let height = viewBox?.[3]
  if (!width || !height) {
    width = parseFloat(svg.getAttribute('width') ?? '')
    height = parseFloat(svg.getAttribute('height') ?? '')
  }
  return width > 0 && height > 0 ? width / height : null
}

/** The rendered shape is lopsided enough to be worth trying the other direction. */
export function needsReorient(aspect: number | null): boolean {
  return aspect !== null && (aspect > MAX_ASPECT || aspect < MIN_ASPECT)
}

/** Whether the flipped shape is closer to the target than the original (in ratio terms). */
export function prefersFlipped(original: number, flipped: number, target = TARGET_ASPECT): boolean {
  const distance = (aspect: number) => Math.abs(Math.log(aspect / target))
  return distance(flipped) < distance(original)
}
