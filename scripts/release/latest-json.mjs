#!/usr/bin/env node
// Build the updater manifest (latest.json) for a GitHub release.
//
// The app's updater (tauri-plugin-updater, see src-tauri/app/src/commands/app_update.rs) fetches
// https://github.com/<repo>/releases/latest/download/latest.json, picks its platform's entry and
// downloads `url`, then checks it against `signature` with the public key in tauri.conf.json.
//
// Usage (from the release job, after the build artifacts are downloaded):
//   node scripts/release/latest-json.mjs --dir artifacts --version 2026.1021.7 \
//     --repo owner/name --notes update-summary.md --out latest.json
//
// The installers are named by the "Collect installers" step:
//   bedrock-engineer-rust-<version>-universal.app.tar.gz (+ .sig)  macOS, both architectures
//   bedrock-engineer-rust-<version>-x64.setup.exe (+ .sig)         Windows (NSIS)
//   bedrock-engineer-rust-<version>-x64.AppImage (+ .sig)          Linux
// Exits non-zero when any of them, or its signature, is missing: a manifest that names a
// platform without a signed bundle would break updates there.
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { basename, join } from 'node:path'
import { fileURLToPath } from 'node:url'

/** Updater platform keys, and the installer each one installs from. */
export const PLATFORM_ASSETS = {
  'darwin-aarch64': (v) => `bedrock-engineer-rust-${v}-universal.app.tar.gz`,
  'darwin-x86_64': (v) => `bedrock-engineer-rust-${v}-universal.app.tar.gz`,
  'windows-x86_64': (v) => `bedrock-engineer-rust-${v}-x64.setup.exe`,
  'linux-x86_64': (v) => `bedrock-engineer-rust-${v}-x64.AppImage`
}

/**
 * The manifest for `version`, given `files` (asset name -> contents, for the `.sig` files) and
 * the release notes. Throws when a platform's bundle or signature is missing.
 */
export function buildLatestJson({ version, repo, notes, files, pubDate }) {
  const platforms = {}
  const missing = []
  for (const [platform, assetFor] of Object.entries(PLATFORM_ASSETS)) {
    const asset = assetFor(version)
    const signature = files.get(`${asset}.sig`)?.trim()
    if (!files.has(asset) || !signature) {
      missing.push(`${platform}: ${asset}${files.has(asset) ? '.sig' : ''}`)
      continue
    }
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/v${version}/${asset}`
    }
  }
  if (missing.length) throw new Error(`Missing updater assets:\n  ${missing.join('\n  ')}`)
  return { version, notes: notes.trim(), pub_date: pubDate, platforms }
}

/** Every file under `dir` (artifact downloads are one folder per job), by name. */
function collect(dir) {
  const files = new Map()
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const path = join(d, name)
      if (statSync(path).isDirectory()) walk(path)
      else files.set(basename(path), name.endsWith('.sig') ? readFileSync(path, 'utf8') : '')
    }
  }
  walk(dir)
  return files
}

const arg = (args, name) => {
  const i = args.indexOf(`--${name}`)
  if (i < 0 || !args[i + 1]) throw new Error(`--${name} is required`)
  return args[i + 1]
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2)
    const manifest = buildLatestJson({
      version: arg(args, 'version'),
      repo: arg(args, 'repo'),
      notes: readFileSync(arg(args, 'notes'), 'utf8'),
      files: collect(arg(args, 'dir')),
      pubDate: new Date().toISOString()
    })
    writeFileSync(arg(args, 'out'), JSON.stringify(manifest, null, 2) + '\n')
    console.log(`latest-json: ${Object.keys(manifest.platforms).join(', ')}`)
  } catch (error) {
    console.error(`latest-json: ${error.message}`)
    process.exit(1)
  }
}
