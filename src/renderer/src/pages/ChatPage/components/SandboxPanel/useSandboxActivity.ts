import { useCallback, useEffect, useState } from 'react'

export type ActivityOutcome =
  | 'running'
  | 'completed'
  | 'requires-input'
  | 'detached'
  | 'timeout'
  | 'failed'

export interface SandboxActivityEntry {
  id: string
  sessionId: string
  service: string
  command: string
  source: 'agent' | 'user'
  outcome: ActivityOutcome
  startedAt: string
  endedAt?: string
  durationMs?: number
  exitCode?: number
  cwd?: string
  pid?: number
  stdoutBytes?: number
  stderrBytes?: number
  stdinBytes?: number
}

type ActivityEvent = { type: 'start' | 'settled' | 'exit'; entry: SandboxActivityEntry }

/**
 * Commands the agent ran in the sandbox, plus the user's own terminal sessions.
 *
 * Loaded once from the main process (which reads the persisted log when it has nothing in
 * memory), then kept current by pushed events. Each event replaces the row with the same
 * id, because a command is announced when it starts and updated as it settles and exits.
 */
export const useSandboxActivity = (sessionId: string | undefined, enabled: boolean) => {
  const [entries, setEntries] = useState<SandboxActivityEntry[]>([])

  const refresh = useCallback(async () => {
    if (!sessionId) {
      setEntries([])
      return
    }
    try {
      const result = await window.api.dockerSandbox.activity(sessionId)
      setEntries(result.entries ?? [])
    } catch {
      setEntries([])
    }
  }, [sessionId])

  useEffect(() => {
    if (!enabled) return
    void refresh()
  }, [enabled, refresh])

  useEffect(() => {
    if (!sessionId) return

    // Subscribed even while the panel is closed, so opening it does not start from an
    // empty list. Unsubscribe via the returned closure only.
    const unsubscribe = window.api.pubsub.subscribe(
      `docker-sandbox:activity:${sessionId}`,
      (event: ActivityEvent) => {
        if (!event?.entry) return
        setEntries((current) => {
          const index = current.findIndex((item) => item.id === event.entry.id)
          if (index === -1) return [...current, event.entry]
          const next = [...current]
          next[index] = event.entry
          return next
        })
      }
    )
    return unsubscribe
  }, [sessionId])

  return { entries, refresh }
}
