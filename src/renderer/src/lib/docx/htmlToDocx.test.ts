import JSZip from 'jszip'
import { bytesToBase64, convertChatHtmlToDocx, dropUnsafeImages } from './htmlToDocx'
import { applyHeadingStyles, compressTables, tightenMessageRules } from './postProcessDocx'

/** The speaker separator `buildChatHtml` emits between groups. */
const MESSAGE_RULE =
  '<table style="border:none;width:100%"><tbody><tr>' +
  '<td style="border:none;border-bottom:1px solid #cccccc"></td>' +
  '</tr></tbody></table>'

async function part(docx: Uint8Array, name: string): Promise<string> {
  const zip = await JSZip.loadAsync(docx)
  return zip.file(name)!.async('string')
}

describe('convertChatHtmlToDocx', () => {
  it('produces a post-processed Word document', async () => {
    const docx = await convertChatHtmlToDocx(
      `<h1>Title</h1><h2>User</h2><p>Hello</p>${MESSAGE_RULE}<h2>Assistant</h2>` +
        '<table><tbody><tr><td>a</td><td>b</td></tr></tbody></table>'
    )
    // A zip (docx) container.
    expect(Array.from(docx.subarray(0, 2))).toEqual([0x50, 0x4b])

    const styles = await part(docx, 'word/styles.xml')
    expect(styles).toMatch(/w:styleId="Heading1"[\s\S]*?<w:sz w:val="36" \/>/)
    expect(styles).toMatch(/w:styleId="Heading2"[\s\S]*?<w:sz w:val="32" \/>/)

    const document = await part(docx, 'word/document.xml')
    // Body font and size from the html-to-docx options.
    expect(await part(docx, 'word/styles.xml')).toContain('Calibri')
    // The rule table's cell paragraph is collapsed; content table cells are compacted.
    expect(document).toContain('w:line="1" w:lineRule="exact"')
    expect(document).toContain('<w:top w:type="dxa" w:w="20"/>')
    expect(document).toContain('w:line="120" w:lineRule="exact"')
  })

  it('leaves no Node globals behind', async () => {
    const g = globalThis as any
    const hadGlobal = 'global' in g
    const hadBuffer = 'Buffer' in g
    await convertChatHtmlToDocx('<p>x</p>')
    expect('global' in g).toBe(hadGlobal)
    expect('Buffer' in g).toBe(hadBuffer)
  })
})

describe('docx post-processing', () => {
  it('halves heading space-before and resizes H1-H3', () => {
    const xml =
      '<w:style w:styleId="Heading1"><w:spacing w:before="480"/><w:sz w:val="48" /><w:szCs w:val="48" /></w:style>' +
      '<w:style w:styleId="Heading5"><w:spacing w:before="240"/><w:sz w:val="20" /></w:style>'
    expect(applyHeadingStyles(xml)).toBe(
      '<w:style w:styleId="Heading1"><w:spacing w:before="240"/><w:sz w:val="36" /><w:szCs w:val="36" /></w:style>' +
        '<w:style w:styleId="Heading5"><w:spacing w:before="120"/><w:sz w:val="20" /></w:style>'
    )
  })

  it('compresses content tables but not message rules', () => {
    const spacer = '<w:p><w:pPr><w:spacing w:lineRule="auto"/></w:pPr><w:r><w:rPr/></w:r></w:p>'
    const content =
      '<w:tbl><w:tblCellMar><w:top w:type="dxa" w:w="80"/><w:left w:type="dxa" w:w="160"/></w:tblCellMar>' +
      '<w:spacing w:lineRule="auto"/></w:tbl>'
    const rule =
      '<w:tbl><w:tblCellMar><w:top w:type="dxa" w:w="80"/><w:bottom w:type="dxa" w:w="80"/></w:tblCellMar>' +
      '<w:b w:color="CCCCCC"/><w:p/></w:tbl>'

    expect(compressTables(content + spacer)).toBe(
      '<w:tbl><w:tblCellMar><w:top w:type="dxa" w:w="20"/><w:left w:type="dxa" w:w="80"/></w:tblCellMar>' +
        '<w:spacing w:before="0" w:after="0" w:lineRule="auto"/></w:tbl>' +
        '<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="120" w:lineRule="exact"/></w:pPr></w:p>'
    )
    expect(compressTables(rule)).toBe(rule)

    expect(tightenMessageRules(rule + spacer)).toBe(
      '<w:tbl><w:tblCellMar><w:top w:type="dxa" w:w="0"/><w:bottom w:type="dxa" w:w="0"/></w:tblCellMar>' +
        '<w:b w:color="CCCCCC"/>' +
        '<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="1" w:lineRule="exact"/></w:pPr></w:p></w:tbl>'
    )
    expect(tightenMessageRules(content + spacer)).toBe(content + spacer)
  })
})

describe('dropUnsafeImages', () => {
  const img = (bytes: number[] | string): string => {
    const raw = typeof bytes === 'string' ? bytes : String.fromCharCode(...bytes)
    return `<img src="data:image/png;base64,${btoa(raw)}" />`
  }

  it('keeps PNG, JPEG, GIF, WebP and BMP images', () => {
    const html =
      img([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13]) +
      img([0xff, 0xd8, 0xff, 0xe0]) +
      img('GIF89a\x01\x00') +
      img('RIFF\x00\x00\x00\x00WEBPVP8 ') +
      img('BM\x00\x00')
    expect(dropUnsafeImages(html)).toBe(html)
  })

  it('drops ICNS, HEIF, other formats and non-data sources', () => {
    const html =
      '<p>a</p>' +
      img('icns\x00\x00\x00\x08') +
      img('\x00\x00\x00\x18ftypheic') +
      img('RIFFftypWEBPxxxx') +
      '<img alt="x" src="https://example.com/a.png">' +
      '<img src="data:image/png;base64,!!!" />' +
      '<p>b</p>'
    expect(dropUnsafeImages(html)).toBe('<p>a</p><p>b</p>')
  })
})

describe('bytesToBase64', () => {
  it('matches Buffer base64 across chunk boundaries', () => {
    const bytes = new Uint8Array(0x8000 * 2 + 5).map((_, i) => (i * 31) & 0xff)
    expect(bytesToBase64(bytes)).toBe(Buffer.from(bytes).toString('base64'))
  })
})
