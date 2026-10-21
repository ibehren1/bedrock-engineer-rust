// node --test scripts/release
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { buildLatestJson } from './latest-json.mjs'

const V = '2026.1105.0'
const allFiles = () =>
  new Map([
    [`bedrock-engineer-rust-${V}-universal.app.tar.gz`, ''],
    [`bedrock-engineer-rust-${V}-universal.app.tar.gz.sig`, 'mac-sig\n'],
    [`bedrock-engineer-rust-${V}-x64.setup.exe`, ''],
    [`bedrock-engineer-rust-${V}-x64.setup.exe.sig`, 'win-sig'],
    [`bedrock-engineer-rust-${V}-x64.AppImage`, ''],
    [`bedrock-engineer-rust-${V}-x64.AppImage.sig`, 'linux-sig'],
    [`bedrock-engineer-rust-${V}-universal.dmg`, '']
  ])

const build = (files) =>
  buildLatestJson({
    version: V,
    repo: 'owner/repo',
    notes: '### 2026-11-05\n\n- Something\n',
    files,
    pubDate: '2026-11-05T00:00:00.000Z'
  })

test('lists every platform with its signature and release URL', () => {
  const manifest = build(allFiles())
  assert.equal(manifest.version, V)
  assert.equal(manifest.notes, '### 2026-11-05\n\n- Something')
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    'darwin-aarch64',
    'darwin-x86_64',
    'linux-x86_64',
    'windows-x86_64'
  ])
  assert.deepEqual(manifest.platforms['darwin-aarch64'], {
    signature: 'mac-sig',
    url: `https://github.com/owner/repo/releases/download/v${V}/bedrock-engineer-rust-${V}-universal.app.tar.gz`
  })
  // Both macOS architectures install the universal bundle.
  assert.deepEqual(manifest.platforms['darwin-x86_64'], manifest.platforms['darwin-aarch64'])
  assert.equal(manifest.platforms['windows-x86_64'].signature, 'win-sig')
})

test('refuses a manifest with a bundle or signature missing', () => {
  const noSig = allFiles()
  noSig.delete(`bedrock-engineer-rust-${V}-x64.setup.exe.sig`)
  assert.throws(() => build(noSig), /windows-x86_64: .*setup\.exe\.sig/)

  const emptySig = allFiles()
  emptySig.set(`bedrock-engineer-rust-${V}-universal.app.tar.gz.sig`, '  \n')
  assert.throws(() => build(emptySig), /darwin-aarch64/)

  const noBundle = allFiles()
  noBundle.delete(`bedrock-engineer-rust-${V}-x64.AppImage`)
  assert.throws(() => build(noBundle), /linux-x86_64/)
})
