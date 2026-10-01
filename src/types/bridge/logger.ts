// `window.logger`, installed by src/renderer/src/lib/tauriBridge.ts. Entries go to the Rust
// logger (`logger_log`), which writes the daily log files under <userData>/logs.
type LogMethod = (message: string, meta?: Record<string, any>) => void

export type RendererLogger = {
  error: LogMethod
  warn: LogMethod
  info: LogMethod
  debug: LogMethod
  verbose: LogMethod
}

export type RendererCategoryLogger = RendererLogger
