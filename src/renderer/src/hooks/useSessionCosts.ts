import { useEffect, useMemo, useState } from 'react'
import type { SessionWindow } from '@/types/usage'

/**
 * Estimated cost of each session (or session window) from the usage ledger, in the order
 * given; refreshed when new usage is recorded. Empty until loaded or if the ledger is
 * unavailable.
 */
export function useSessionCosts(windows: SessionWindow[]): number[] {
  const key = useMemo(() => JSON.stringify(windows), [windows])
  const [costs, setCosts] = useState<number[]>([])

  useEffect(() => {
    const list: SessionWindow[] = JSON.parse(key)
    if (list.length === 0) {
      setCosts([])
      return
    }
    let live = true
    const load = () =>
      window.api.usage
        .sessionCosts(list)
        .then((rows) => live && setCosts(rows.map((r) => r.costUsd)))
        .catch(() => live && setCosts([]))
    load()
    const off = window.api.usage.onChanged(load)
    return () => {
      live = false
      off()
    }
  }, [key])

  return costs
}

/**
 * Windows for background-task runs: a session can be continued by later runs, so each run
 * covers the calls after the previous run of the same session finished, up to its own end
 * (`executedAt` is when a run finished).
 */
export function runWindows(runs: { sessionId: string; executedAt: number }[]): SessionWindow[] {
  return runs.map((run) => {
    const previous = runs
      .filter((r) => r.sessionId === run.sessionId && r.executedAt < run.executedAt)
      .reduce<number | undefined>(
        (max, r) => Math.max(max ?? r.executedAt, r.executedAt),
        undefined
      )
    return { sessionId: run.sessionId, fromTs: previous, toTs: run.executedAt }
  })
}
