import { sanitizeGeneratedTitle, MAX_TITLE_LENGTH } from './sanitizeTitle'

describe('sanitizeGeneratedTitle', () => {
  it('leaves a good title untouched', () => {
    expect(sanitizeGeneratedTitle('Monthly sales summary')).toBe('Monthly sales summary')
  })

  it.each([
    ['**Bold title**', 'Bold title'],
    ['*Italic title*', 'Italic title'],
    ['***Both***', 'Both'],
    ['__Underline bold__', 'Underline bold'],
    ['_Underscore italic_', 'Underscore italic'],
    ['~~Struck~~ through', 'Struck through'],
    ['`readFiles` refactor', 'readFiles refactor'],
    ['## Heading title', 'Heading title'],
    ['> Quoted title', 'Quoted title'],
    ['- Bulleted title', 'Bulleted title'],
    ['1. Numbered title', 'Numbered title'],
    ['[Linked title](https://example.com)', 'Linked title'],
    ['![Alt text](img.png)', 'Alt text'],
    ['<b>Tagged title</b>', 'Tagged title']
  ])('strips markdown from %j', (input, expected) => {
    expect(sanitizeGeneratedTitle(input)).toBe(expected)
  })

  it('keeps underscores inside identifiers', () => {
    expect(sanitizeGeneratedTitle('Fix my_var handling')).toBe('Fix my_var handling')
  })

  it('drops a fenced code block entirely', () => {
    expect(sanitizeGeneratedTitle('```\nconst a = 1\n```\nCode review')).toBe('Code review')
  })

  it('takes only the first non-empty line', () => {
    expect(sanitizeGeneratedTitle('\n\nCSV summariser\nSome explanation here')).toBe(
      'CSV summariser'
    )
  })

  it.each([
    ['"Quoted title"', 'Quoted title'],
    ["'Single quoted'", 'Single quoted'],
    ['“Curly quoted”', 'Curly quoted'],
    ['`Backticked`', 'Backticked']
  ])('removes wrapping quotes from %j', (input, expected) => {
    expect(sanitizeGeneratedTitle(input)).toBe(expected)
  })

  it.each([
    ['Title: Deploy pipeline', 'Deploy pipeline'],
    ['Chat title - Deploy pipeline', 'Deploy pipeline'],
    ['Summary: Deploy pipeline', 'Deploy pipeline']
  ])('removes a label prefix from %j', (input, expected) => {
    expect(sanitizeGeneratedTitle(input)).toBe(expected)
  })

  it('strips trailing sentence punctuation but keeps a question mark', () => {
    expect(sanitizeGeneratedTitle('Fixing the build.')).toBe('Fixing the build')
    expect(sanitizeGeneratedTitle('Why does the build fail?')).toBe('Why does the build fail?')
  })

  it('collapses whitespace and newlines', () => {
    expect(sanitizeGeneratedTitle('  Too   many\tspaces  ')).toBe('Too many spaces')
  })

  it('truncates on a word boundary without an ellipsis', () => {
    const long =
      'Investigating why the deployment pipeline keeps failing on the staging environment'
    const out = sanitizeGeneratedTitle(long)!
    expect(out.length).toBeLessThanOrEqual(MAX_TITLE_LENGTH)
    expect(out).not.toMatch(/\s$/)
    // cut at a space, so no partial word
    expect(long.startsWith(out)).toBe(true)
    expect(out).toBe('Investigating why the deployment pipeline keeps failing on')
  })

  it('truncates CJK text, which has no spaces to break on', () => {
    const cjk = '会議'.repeat(40)
    const out = sanitizeGeneratedTitle(cjk)!
    expect(out.length).toBe(MAX_TITLE_LENGTH)
  })

  it.each([null, undefined, '', '   ', '\n\n', '**  **', '```\n```'])(
    'returns null for %j so the caller can fall back',
    (input) => {
      expect(sanitizeGeneratedTitle(input as string | null | undefined)).toBeNull()
    }
  )

  it('handles a realistic messy response', () => {
    const messy = 'Title: **`sales.csv` monthly totals**\n\nThis title summarises the request.'
    expect(sanitizeGeneratedTitle(messy)).toBe('sales.csv monthly totals')
  })
})
