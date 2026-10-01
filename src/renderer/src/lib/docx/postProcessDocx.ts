/**
 * Rewrites of a generated docx that html-to-docx exposes no options for. Moved unchanged from
 * the Electron main process (`src/main/handlers/file-handlers.ts`), which ran them after
 * `HTMLtoDOCX`; the Tauri build converts in the renderer (see `htmlToDocx.ts`).
 */
import JSZip from 'jszip'

/**
 * Heading sizes in HIP (half-points) for the exported docx: 18pt / 16pt / 14pt. html-to-docx
 * hardcodes 24/18/14pt into its `styles.xml` template with no option to override, so these are
 * patched in afterwards. H4-H6 keep the library's defaults (12pt, 10pt, 10pt), which already
 * sit below H3.
 */
const DOCX_HEADING_SIZES_HIP: Record<string, number> = {
  Heading1: 36,
  Heading2: 32,
  Heading3: 28
}

/**
 * Cell padding for exported tables in TWIP (1/20 pt): 1pt vertical, 4pt horizontal.
 * html-to-docx uses 4pt / 8pt, which is a lot of dead space on a one-line row.
 */
const DOCX_TABLE_CELL_MARGINS: Record<string, number> = {
  top: 20,
  bottom: 20,
  left: 80,
  right: 80
}

/** Height in TWIP (6pt) of the empty paragraph html-to-docx puts after each table. */
const DOCX_TABLE_SPACER_HEIGHT = 120

/**
 * Rewrite the parts of a generated docx that html-to-docx exposes no options for: the
 * message-separator and table spacing in `word/document.xml`, and the heading styles in
 * `word/styles.xml`. Returns the original bytes unchanged if neither part needed edits.
 */
export async function postProcessDocx(buffer: Uint8Array): Promise<Uint8Array> {
  const zip = await JSZip.loadAsync(buffer)
  let changed = false

  const parts: [string, (xml: string) => string][] = [
    ['word/document.xml', (xml) => compressTables(tightenMessageRules(xml))],
    ['word/styles.xml', applyHeadingStyles]
  ]

  for (const [entryPath, transform] of parts) {
    const entry = zip.file(entryPath)
    if (!entry) continue
    const xml = await entry.async('string')
    const out = transform(xml)
    if (out === xml) continue
    zip.file(entryPath, out)
    changed = true
  }

  return changed ? zip.generateAsync({ type: 'uint8array' }) : buffer
}

/**
 * Retune the heading styles in `styles.xml`, which html-to-docx hardcodes into its template
 * with no option to override: set the H1-H3 run sizes to {@link DOCX_HEADING_SIZES_HIP} and
 * halve every heading's space-before (H1 goes from 24pt to 12pt, and so on down to H6).
 * Heading styles are `basedOn` Normal, so the font itself comes from the document defaults
 * and needs no patching here.
 */
export function applyHeadingStyles(xml: string): string {
  return xml.replace(
    /<w:style\b[^>]*w:styleId="(Heading[1-6])"[\s\S]*?<\/w:style>/g,
    (style, styleId: string) => {
      let out = style.replace(
        /<w:spacing\b([^>]*?)w:before="(\d+)"/,
        (_whole, before: string, twip: string) =>
          `<w:spacing${before}w:before="${Math.round(Number(twip) / 2)}"`
      )

      const size = DOCX_HEADING_SIZES_HIP[styleId]
      if (size) {
        out = out
          .replace(/<w:sz w:val="\d+"\s*\/>/, `<w:sz w:val="${size}" />`)
          .replace(/<w:szCs w:val="\d+"\s*\/>/, `<w:szCs w:val="${size}" />`)
      }

      return out
    }
  )
}

/**
 * Compress the vertical space in content tables. html-to-docx pads every cell with 4pt top and
 * bottom, lets the cell paragraph inherit the document default's 6pt space-after, and follows
 * each table with a full-height empty paragraph — roughly doubling the height of a one-line row.
 * This trims the cell padding to {@link DOCX_TABLE_CELL_MARGINS}, zeroes the space around cell
 * paragraphs, and shrinks the trailing paragraph to {@link DOCX_TABLE_SPACER_HEIGHT}. That
 * paragraph is kept rather than dropped: without it two adjacent tables merge into one.
 * Message-separator rules (identified by their CCCCCC border) belong to
 * {@link tightenMessageRules} and are left alone here.
 */
export function compressTables(xml: string): string {
  const out = xml.replace(/<w:tbl>[\s\S]*?<\/w:tbl>/g, (table) => {
    if (table.includes('w:color="CCCCCC"')) return table

    return (
      table
        .replace(/<w:tblCellMar>[\s\S]*?<\/w:tblCellMar>/, (margins) =>
          margins.replace(
            /<w:(top|bottom|left|right) w:type="dxa" w:w="\d+"\s*\/>/g,
            (whole, edge: string) =>
              edge in DOCX_TABLE_CELL_MARGINS
                ? `<w:${edge} w:type="dxa" w:w="${DOCX_TABLE_CELL_MARGINS[edge]}"/>`
                : whole
          )
        )
        // Cell paragraphs set only the line rule, so they inherit the 6pt document space-after.
        .replace(
          /<w:spacing w:lineRule="auto"\s*\/>/g,
          '<w:spacing w:before="0" w:after="0" w:lineRule="auto"/>'
        )
    )
  })

  return out.replace(
    /(<\/w:tbl>\s*)<w:p>\s*<w:pPr>\s*<w:spacing w:lineRule="auto"\s*\/>\s*<\/w:pPr>\s*<w:r>\s*<w:rPr\s*\/>\s*<\/w:r>\s*<\/w:p>/g,
    `$1<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="${DOCX_TABLE_SPACER_HEIGHT}" w:lineRule="exact"/></w:pPr></w:p>`
  )
}

/**
 * Remove the blank space html-to-docx puts around the message-separator rules. The rule is a
 * single-cell table whose only visible edge is a light-gray (CCCCCC) bottom border; the
 * library gives it fixed 80-dxa cell margins, a full-height empty cell paragraph, and an
 * auto-inserted empty paragraph after the table. This zeroes the rule's cell margins,
 * collapses its cell paragraph, and drops the trailing empty paragraph. Only rule tables
 * (identified by the CCCCCC border) are touched; body/markdown tables are left intact.
 */
export function tightenMessageRules(xml: string): string {
  let out = xml

  // Drop the empty paragraph inserted immediately after each rule table.
  out = out.replace(
    /(<w:tbl>[\s\S]*?<\/w:tbl>)(\s*<w:p>\s*<w:pPr>\s*<w:spacing w:lineRule="auto"\s*\/>\s*<\/w:pPr>\s*<w:r>\s*<w:rPr\s*\/>\s*<\/w:r>\s*<\/w:p>)/g,
    (whole, table) => (table.includes('w:color="CCCCCC"') ? table : whole)
  )

  // Zero the rule cell's top/bottom margins and collapse its (empty) cell paragraph.
  out = out.replace(/<w:tbl>[\s\S]*?<\/w:tbl>/g, (table) => {
    if (!table.includes('w:color="CCCCCC"')) return table
    return table
      .replace(/(<w:tblCellMar>[\s\S]*?<w:top w:type="dxa" w:w=")\d+("\/>)/, '$10$2')
      .replace(/(<w:tblCellMar>[\s\S]*?<w:bottom w:type="dxa" w:w=")\d+("\/>)/, '$10$2')
      .replace(
        /<w:p\/>/,
        '<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="1" w:lineRule="exact"/></w:pPr></w:p>'
      )
  })

  return out
}
