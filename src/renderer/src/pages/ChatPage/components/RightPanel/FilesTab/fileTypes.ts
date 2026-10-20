/** How the Files panel previews a file, chosen by extension. */
export type PreviewKind = 'markdown' | 'html' | 'image' | 'pdf' | 'docx' | 'binary' | 'text'

const IMAGE = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico', 'avif'])

/** Extensions that are never text; everything else is tried as text and falls back. */
const BINARY = new Set([
  'zip',
  'gz',
  'tgz',
  'bz2',
  'xz',
  '7z',
  'rar',
  'tar',
  'jar',
  'war',
  'exe',
  'dll',
  'so',
  'dylib',
  'bin',
  'o',
  'a',
  'class',
  'pyc',
  'wasm',
  'dmg',
  'pkg',
  'iso',
  'img',
  'mp3',
  'wav',
  'flac',
  'ogg',
  'm4a',
  'aac',
  'mp4',
  'mov',
  'avi',
  'mkv',
  'webm',
  'm4v',
  'doc',
  'xls',
  'xlsx',
  'ppt',
  'pptx',
  'odt',
  'ods',
  'odp',
  'pages',
  'numbers',
  'key',
  'psd',
  'ai',
  'sketch',
  'fig',
  'heic',
  'tif',
  'tiff',
  'ttf',
  'otf',
  'woff',
  'woff2',
  'eot',
  'sqlite',
  'db',
  'parquet',
  'avro',
  'pickle',
  'pkl',
  'npy',
  'npz',
  'onnx',
  'pt',
  'safetensors'
])

export function previewKind(ext: string): PreviewKind {
  const e = ext.toLowerCase()
  if (e === 'md' || e === 'markdown' || e === 'mdx') return 'markdown'
  if (e === 'html' || e === 'htm') return 'html'
  if (IMAGE.has(e)) return 'image'
  if (e === 'pdf') return 'pdf'
  if (e === 'docx') return 'docx'
  if (BINARY.has(e)) return 'binary'
  return 'text'
}

/** Kinds whose content is read as text (and can be edited). */
export const isTextKind = (kind: PreviewKind): boolean =>
  kind === 'markdown' || kind === 'html' || kind === 'text'

/** Kinds with both a rendered view and a source view. */
export const hasRenderedView = (kind: PreviewKind): boolean =>
  kind === 'markdown' || kind === 'html'

const LANGUAGES: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  mts: 'typescript',
  cts: 'typescript',
  js: 'javascript',
  jsx: 'jsx',
  mjs: 'javascript',
  cjs: 'javascript',
  json: 'json',
  jsonc: 'json',
  json5: 'json5',
  py: 'python',
  pyi: 'python',
  rb: 'ruby',
  rs: 'rust',
  go: 'go',
  java: 'java',
  kt: 'kotlin',
  kts: 'kotlin',
  scala: 'scala',
  swift: 'swift',
  c: 'c',
  h: 'c',
  cc: 'cpp',
  cpp: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  cs: 'csharp',
  php: 'php',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  fish: 'bash',
  ps1: 'powershell',
  bat: 'batch',
  cmd: 'batch',
  sql: 'sql',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
  ini: 'ini',
  cfg: 'ini',
  conf: 'ini',
  env: 'bash',
  xml: 'xml',
  svg: 'xml',
  html: 'markup',
  htm: 'markup',
  vue: 'markup',
  svelte: 'markup',
  css: 'css',
  scss: 'scss',
  sass: 'sass',
  less: 'less',
  md: 'markdown',
  markdown: 'markdown',
  mdx: 'markdown',
  graphql: 'graphql',
  gql: 'graphql',
  proto: 'protobuf',
  tf: 'hcl',
  hcl: 'hcl',
  lua: 'lua',
  r: 'r',
  dart: 'dart',
  ex: 'elixir',
  exs: 'elixir',
  erl: 'erlang',
  hs: 'haskell',
  clj: 'clojure',
  ml: 'ocaml',
  pl: 'perl',
  diff: 'diff',
  patch: 'diff',
  csv: 'csv',
  dockerfile: 'docker'
}

/** File names without a telling extension. */
const NAMED: Record<string, string> = {
  dockerfile: 'docker',
  makefile: 'makefile',
  gemfile: 'ruby',
  rakefile: 'ruby',
  '.bashrc': 'bash',
  '.zshrc': 'bash',
  '.env': 'bash'
}

/** Prism language for syntax highlighting (`text` when unknown). */
export function languageFor(name: string, ext: string): string {
  return NAMED[name.toLowerCase()] ?? LANGUAGES[ext.toLowerCase()] ?? 'text'
}

/** "4.2 KB" style sizes. */
export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return ''
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

/**
 * "2 minutes ago" in `locale`, or `null` for under a minute (the caller says "just now").
 */
export function relativeTime(ms: number, now: number, locale: string): string | null {
  const seconds = Math.round((ms - now) / 1000)
  if (Math.abs(seconds) < 60) return null
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ['minute', 60],
    ['hour', 3600],
    ['day', 86400],
    ['month', 2592000],
    ['year', 31536000]
  ]
  let chosen = steps[0]
  for (const step of steps) if (Math.abs(seconds) >= step[1]) chosen = step
  return rtf.format(Math.round(seconds / chosen[1]), chosen[0])
}
