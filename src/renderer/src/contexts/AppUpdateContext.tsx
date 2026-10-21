import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

/**
 * In-app updates (see `commands/app_update.rs`): check at launch and every few hours, download
 * in the background, then offer a restart. Only the main window drives this; the commands are
 * granted to it alone, and other windows get a provider that never checks.
 */

export type AppUpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'upToDate' }
  | { state: 'downloading'; version: string }
  | { state: 'ready'; version: string }
  | { state: 'error'; message: string }

type AppUpdateContextValue = {
  status: AppUpdateStatus
  currentVersion: string
  autoUpdate: boolean
  setAutoUpdate: (value: boolean) => void
  /** Check now and, when there is an update, download it. */
  checkNow: () => Promise<void>
  restart: () => Promise<void>
  /** The banner was dismissed for this run. */
  dismissed: boolean
  dismiss: () => void
}

/** First check, once the app has settled after launch. */
const FIRST_CHECK_MS = 10_000
const CHECK_EVERY_MS = 6 * 60 * 60 * 1000

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

const AppUpdateContext = createContext<AppUpdateContextValue | null>(null)

export const AppUpdateProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  // Dev builds run from the Vite server and have nothing to update to.
  const enabled = !import.meta.env.DEV && getCurrentWindow().label === 'main'
  const [status, setStatus] = useState<AppUpdateStatus>({ state: 'idle' })
  const [currentVersion, setCurrentVersion] = useState('')
  const [autoUpdate, setAutoUpdateState] = useState(() => window.store.get('autoUpdate') !== false)
  const [dismissed, setDismissed] = useState(false)
  const busy = useRef(false)

  useEffect(() => {
    if (getCurrentWindow().label !== 'main') return
    window.api.appUpdate
      .currentVersion()
      .then(setCurrentVersion)
      .catch(() => {})
  }, [])

  const checkNow = useCallback(async () => {
    if (busy.current || getCurrentWindow().label !== 'main') return
    busy.current = true
    try {
      setStatus({ state: 'checking' })
      const result = await window.api.appUpdate.check()
      if (!result.available || !result.version) {
        setStatus({ state: 'upToDate' })
        return
      }
      setStatus({ state: 'downloading', version: result.version })
      const version = await window.api.appUpdate.download()
      setStatus({ state: 'ready', version })
    } catch (e) {
      setStatus({ state: 'error', message: errorText(e) })
    } finally {
      busy.current = false
    }
  }, [])

  // Automatic checks. A downloaded update is kept: once ready, there is nothing more to fetch.
  useEffect(() => {
    if (!enabled || !autoUpdate) return
    const first = window.setTimeout(() => void checkNow(), FIRST_CHECK_MS)
    const every = window.setInterval(() => void checkNow(), CHECK_EVERY_MS)
    return () => {
      window.clearTimeout(first)
      window.clearInterval(every)
    }
  }, [enabled, autoUpdate, checkNow])

  const setAutoUpdate = useCallback((value: boolean) => {
    setAutoUpdateState(value)
    window.store.set('autoUpdate', value)
  }, [])

  const restart = useCallback(async () => {
    try {
      await window.api.appUpdate.restart()
    } catch (e) {
      setStatus({ state: 'error', message: errorText(e) })
    }
  }, [])

  return (
    <AppUpdateContext.Provider
      value={{
        status,
        currentVersion,
        autoUpdate,
        setAutoUpdate,
        checkNow,
        restart,
        dismissed,
        dismiss: () => setDismissed(true)
      }}
    >
      {children}
    </AppUpdateContext.Provider>
  )
}

export const useAppUpdate = (): AppUpdateContextValue => {
  const value = useContext(AppUpdateContext)
  if (!value) throw new Error('useAppUpdate must be used inside AppUpdateProvider')
  return value
}
