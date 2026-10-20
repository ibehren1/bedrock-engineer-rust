import {
  ancestorDirs,
  changedPathsRelative,
  detectFilePath,
  extOf,
  extractTouchedPaths,
  fileToolTargets,
  groupByFolder,
  toAbsolutePath,
  toProjectRelative
} from './paths'

const ROOT = '/Users/me/acme'

describe('toProjectRelative', () => {
  it('maps absolute paths under the project folder', () => {
    expect(toProjectRelative('/Users/me/acme/out/report.md', ROOT)).toBe('out/report.md')
    expect(toProjectRelative('/Users/me/acme/', ROOT)).toBe('')
    expect(toProjectRelative('/Users/me/acme/a/../b.md', ROOT)).toBe('b.md')
  })

  it('drops paths outside the project folder', () => {
    expect(toProjectRelative('/Users/me/acme-other/x.md', ROOT)).toBeNull()
    expect(toProjectRelative('/etc/passwd', ROOT)).toBeNull()
    expect(toProjectRelative('../x.md', ROOT)).toBeNull()
    expect(toProjectRelative('/Users/me/acme/../secret', ROOT)).toBeNull()
    expect(toProjectRelative('x.md', undefined)).toBeNull()
  })

  it('normalizes relative paths', () => {
    expect(toProjectRelative('./src//app.ts', ROOT)).toBe('src/app.ts')
    expect(toProjectRelative('src\\app.ts', ROOT)).toBe('src/app.ts')
  })

  it('compares Windows paths case-insensitively', () => {
    expect(toProjectRelative('c:\\Work\\Acme\\docs\\a.md', 'C:\\work\\acme')).toBe('docs/a.md')
    expect(toProjectRelative('D:\\work\\acme\\a.md', 'C:\\work\\acme')).toBeNull()
  })
})

describe('toAbsolutePath', () => {
  it('joins in the project folder separator style', () => {
    expect(toAbsolutePath(ROOT, 'out/r.md')).toBe('/Users/me/acme/out/r.md')
    expect(toAbsolutePath('/Users/me/acme/', 'r.md')).toBe('/Users/me/acme/r.md')
    expect(toAbsolutePath('C:\\work\\acme', 'out/r.md')).toBe('C:\\work\\acme\\out\\r.md')
    expect(toAbsolutePath(ROOT, '')).toBe(ROOT)
  })
})

describe('small helpers', () => {
  it('lists ancestors, extensions and folder groups', () => {
    expect(ancestorDirs('a/b/c.md')).toEqual(['a', 'a/b'])
    expect(ancestorDirs('c.md')).toEqual([])
    expect(extOf('a/Report.DOCX')).toBe('docx')
    expect(extOf('.gitignore')).toBe('')
    expect(groupByFolder(['b/z.md', 'top.md', 'a/y.md', 'b/a.md'])).toEqual([
      { folder: '', paths: ['top.md'] },
      { folder: 'a', paths: ['a/y.md'] },
      { folder: 'b', paths: ['b/a.md', 'b/z.md'] }
    ])
  })
})

describe('extractTouchedPaths', () => {
  it('reads the target fields of file-writing tools', () => {
    expect(extractTouchedPaths('writeToFile', { path: `${ROOT}/out/r.md` }, ROOT)).toEqual([
      'out/r.md'
    ])
    expect(extractTouchedPaths('applyDiffEdit', { path: 'src/app.ts' }, ROOT)).toEqual([
      'src/app.ts'
    ])
    expect(extractTouchedPaths('createFolder', { path: `${ROOT}/out` }, ROOT)).toEqual(['out'])
    expect(
      extractTouchedPaths('moveFile', { source: `${ROOT}/a.md`, destination: `${ROOT}/b.md` }, ROOT)
    ).toEqual(['b.md'])
    expect(
      extractTouchedPaths('copyFile', { source: `${ROOT}/a.md`, destination: 'c/a.md' }, ROOT)
    ).toEqual(['c/a.md'])
  })

  it('ignores other tools, missing fields and paths outside the project', () => {
    expect(extractTouchedPaths('readFiles', { paths: [`${ROOT}/a.md`] }, ROOT)).toEqual([])
    expect(extractTouchedPaths('writeToFile', { path: '/tmp/x.md' }, ROOT)).toEqual([])
    expect(extractTouchedPaths('writeToFile', { path: 3 }, ROOT)).toEqual([])
    expect(extractTouchedPaths('writeToFile', null, ROOT)).toEqual([])
    expect(extractTouchedPaths('writeToFile', { path: 'a.md' }, undefined)).toEqual([])
    expect(extractTouchedPaths(undefined, { path: 'a.md' }, ROOT)).toEqual([])
  })
})

describe('fileToolTargets', () => {
  it('lists the paths a file tool names', () => {
    expect(fileToolTargets('readFiles', { paths: ['a.md', 3, 'b/c.ts'] })).toEqual([
      'a.md',
      'b/c.ts'
    ])
    expect(fileToolTargets('moveFile', { source: 'a', destination: 'b/a' })).toEqual(['b/a'])
    expect(fileToolTargets('writeToFile', { path: '' })).toEqual([])
    expect(fileToolTargets('executeCommand', { command: 'ls' })).toEqual([])
  })
})

describe('changedPathsRelative', () => {
  it('accepts absolute and relative event paths, deduplicated', () => {
    expect(
      changedPathsRelative(
        { root: ROOT, paths: [`${ROOT}/a.md`, 'a.md', 'b/c.md', '/elsewhere/x'] },
        ROOT
      )
    ).toEqual(['a.md', 'b/c.md'])
  })
})

describe('detectFilePath', () => {
  it('accepts relative paths with a folder or an extension', () => {
    expect(detectFilePath('output/market-analysis/report.md', ROOT)).toBe(
      'output/market-analysis/report.md'
    )
    expect(detectFilePath('report.docx', ROOT)).toBe('report.docx')
    expect(detectFilePath('./src/app.ts', ROOT)).toBe('src/app.ts')
    expect(detectFilePath('src/components/', ROOT)).toBe('src/components')
    expect(detectFilePath('src/app.ts:12:3', ROOT)).toBe('src/app.ts')
    expect(detectFilePath('.env.local', ROOT)).toBe('.env.local')
  })

  it('accepts absolute paths only under the project folder', () => {
    expect(detectFilePath(`${ROOT}/out/r.pdf`, ROOT)).toBe(`${ROOT}/out/r.pdf`)
    expect(detectFilePath('/etc/hosts', ROOT)).toBeNull()
  })

  it('rejects things that are not paths', () => {
    for (const text of [
      'npm install',
      'https://example.com/a.md',
      'mailto:a@b.c',
      'README',
      'v1.2.3',
      '1.5',
      'foo()',
      'a = b.c',
      '--flag',
      '~/x.md',
      '../outside.md',
      'line one\nline/two',
      ''
    ]) {
      expect(detectFilePath(text, ROOT)).toBeNull()
    }
  })

  it('does nothing without a project folder', () => {
    expect(detectFilePath('src/app.ts', undefined)).toBeNull()
  })
})
