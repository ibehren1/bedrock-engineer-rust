/**
 * Content-Security-Policy violations → the app log (`logger_log`, category `security:csp`), so a
 * resource the policy in src-tauri/app/tauri.conf.json blocks shows up in the log files instead
 * of only in the (usually closed) web inspector. Low noise: each distinct directive + blocked
 * resource is logged once, and at most {@link MAX_REPORTS} per page load.
 */
import { invoke } from '@tauri-apps/api/core'

export const MAX_REPORTS = 50

type Violation = Pick<
  SecurityPolicyViolationEvent,
  'effectiveDirective' | 'blockedURI' | 'sourceFile' | 'lineNumber' | 'disposition'
>

export type CspLogEntry = {
  level: 'warn'
  message: string
  timestamp: string
  process: 'renderer'
  category: 'security:csp'
  directive: string
  blockedURI: string
  sourceFile: string
  lineNumber: number
  disposition: string
}

/** A reporter that turns violations into log entries, or `null` for repeats / past the cap. */
export function createCspReporter(now: () => Date = () => new Date()) {
  const seen = new Set<string>()
  return (v: Violation): CspLogEntry | null => {
    const blocked = v.blockedURI || 'inline'
    const key = `${v.effectiveDirective} ${blocked}`
    if (seen.has(key) || seen.size >= MAX_REPORTS) return null
    seen.add(key)
    return {
      level: 'warn',
      message: `CSP blocked ${blocked} (${v.effectiveDirective})`,
      timestamp: now().toISOString(),
      process: 'renderer',
      category: 'security:csp',
      directive: v.effectiveDirective,
      blockedURI: blocked,
      sourceFile: v.sourceFile,
      lineNumber: v.lineNumber,
      disposition: v.disposition
    }
  }
}

/** Start forwarding violations to the app log. */
export function installCspViolationLogger(): void {
  if (typeof document === 'undefined') return
  const report = createCspReporter()
  document.addEventListener('securitypolicyviolation', (event) => {
    const entry = report(event)
    if (entry) invoke('logger_log', { entry }).catch(() => undefined)
  })
}
