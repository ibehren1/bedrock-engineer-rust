import { createCspReporter, MAX_REPORTS } from './cspViolationLogger'

const violation = (blockedURI: string, effectiveDirective = 'script-src') => ({
  effectiveDirective,
  blockedURI,
  sourceFile: 'tauri://localhost/assets/index.js',
  lineNumber: 1,
  disposition: 'enforce' as const
})

describe('createCspReporter', () => {
  test('builds a renderer warn entry', () => {
    const report = createCspReporter(() => new Date('2026-01-01T00:00:00Z'))
    expect(report(violation('https://cdn.example/x.js'))).toEqual({
      level: 'warn',
      message: 'CSP blocked https://cdn.example/x.js (script-src)',
      timestamp: '2026-01-01T00:00:00.000Z',
      process: 'renderer',
      category: 'security:csp',
      directive: 'script-src',
      blockedURI: 'https://cdn.example/x.js',
      sourceFile: 'tauri://localhost/assets/index.js',
      lineNumber: 1,
      disposition: 'enforce'
    })
  })

  test('logs each directive + resource once', () => {
    const report = createCspReporter()
    expect(report(violation(''))).not.toBeNull()
    expect(report(violation(''))).toBeNull()
    expect(report(violation('', 'style-src'))).not.toBeNull()
  })

  test('stops after the cap', () => {
    const report = createCspReporter()
    for (let i = 0; i < MAX_REPORTS; i++) expect(report(violation(`https://h${i}/`))).not.toBeNull()
    expect(report(violation('https://one-more/'))).toBeNull()
  })
})
