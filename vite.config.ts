// Vite config for the React renderer. The Tauri app (src-tauri/app) loads the dev server
// (build.devUrl) in development and bundles the build output (build.frontendDist).
import { resolve } from 'path'
import { readFileSync } from 'fs'
import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import svgr from 'vite-plugin-svgr'
import tailwindcss from 'tailwindcss'
import autoprefixer from 'autoprefixer'

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
  css: {
    postcss: {
      plugins: [tailwindcss() as any, autoprefixer() as any]
    }
  },
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
