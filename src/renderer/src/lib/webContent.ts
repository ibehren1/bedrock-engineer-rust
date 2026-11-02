import { useSyncExternalStore } from 'react'
import type { WebContentStatus } from '@/types/bridge/api'

/**
 * Whether web content in this window may load remote resources (AGENT-B5): while a proxy is on
 * that the app's web content isn't going through (proxy changed since startup, or an older macOS
 * that can't set one), images, previews and embedded editors must not load, since they would
 * connect without the proxy. Starts allowed until the first answer arrives.
 */
let status: WebContentStatus = { allowed: true }
const listeners = new Set<() => void>()
let started = false

const set = (next: WebContentStatus): void => {
  status = next
  listeners.forEach((l) => l())
}

const start = (): void => {
  if (started) return
  started = true
  const api = window.api?.webContent
  if (!api) return
  api.status().then(set, () => {})
  api.onChange(set)
}

const subscribe = (listener: () => void): (() => void) => {
  start()
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export const useWebContentStatus = (): WebContentStatus =>
  useSyncExternalStore(subscribe, () => status)

/** The current status, for code outside React (exports). */
export const currentWebContentStatus = (): WebContentStatus => {
  start()
  return status
}

/** `html` with remote (http/https) images replaced by their alt text. */
export const withoutRemoteImages = (html: string): string =>
  html.replace(/<img\b[^>]*\bsrc="https?:[^"]*"[^>]*>/gi, (tag) => {
    const alt = /\balt="([^"]*)"/i.exec(tag)?.[1] ?? ''
    return alt ? `<span>[${alt}]</span>` : ''
  })
