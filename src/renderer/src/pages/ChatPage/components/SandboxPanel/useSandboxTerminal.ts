import { useCallback, useEffect, useRef, useState } from 'react'
import type { Terminal } from '@xterm/xterm'

export interface TerminalCapability {
  supported: boolean
  reason?: string
  endpoint?: string
}

type TerminalMessage =
  | { type: 'data'; bytes: Uint8Array; truncated?: boolean }
  | { type: 'exit'; exitCode: number | null }
  | { type: 'error'; message: string }

/**
 * Attach an xterm instance to a shell in the chat's sandbox container.
 *
 * Ordering matters and is the whole reason this is a hook rather than inline effects:
 * `open` returns a channel and buffers output without publishing, then the subscription
 * goes up, and only then does `attach` release the backlog and start the live stream.
 * Publishing before the subscription existed would lose it — pubsub has no replay.
 */
export const useSandboxTerminal = (
  sessionId: string | undefined,
  terminal: Terminal | null,
  active: boolean,
  service?: string
) => {
  const [terminalId, setTerminalId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [exitCode, setExitCode] = useState<number | null | undefined>(undefined)
  const [attempt, setAttempt] = useState(0)
  const terminalIdRef = useRef<string | null>(null)
  const sizeRef = useRef<{ cols: number; rows: number }>({ cols: 80, rows: 24 })

  useEffect(() => {
    if (!active || !sessionId || !terminal) return

    let disposed = false
    let unsubscribe: (() => void) | undefined

    const start = async () => {
      try {
        setError(null)
        setExitCode(undefined)
        const opened = await window.api.dockerSandbox.terminal.open(sessionId, {
          service,
          cols: terminal.cols,
          rows: terminal.rows
        })
        if (disposed) return

        terminalIdRef.current = opened.terminalId
        setTerminalId(opened.terminalId)
        sizeRef.current = { cols: opened.cols, rows: opened.rows }

        unsubscribe = window.api.pubsub.subscribe(opened.channel, (message: TerminalMessage) => {
          if (message.type === 'data') {
            terminal.write(new Uint8Array(message.bytes))
            if (message.truncated) {
              terminal.write('\r\n\u001b[33m[output trimmed]\u001b[0m\r\n')
            }
            return
          }
          if (message.type === 'exit') {
            setExitCode(message.exitCode)
            terminal.write(
              `\r\n\u001b[90m[shell exited${
                message.exitCode === null ? '' : ` with code ${message.exitCode}`
              }]\u001b[0m\r\n`
            )
            return
          }
          setError(message.message)
        })

        // Only now is it safe for the main process to start publishing.
        const { backlog } = await window.api.dockerSandbox.terminal.attach(opened.terminalId)
        if (disposed) return
        if (backlog && backlog.byteLength > 0) terminal.write(new Uint8Array(backlog))
      } catch (caught) {
        if (!disposed) setError(caught instanceof Error ? caught.message : String(caught))
      }
    }

    void start()

    return () => {
      disposed = true
      unsubscribe?.()
    }
    // The shell deliberately outlives this effect: a `npm run dev` the user typed must not
    // die because they switched tabs or collapsed the panel. It is closed when the sandbox
    // stops, the chat is deleted, or the app quits.
    //
    // `attempt` is the only retry path. Nothing here re-opens on failure by itself: a shell
    // that dies on startup used to be reopened immediately, which spun as fast as Docker
    // could refuse it and filled the activity log.
  }, [active, sessionId, service, terminal, attempt])

  const sendInput = useCallback((data: string) => {
    const id = terminalIdRef.current
    if (!id) return
    void window.api.dockerSandbox.terminal.input(id, data)
  }, [])

  const resize = useCallback((cols: number, rows: number) => {
    const id = terminalIdRef.current
    if (!id) return
    if (sizeRef.current.cols === cols && sizeRef.current.rows === rows) return
    sizeRef.current = { cols, rows }
    void window.api.dockerSandbox.terminal.resize(id, cols, rows)
  }, [])

  const close = useCallback(() => {
    const id = terminalIdRef.current
    if (!id) return
    void window.api.dockerSandbox.terminal.close(id)
    terminalIdRef.current = null
    setTerminalId(null)
  }, [])

  /** Open a fresh shell. Only ever called from the button in the panel. */
  const reconnect = useCallback(() => {
    terminalIdRef.current = null
    setTerminalId(null)
    setAttempt((value) => value + 1)
  }, [])

  return { terminalId, error, exitCode, sendInput, resize, close, reconnect }
}

/** Whether a shell can be opened at all, and the reason when it cannot. */
export const useTerminalCapability = (enabled: boolean) => {
  const [capability, setCapability] = useState<TerminalCapability | null>(null)

  useEffect(() => {
    if (!enabled) return
    let cancelled = false

    void window.api.dockerSandbox.terminal
      .capability()
      .then((result) => {
        if (!cancelled) setCapability(result)
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setCapability({
            supported: false,
            reason: error instanceof Error ? error.message : String(error)
          })
        }
      })

    return () => {
      cancelled = true
    }
  }, [enabled])

  return capability
}
