#!/usr/bin/env node
// Generate third-party notices for the JavaScript dependencies that ship in the app.
//
// Walks the production dependency tree (`npm ls --omit=dev --all`), reads each installed
// package's package.json and LICENSE/NOTICE files, and writes one Markdown document with a
// license summary followed by every package's license text. No extra npm dependencies.
//
// The production tree is a superset of what Vite actually bundles into the renderer (tree
// shaking drops some of it); over-reporting is the safe direction for notices.
//
// Usage:
//   node scripts/notices/frontend-notices.mjs [--out notices/frontend.md] [--strict]
//
// --strict exits non-zero when a package's license is not on the accept-list below (e.g. a
// copyleft license or a missing license field). Without it, those packages are only reported.
import { execFileSync } from 'node:child_process'
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'

// Permissive licenses that need no review. Keep in step with src-tauri/about.toml `accepted`.
const ACCEPTED = new Set([
  'MIT',
  'MIT-0',
  'ISC',
  'BSD-2-Clause',
  'BSD-3-Clause',
  '0BSD',
  'Apache-2.0',
  'Zlib',
  'Unicode-3.0',
  'Unicode-DFS-2016',
  'CC0-1.0',
  'CC-BY-4.0',
  'Unlicense',
  'BlueOak-1.0.0',
  'Python-2.0',
  'WTFPL',
  'BSL-1.0',
  // Attribution-only licenses for bundled fonts, icon sets and polyfills.
  'OFL-1.1',
  'CC-BY-3.0',
  'W3C-20150513'
])

// Packages whose package.json license field is missing or vague, pinned by version to the
// license their shipped LICENSE file actually contains (manually reviewed).
const OVERRIDES = {
  'khroma@2.1.0': 'MIT',
  'duck@0.1.12': 'BSD-2-Clause'
}

// Reviewed packages outside the accept-list that are allowed for now. They are still listed in
// the console output on every run; --strict only fails on packages NOT in this map.
const KNOWN_EXCEPTIONS = {
  '@codesandbox/nodebox':
    'Sustainable Use License (source-available, non-commercial redistribution terms); ' +
    'transitive dep of @codesandbox/sandpack-react via sandpack-client. Accepted: the app is ' +
    'distributed free of charge for internal use, which these terms allow. See NOTICE.'
}

const args = process.argv.slice(2)
const strict = args.includes('--strict')
const outIdx = args.indexOf('--out')
const repoRoot = resolve(dirname(new URL(import.meta.url).pathname), '..', '..')
const outPath = resolve(repoRoot, outIdx >= 0 ? args[outIdx + 1] : 'notices/frontend.md')

const rootPkg = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'))

// `npm ls` exits non-zero on benign tree problems (extraneous/invalid peer); the output is
// still complete, so take stdout either way.
let lsOut
try {
  lsOut = execFileSync('npm', ['ls', '--omit=dev', '--all', '--parseable'], {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'ignore']
  })
} catch (err) {
  lsOut = err.stdout ?? ''
}
const dirs = [...new Set(lsOut.split('\n').map((l) => l.trim()).filter(Boolean))].filter(
  (d) => resolve(d) !== repoRoot
)
if (dirs.length === 0) {
  console.error('frontend-notices: npm ls returned no packages (run `npm ci` first)')
  process.exit(1)
}

/** Normalise the many historical shapes of the package.json license field to an SPDX-ish string. */
function licenseOf(pkg) {
  let l = pkg.license ?? pkg.licenses
  if (Array.isArray(l)) l = l.map((x) => (typeof x === 'string' ? x : x?.type)).join(' OR ')
  else if (l && typeof l === 'object') l = l.type
  return (l || 'UNKNOWN').trim()
}

/** An SPDX expression is acceptable when any OR-branch is fully accepted. */
function isAccepted(expr) {
  const clean = expr.replace(/[()]/g, ' ')
  return clean
    .split(/\s+OR\s+/i)
    .some((branch) =>
      branch
        .split(/\s+AND\s+/i)
        .map((s) => s.trim())
        .every((id) => ACCEPTED.has(id))
    )
}

function licenseTexts(dir) {
  let files = []
  try {
    files = readdirSync(dir).filter((f) => /^(licen[cs]e|copying|notice)(\.|-|$)/i.test(f))
  } catch {
    return []
  }
  return files.sort().map((f) => ({ file: f, text: readFileSync(join(dir, f), 'utf8').trim() }))
}

const seen = new Map()
for (const dir of dirs) {
  const pj = join(dir, 'package.json')
  if (!existsSync(pj)) continue
  const pkg = JSON.parse(readFileSync(pj, 'utf8'))
  if (!pkg.name) continue
  const key = `${pkg.name}@${pkg.version}`
  if (seen.has(key)) continue
  const repo = typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url
  seen.set(key, {
    name: pkg.name,
    version: pkg.version,
    license: OVERRIDES[key] ?? licenseOf(pkg),
    repo: repo || pkg.homepage || '',
    texts: licenseTexts(dir)
  })
}

const pkgs = [...seen.values()].sort((a, b) => a.name.localeCompare(b.name))
const byLicense = new Map()
for (const p of pkgs) byLicense.set(p.license, (byLicense.get(p.license) ?? 0) + 1)
const flagged = pkgs.filter((p) => !isAccepted(p.license))
const unexpected = flagged.filter((p) => !(p.name in KNOWN_EXCEPTIONS))

const lines = []
lines.push(`# Third-party notices: JavaScript dependencies`)
lines.push('')
lines.push(
  `${rootPkg.productName} includes the following ${pkgs.length} npm packages. Generated by ` +
    '`scripts/notices/frontend-notices.mjs`; do not edit by hand.'
)
lines.push('')
lines.push('## License summary')
lines.push('')
lines.push('| License | Packages |')
lines.push('| ------- | -------- |')
for (const [lic, n] of [...byLicense.entries()].sort((a, b) => b[1] - a[1])) {
  lines.push(`| ${lic} | ${n} |`)
}
lines.push('')
for (const p of pkgs) {
  lines.push(`## ${p.name} ${p.version}`)
  lines.push('')
  lines.push(`License: ${p.license}${p.repo ? `  \nSource: ${p.repo}` : ''}`)
  lines.push('')
  if (p.texts.length === 0) {
    lines.push('_No license file shipped with the package; see the license field above._')
    lines.push('')
  }
  for (const t of p.texts) {
    lines.push('```text')
    lines.push(t.text.replace(/```/g, "'''"))
    lines.push('```')
    lines.push('')
  }
}

mkdirSync(dirname(outPath), { recursive: true })
writeFileSync(outPath, lines.join('\n'))
console.log(`frontend-notices: wrote ${pkgs.length} packages to ${outPath}`)
if (flagged.length) {
  console.warn(`frontend-notices: ${flagged.length} package(s) outside the accept-list:`)
  for (const p of flagged) {
    const note = KNOWN_EXCEPTIONS[p.name]
    console.warn(`  ${p.name}@${p.version}: ${p.license}${note ? ` (known: ${note})` : ''}`)
  }
  if (strict && unexpected.length) process.exit(1)
}
