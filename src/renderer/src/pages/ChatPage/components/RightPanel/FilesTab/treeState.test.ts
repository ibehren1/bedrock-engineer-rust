import type { WorkspaceEntry } from '@/types/bridge/workspace-files'
import {
  dirsToReload,
  filterPaths,
  filterTree,
  noteUserToggle,
  rememberUserCollapsed,
  touchedAncestors,
  touchedDirsToExpand,
  userCollapsedDirs,
  withExpanded
} from './treeState'
import { formatSize, languageFor, previewKind, relativeTime } from './fileTypes'

const entry = (path: string, kind: 'file' | 'dir' = 'file'): WorkspaceEntry => ({
  name: path.slice(path.lastIndexOf('/') + 1),
  path,
  kind,
  size: 1,
  modifiedMs: 0,
  ext: kind === 'file' ? path.slice(path.lastIndexOf('.') + 1) : ''
})

describe('filterTree', () => {
  const dirs = new Map([
    ['', [entry('output', 'dir'), entry('README.md')]],
    ['output', [entry('output/market', 'dir')]],
    ['output/market', [entry('output/market/report.md'), entry('output/market/chart.png')]]
  ])

  it('returns null without a filter', () => {
    expect(filterTree(dirs, '  ')).toBeNull()
  })

  it('keeps matches and their ancestors, expanding the ancestors', () => {
    const result = filterTree(dirs, 'REPORT')!
    expect([...result.visible].sort()).toEqual([
      'output',
      'output/market',
      'output/market/report.md'
    ])
    expect([...result.expand].sort()).toEqual(['output', 'output/market'])
  })
})

describe('dirsToReload', () => {
  it('picks loaded parents and loaded directories that changed', () => {
    expect(
      dirsToReload(['a/b.md', 'c/d.md', 'x', 'deep/er/file'], ['', 'a', 'x', 'deep']).sort()
    ).toEqual(['', 'a', 'x'])
  })
})

describe('touched helpers', () => {
  it('lists ancestors of touched files', () => {
    expect([...touchedAncestors(['a/b/c.md', 'a/d.md', 'top.md'])].sort()).toEqual(['a', 'a/b'])
  })

  it('filters chat files by name or folder', () => {
    expect(filterPaths(['out/report.md', 'src/app.ts'], 'out')).toEqual(['out/report.md'])
    expect(filterPaths(['out/report.md', 'src/app.ts'], '')).toHaveLength(2)
  })
})

describe('auto-expanding touched folders', () => {
  it('opens the ancestors of touched files, outermost first, once each', () => {
    expect(
      touchedDirsToExpand(
        ['out/market/r.md', 'out/market/c.png', 'src/a.ts', 'top.md'],
        new Set(),
        new Set()
      )
    ).toEqual(['out', 'src', 'out/market'])
  })

  it('skips folders already open and folders the user collapsed', () => {
    expect(
      touchedDirsToExpand(['out/market/r.md', 'src/x/a.ts'], new Set(['out']), new Set(['src']))
    ).toEqual(['out/market', 'src/x'])
    expect(touchedDirsToExpand([], new Set(), new Set())).toEqual([])
  })

  it('only ever adds expansions', () => {
    const expanded = new Set(['user-opened'])
    const next = withExpanded(expanded, ['out', 'out/market'])
    expect([...next].sort()).toEqual(['out', 'out/market', 'user-opened'])
    expect(expanded.has('out')).toBe(false) // not mutated
    // Nothing new: the same set back.
    expect(withExpanded(next, ['out'])).toBe(next)
    expect(withExpanded(next, [])).toBe(next)
  })

  it('tracks user collapses per project folder and chat', () => {
    let collapsed = noteUserToggle(new Set(), 'out', false)
    collapsed = noteUserToggle(collapsed, 'src', false)
    collapsed = noteUserToggle(collapsed, 'src', true) // reopened: forgotten
    expect([...collapsed]).toEqual(['out'])
    rememberUserCollapsed('/p', 's1', collapsed)
    expect([...userCollapsedDirs('/p', 's1')]).toEqual(['out'])
    expect(userCollapsedDirs('/p', 's2').size).toBe(0)
    expect(userCollapsedDirs('/q', 's1').size).toBe(0)
    // The stored copy is not the caller's set.
    collapsed.add('later')
    expect(userCollapsedDirs('/p', 's1').has('later')).toBe(false)
  })
})

describe('file types', () => {
  it('maps extensions to preview kinds and languages', () => {
    expect(previewKind('MD')).toBe('markdown')
    expect(previewKind('htm')).toBe('html')
    expect(previewKind('svg')).toBe('image')
    expect(previewKind('pdf')).toBe('pdf')
    expect(previewKind('docx')).toBe('docx')
    expect(previewKind('zip')).toBe('binary')
    expect(previewKind('')).toBe('text')
    expect(languageFor('app.tsx', 'tsx')).toBe('tsx')
    expect(languageFor('Dockerfile', '')).toBe('docker')
    expect(languageFor('notes.unknown', 'unknown')).toBe('text')
  })

  it('formats sizes and relative times', () => {
    expect(formatSize(512)).toBe('512 B')
    expect(formatSize(4300)).toBe('4.2 KB')
    expect(formatSize(8.4 * 1024 * 1024)).toBe('8.4 MB')
    expect(relativeTime(1_000_000 - 10_000, 1_000_000, 'en')).toBeNull()
    expect(relativeTime(1_000_000 - 120_000, 1_000_000, 'en')).toBe('2 minutes ago')
    expect(relativeTime(0, 3 * 86_400_000, 'en')).toBe('3 days ago')
  })
})
