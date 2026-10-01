import { useCallback, useEffect, useState } from 'react'

export interface ChatSandboxStatus {
  exists: boolean
  state: 'running' | 'stopped' | 'partial' | 'missing'
  metadata?: {
    projectName: string
    directory: string
    /** Host folder bind-mounted at /workspace — the user's real project directory. */
    projectPath?: string
    services: { name: string; image: string; ports: { host: number; container: number }[] }[]
    composeless: boolean
  }
  containers: { service: string; name: string; state: string; ports: string }[]
}

const EMPTY: ChatSandboxStatus = { exists: false, state: 'missing', containers: [] }

/**
 * Track whether the current chat has a Docker sandbox, so the toolbar can show its
 * controls only when there is something to control.
 *
 * Sandboxes are created lazily by the agent's first command, so this polls at a low
 * cadence while a chat is open rather than only on mount. `refresh` is exposed for
 * immediate updates after a stop/remove action.
 *
 * The main process also pushes state on create/start/stop/remove, which is what makes a
 * sandbox appear the moment the agent's first command creates it. The poll stays as
 * well: pushes are dropped when nothing is subscribed, and only a poll notices a
 * container the user stopped from their own terminal or a daemon that went away.
 */
export const useChatSandbox = (sessionId?: string, pollMs = 15_000) => {
  const [status, setStatus] = useState<ChatSandboxStatus>(EMPTY)
  const [isBusy, setIsBusy] = useState(false)

  const refresh = useCallback(async () => {
    if (!sessionId) {
      setStatus(EMPTY)
      return EMPTY
    }
    try {
      const result = await window.api.dockerSandbox.status(sessionId)
      setStatus(result)
      return result as ChatSandboxStatus
    } catch {
      // A missing sandbox and an unreachable Docker daemon look the same from here;
      // either way there is nothing for the toolbar to offer.
      setStatus(EMPTY)
      return EMPTY
    }
  }, [sessionId])

  useEffect(() => {
    void refresh()

    if (!sessionId) return
    const timer = setInterval(() => void refresh(), pollMs)
    return () => clearInterval(timer)
  }, [refresh, sessionId, pollMs])

  useEffect(() => {
    if (!sessionId) return

    // Always unsubscribe with the closure this returns: window.api.pubsub.unsubscribe
    // calls removeAllListeners for the channel, which would take out any other listener.
    const unsubscribe = window.api.pubsub.subscribe(
      `docker-sandbox:state:${sessionId}`,
      (event: { type: string; status?: ChatSandboxStatus }) => {
        if (event?.type === 'state' && event.status) setStatus(event.status)
      }
    )
    return unsubscribe
  }, [sessionId])

  const stop = useCallback(async () => {
    if (!sessionId) return
    setIsBusy(true)
    try {
      await window.api.dockerSandbox.stop(sessionId)
      await refresh()
    } finally {
      setIsBusy(false)
    }
  }, [sessionId, refresh])

  const start = useCallback(async () => {
    if (!sessionId) return
    setIsBusy(true)
    try {
      await window.api.dockerSandbox.start(sessionId)
      await refresh()
    } finally {
      setIsBusy(false)
    }
  }, [sessionId, refresh])

  const remove = useCallback(
    async (deleteData: boolean) => {
      if (!sessionId) return
      setIsBusy(true)
      try {
        await window.api.dockerSandbox.remove(sessionId, { deleteData })
        await refresh()
      } finally {
        setIsBusy(false)
      }
    },
    [sessionId, refresh]
  )

  const openFolder = useCallback(async () => {
    if (!sessionId) return
    const result = await window.api.dockerSandbox.openFolder(sessionId)
    if (!result.success) {
      throw new Error(result.error ?? 'Could not open the sandbox folder')
    }
  }, [sessionId])

  return { status, isBusy, refresh, stop, start, remove, openFolder }
}
