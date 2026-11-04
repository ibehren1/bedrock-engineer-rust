// Vite config for the React renderer. The Tauri app (src-tauri/app) loads the dev server
// (build.devUrl) in development and bundles the build output (build.frontendDist).
import { resolve } from 'path'
import { globSync, readFileSync, writeFileSync } from 'fs'
import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import svgr from 'vite-plugin-svgr'
import tailwindcss from '@tailwindcss/vite'
import { compile } from '@tailwindcss/node'
import { buildClassList } from 'flowbite-react/cli/utils/build-class-list'
import { extractComponentImports } from 'flowbite-react/cli/utils/extract-component-imports'

// Single source of truth for the application display name: package.json productName.
const appName: string = JSON.parse(readFileSync(resolve('package.json'), 'utf-8')).productName

/** Node built-in → browser stand-in module (html-to-docx and its dependencies import these). */
function nodeShimAliases(dir: string): { find: RegExp; replacement: string }[] {
  return [
    { find: /^events$/, replacement: resolve(dir, 'events.ts') },
    { find: /^stream$/, replacement: resolve(dir, 'stream.ts') },
    { find: /^util$/, replacement: resolve(dir, 'util.ts') },
    { find: /^crypto$/, replacement: resolve(dir, 'crypto.ts') },
    { find: /^url$/, replacement: resolve(dir, 'url.ts') },
    { find: /^(fs|path|http|https|zlib)$/, replacement: resolve(dir, 'unavailable.ts') }
  ]
}

/**
 * Dependencies that fetch the global object with `eval` / `new Function`, which the app's
 * Content-Security-Policy (script-src without 'unsafe-eval', src-tauri/app/tauri.conf.json)
 * forbids, get `globalThis` instead:
 * - sandpack-react's bundled console-feed does `savedEval("this")` at module load; the EvalError
 *   stopped the whole bundle.
 * - webpack's `global` shim (`new Function("return this")()`, e.g. in @tshepomgaga/aws-sfn-graph)
 *   is caught, but still logged a CSP violation on every start.
 * Warns if the sandpack pattern disappears (e.g. after an upgrade) so the CSP can be rechecked.
 */
