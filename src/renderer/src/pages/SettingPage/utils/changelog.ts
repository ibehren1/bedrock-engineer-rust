/**
 * The bundled CHANGELOG.md, split into its dated sections for the Updates tab. Each build ships
 * the changelog as it was when the build was made, so its sections are that version's history.
 */

export interface ChangelogSection {
  /** `YYYY-MM-DD`, from the `### YYYY-MM-DD` heading. */
  date: string
  /** The section's Markdown, without its heading. */
  body: string
}

const SECTION_HEADING = /^### (\d{4}-\d{2}-\d{2})\s*$/

/** The dated sections, in file order (newest first). Text before the first one is dropped. */
export const parseChangelog = (markdown: string): ChangelogSection[] => {
  const sections: ChangelogSection[] = []
  let current: { date: string; lines: string[] } | null = null
  const flush = () => {
    if (current) sections.push({ date: current.date, body: current.lines.join('\n').trim() })
  }
  for (const line of markdown.split(/\r?\n/)) {
    const match = SECTION_HEADING.exec(line)
    if (match) {
      flush()
      current = { date: match[1], lines: [] }
    } else if (current) {
      current.lines.push(line)
    }
  }
  flush()
  return sections
}

/**
 * The release date a version encodes: `2026.1022.0` is 2026-10-22 and `2026.930.1` is
 * 2026-09-30. `null` for anything else.
 */
export const versionDate = (version: string): string | null => {
  const match = /^(\d{4})\.(\d{3,4})\.\d+/.exec(version)
  if (!match) return null
  const monthDay = Number(match[2])
  const month = Math.floor(monthDay / 100)
  const day = monthDay % 100
  if (month < 1 || month > 12 || day < 1 || day > 31) return null
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${match[1]}-${pad(month)}-${pad(day)}`
}

/** The sections released by `version` (all of them when its date can't be read). */
export const sectionsForVersion = (
  sections: ChangelogSection[],
  version: string
): ChangelogSection[] => {
  const date = versionDate(version)
  return date ? sections.filter((section) => section.date <= date) : sections
}
