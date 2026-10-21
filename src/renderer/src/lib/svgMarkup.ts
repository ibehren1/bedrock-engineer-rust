/**
 * Mermaid returns its SVG serialized as HTML, so HTML labels hold void elements like `<br>`
 * with no closing slash. That is fine for `innerHTML`, but it is not well-formed XML: parsing it
 * as `image/svg+xml` stops at the first `<br>` and silently drops the rest of the drawing (the
 * nodes after it), and an `<img>` of it fails to load.
 *
 * These helpers parse leniently as HTML and hand back a real <svg> element or well-formed XML.
 */

/** The <svg> element of `markup`, parsed as HTML so nothing after a `<br>` is lost. */
export const parseSvg = (markup: string): SVGSVGElement | null =>
  new DOMParser().parseFromString(markup, 'text/html').body.querySelector('svg')

/** `markup` as well-formed SVG XML, for `data:image/svg+xml` and other XML consumers. */
export const toSvgXml = (markup: string): string => {
  const svg = parseSvg(markup)
  return svg ? new XMLSerializer().serializeToString(svg) : markup
}
