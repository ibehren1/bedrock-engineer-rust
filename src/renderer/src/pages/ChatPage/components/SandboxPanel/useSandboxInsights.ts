import { useCallback, useEffect, useState } from 'react'

export interface SandboxInsights {
  containerName: string
  service: string
  image?: string
  imageId?: string
  status?: string
  startedAt?: string
  /** Percent of one core, as `docker stats` reports it: 200% means two cores. */
  cpuPercent?: number
  memoryUsed?: number
  memoryLimit?: number
  /** Cumulative network totals since the container started. */
  netRx?: number
  netTx?: number
  /** Throughput since the previous sample, in bytes per second. */
  netRxPerSecond?: number
  netTxPerSecond?: number
  /** Block IO. Undefined on hosts that do not report it, macOS included. */
  blockRead?: number
  blockWrite?: number
  blockReadPerSecond?: number
  blockWritePerSecond?: number
  error?: string
}

/**
 * Container identity and resource use for the Overview tab.
 *
 * Polled only while the panel is open, and only while the container is running: a CPU
 * percentage needs two samples, so the first response after opening the panel carries
 * memory but no CPU figure.
 */
export const useSandboxInsights = (
  sessionId: string | undefined,
  enabled: boolean,
  pollMs = 3000
) => {
  const [insights, setInsights] = useState<SandboxInsights | null>(null)

  const refresh = useCallback(async () => {
    if (!sessionId) return
    try {
      setInsights(await window.api.dockerSandbox.insights(sessionId))
    } catch (error) {
      setInsights({
        containerName: '',
        service: '',
        error: error instanceof Error ? error.message : String(error)
      })
    }
  }, [sessionId])

  useEffect(() => {
    if (!enabled || !sessionId) {
      setInsights(null)
      return
    }

    void refresh()
    const timer = setInterval(() => void refresh(), pollMs)
    return () => clearInterval(timer)
  }, [enabled, sessionId, pollMs, refresh])

  return { insights, refresh }
}
