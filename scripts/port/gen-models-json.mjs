#!/usr/bin/env node
// Generates src-tauri/crates/models/data/models.json from the TypeScript model registry
// (src/common/models/models.ts), which remains the source of truth during the port.
//
// Usage (from the repo root, Node >= 23.6 for built-in TypeScript type stripping):
//   node scripts/port/gen-models-json.mjs          # regenerate the JSON
//   node scripts/port/gen-models-json.mjs --check  # exit 1 if the JSON is stale
//
// The registry arrays are module-private in models.ts, so this script copies the file to a
// temporary .ts module, appends an export of MODEL_REGISTRY / IMAGE_GENERATION_MODELS, and
// imports it. Only `import type` statements exist in models.ts, so type stripping suffices.
//
// Besides the registry itself, the JSON carries `regionOrder`: for every region referenced by
// the registry, the modelIds that `getModelsForRegion` returns (sorted with JS `localeCompare`).
// The Rust crate's tests check its own collation against these fixtures.

import {
  copyFileSync,
  appendFileSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const source = join(repoRoot, 'src/common/models/models.ts')
const outFile = join(repoRoot, 'src-tauri/crates/models/data/models.json')

const tmp = mkdtempSync(join(tmpdir(), 'gen-models-'))
try {
  const tmpModule = join(tmp, 'models.ts')
  copyFileSync(source, tmpModule)
  appendFileSync(
    tmpModule,
    '\nexport const __TEXT_MODELS = MODEL_REGISTRY\nexport const __IMAGE_MODELS = IMAGE_GENERATION_MODELS\n'
  )
  // The module is a temporary copy written above, so it can only be imported at run time.
  // eslint-disable-next-line no-restricted-syntax
  const mod = await import(pathToFileURL(tmpModule).href)

  const regions = new Set()
  for (const config of [...mod.__TEXT_MODELS, ...mod.__IMAGE_MODELS]) {
    for (const profile of config.inferenceProfiles) profile.regions.forEach((r) => regions.add(r))
  }
  const regionOrder = {}
  for (const region of [...regions].sort()) {
    regionOrder[region] = mod.getModelsForRegion(region).map((m) => m.modelId)
  }
  const imageRegionOrder = {}
  for (const region of [...regions].sort()) {
    imageRegionOrder[region] = mod.getImageGenerationModelsForRegion(region).map((m) => m.id)
  }

  const json =
    JSON.stringify(
      {
        _generated:
          'by scripts/port/gen-models-json.mjs from src/common/models/models.ts; do not edit',
        textModels: mod.__TEXT_MODELS,
        imageModels: mod.__IMAGE_MODELS,
        regionOrder,
        imageRegionOrder
      },
      null,
      2
    ) + '\n'

  if (process.argv.includes('--check')) {
    let current = ''
    try {
      current = readFileSync(outFile, 'utf8')
    } catch {
      // missing file counts as stale
    }
    if (current !== json) {
      console.error(`${outFile} is stale; run: node scripts/port/gen-models-json.mjs`)
      process.exit(1)
    }
    console.log('models.json is up to date')
  } else {
    writeFileSync(outFile, json)
    console.log(
      `wrote ${outFile} (${mod.__TEXT_MODELS.length} text, ${mod.__IMAGE_MODELS.length} image configs)`
    )
  }
} finally {
  rmSync(tmp, { recursive: true, force: true })
}
