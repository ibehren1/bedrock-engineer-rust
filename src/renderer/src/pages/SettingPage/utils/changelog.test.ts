import { parseChangelog, sectionsForVersion, versionDate } from './changelog'

const SAMPLE = `# Update summary

Intro text.

### 2026-10-22

- Newest change.

### 2026-10-21

- Older change.
  Continued.

### 2026-09-30

- Oldest change.
`

describe('parseChangelog', () => {
  it('splits dated sections and drops the preamble', () => {
    expect(parseChangelog(SAMPLE)).toEqual([
      { date: '2026-10-22', body: '- Newest change.' },
      { date: '2026-10-21', body: '- Older change.\n  Continued.' },
      { date: '2026-09-30', body: '- Oldest change.' }
    ])
  })

  it('returns nothing without dated sections', () => {
    expect(parseChangelog('# Title\n\nNo sections.')).toEqual([])
  })

  it('handles CRLF line endings', () => {
    expect(parseChangelog('### 2026-01-02\r\n\r\n- A\r\n')).toEqual([
      { date: '2026-01-02', body: '- A' }
    ])
  })
})

describe('versionDate', () => {
  it('reads three- and four-digit month-day parts', () => {
    expect(versionDate('2026.1022.0')).toBe('2026-10-22')
    expect(versionDate('2026.930.1')).toBe('2026-09-30')
    expect(versionDate('2027.101.0')).toBe('2027-01-01')
  })

  it('rejects other versions', () => {
    expect(versionDate('1.2.3')).toBeNull()
    expect(versionDate('2026.1350.0')).toBeNull()
    expect(versionDate('')).toBeNull()
  })
})

describe('sectionsForVersion', () => {
  const sections = parseChangelog(SAMPLE)

  it('keeps sections up to the version date', () => {
    expect(sectionsForVersion(sections, '2026.1021.0').map((s) => s.date)).toEqual([
      '2026-10-21',
      '2026-09-30'
    ])
  })

  it('keeps everything for an unreadable version', () => {
    expect(sectionsForVersion(sections, 'dev')).toHaveLength(3)
  })
})