function globalThisInsteadOfEval(): Plugin {
  const sandpackEval = /var savedEval = eval;\s*return savedEval\("this"\);/
  const functionThis = /new Function\(["']return this["']\)\(\)/g
  let sandpackSeen = false
  let sandpackPatched = false
  return {
    name: 'global-this-instead-of-eval',
    transform(code, id) {
      if (!id.includes('node_modules')) return null
      let out = code
      if (/[\\/]@codesandbox[\\/]sandpack-react[\\/]/.test(id) && code.includes('savedEval')) {
        sandpackSeen = true
        out = out.replace(sandpackEval, 'return globalThis;')
        sandpackPatched ||= out !== code
      }
      out = out.replace(functionThis, 'globalThis')
      return out === code ? null : { code: out, map: null }
    },
    buildEnd() {
      if (sandpackSeen && !sandpackPatched) {
        this.warn('sandpack-react eval("this") pattern not found; check the CSP still holds')
      }
    }
  }
}

/** Write `content` to `file` only when it differs, so Tailwind doesn't rebuild on every save. */
function writeIfChanged(file: string, content: string): void {
  let current: string | null = null
  try {
    current = readFileSync(file, 'utf-8')
  } catch {
    // first run
  }
  if (current !== content) writeFileSync(file, content)
}

/** The body of the first top-level `@layer <name> { … }` block in `css`. */
function layerBody(css: string, name: string): string {
  const start = css.indexOf(`@layer ${name} {`)
  if (start < 0) throw new Error(`no @layer ${name} in the generated CSS`)
  let depth = 0
  for (let i = css.indexOf('{', start); i < css.length; i++) {
    if (css[i] === '{') depth++
    else if (css[i] === '}' && --depth === 0) return css.slice(css.indexOf('{', start) + 1, i)
  }
  throw new Error(`unterminated @layer ${name}`)
}

/**
 * Writes the two files index.css reads from .flowbite-react/ (git-ignored, regenerated on
 * every build and dev start):
 *
 * - class-list.json: the classes of the Flowbite React components the renderer imports, for
 *   Tailwind to generate (@source). Settings come from .flowbite-react/config.json.
 * - flowbite-base.css: the flowbite plugin's form and tooltip base styles, which index.css
 *   imports into the utilities layer (see tailwind.flowbite.cjs for why).
 *
 * This replaces flowbite-react's own Vite plugin, which also rewrites index.css on every build.
 */
function flowbiteAssets(): Plugin {
  const dir = resolve('.flowbite-react')
  const writeClassList = () => {
    const config = JSON.parse(readFileSync(resolve(dir, 'config.json'), 'utf-8'))
    const files = globSync('src/renderer/src/**/*.{ts,tsx}', {
      exclude: (f) => f.endsWith('.d.ts')
    })
    const components = [
      ...new Set(files.flatMap((file) => extractComponentImports(readFileSync(file, 'utf-8'))))
    ]
    const classList = buildClassList({
      components: config.components?.length ? config.components : components,
      dark: config.dark,
      prefix: config.prefix,
      version: config.version
    })
    writeIfChanged(resolve(dir, 'class-list.json'), JSON.stringify(classList, null, 2))
  }
  const writeBaseCss = async () => {
    // The app's theme and config, so theme() lookups resolve as they do in index.css.
    const source = [
      "@import 'tailwindcss/theme.css' layer(theme);",
      `@config '${resolve('tailwind.config.js')}';`,
      `@plugin '${resolve('tailwind.flowbite.cjs')}' { base: true; }`
    ].join('\n')
    const compiler = await compile(source, { base: resolve('.'), onDependency: () => {} })
    const body = layerBody(compiler.build([]), 'base')
    writeIfChanged(
      resolve(dir, 'flowbite-base.css'),
      `/* Generated by vite.config.mts (flowbiteAssets) from tailwind.flowbite.cjs. */\n${body}\n`
    )
  }
  return {
    name: 'flowbite-assets',
    async buildStart() {
      writeClassList()
      await writeBaseCss()
    },
    async handleHotUpdate({ file, read }) {
      if (/\.tsx?$/.test(file) && (await read()).includes('flowbite-react')) writeClassList()
    }
  }
}

// Tauri's dev server URL (tauri.conf.json build.devUrl) is fixed, so the port must be too.
const DEV_PORT = 5173

export default defineConfig({
  root: resolve('src/renderer'),
  // Tauri serves frontendDist from its own custom protocol; relative asset paths keep the
  // bundle location-independent.
  base: './',
  publicDir: resolve('src/renderer/public'),
  clearScreen: false,
  define: {
    __APP_NAME__: JSON.stringify(appName)
  },
  resolve: {
    alias: [
      { find: '@renderer', replacement: resolve('src/renderer/src') },
      { find: '@', replacement: resolve('src') },
      { find: '@common', replacement: resolve('src/common') },
      // The Word export runs html-to-docx (a Node bundle) in the renderer; its Node built-in
      // imports resolve to browser stand-ins (src/renderer/src/lib/docx/htmlToDocx.ts).
      ...nodeShimAliases(resolve('src/renderer/src/lib/docx/nodeShims'))
    ]
  },
  plugins: [
    globalThisInsteadOfEval(),
    // Tailwind CSS 4 (index.css), with the Flowbite component classes it must generate.
    flowbiteAssets(),
    tailwindcss(),
    react(),
    svgr({
      svgrOptions: {
        exportType: 'default',
        ref: true,
        svgo: false,
        titleProp: true
      },
      include: '**/*.svg'
    })
  ],
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: {
      // The Rust side rebuilds itself; don't let Vite reload on target/ churn.
      ignored: ['**/src-tauri/**']
    }
  },
  build: {
    outDir: resolve('dist/renderer'),
    emptyOutDir: true,
    // Tauri v2 webviews: WebKit on macOS/Linux, Chromium-based WebView2 on Windows.
    target: ['es2021', 'chrome105', 'safari15'],
    rollupOptions: {
      input: {
        index: resolve('src/renderer/index.html'),
        // Camera preview windows (camera_show_preview_window) load this page directly.
        cameraPreview: resolve('src/renderer/camera-preview.html')
      }
    }
  }
})
