/** Link targets a rendered document may keep: web, mail, and in-document anchors. */
const SAFE_HREF = /^(https?:|mailto:|#)/i

/**
 * Remove every `href` under `root` that isn't a web, mail or in-document link (UI-5). Documents
 * rendered into the app's own page (Word files via docx-preview) can carry `javascript:` or
 * `data:` hyperlink targets, which would run in the app if clicked. The link text stays; it just
 * stops being a link. Returns how many were removed.
 */
export function stripUnsafeLinks(root: ParentNode): number {
  let removed = 0
  root.querySelectorAll('a[href]').forEach((a) => {
    const href = (a.getAttribute('href') ?? '').trim()
    if (!SAFE_HREF.test(href)) {
      a.removeAttribute('href')
      removed++
    }
  })
  return removed
}
