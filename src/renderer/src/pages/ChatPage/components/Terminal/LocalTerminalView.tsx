import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { LocalTerminalInfo } from '@/types/bridge/api'
import { TERMINAL_BACKGROUND, useXterm } from './useXterm'

type TerminalMessage =
  | { type: 'data'; bytes: Uint8Array; truncated?: boolean }
  | { type: 'exit'; exitCode: number | null }
  | { type: 'error'; message: string }

type LocalTerminalViewProps = {
  info: LocalTerminalInfo
  /** This shell's view is the one showing. */
  active: boolean
  onExit: (exitCode: number | null) => void
  onClose: () => void
}

/**
 * One local shell. Ordering matters, as for the sandbox terminal: subscribe to the shell's
 * channel first, then `attach`, which returns what it printed so far and turns on the live
 * stream. The other way round would lose output, since pub/sub has no replay.
 */
export const LocalTerminalView: React.FC<LocalTerminalViewProps> = ({
  info,
  active,
  onExit,
  onClose
}) => {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement>(null)
  const sizeRef = useRef({ cols: 0, rows: 0 })
  const [error, setError] = useState<string | null>(null)
  // Kept in refs so a new callback or language does not resubscribe (and replay the backlog).
  const onExitRef = useRef(onExit)
  onExitRef.current = onExit
  const tRef = useRef(t)
  tRef.current = t

  const resize = useCallback(
    (cols: number, rows: number) => {
      if (sizeRef.current.cols === cols && sizeRef.current.rows === rows) return
      sizeRef.current = { cols, rows }
      void window.api.localTerminal.resize(info.terminalId, cols, rows)
    },
    [info.terminalId]
  )

  // Created on first view and kept after, so scrollback survives switching between shells.
  const [viewed, setViewed] = useState(active)
  useEffect(() => {
    if (active) setViewed(true)
  }, [active])
  const terminal = useXterm(hostRef, viewed, resize)

  useEffect(() => {
    if (!terminal) return
    let disposed = false
    resize(terminal.cols, terminal.rows)

    const unsubscribe = window.api.pubsub.subscribe(info.channel, (message: TerminalMessage) => {
      if (message.type === 'data') {
        terminal.write(new Uint8Array(message.bytes))
        if (message.truncated) {
          terminal.write('\r\n\u001b[33m[output trimmed]\u001b[0m\r\n')
        }
        return
      }
      if (message.type === 'exit') {
        terminal.write(
          `\r\n\u001b[90m[${tRef.current('filesPanel.terminalExited', {
            code: message.exitCode === null ? '—' : message.exitCode
          })}]\u001b[0m\r\n`
        )
        onExitRef.current(message.exitCode)
        return
      }
      setError(message.message)
    })

    window.api.localTerminal
      .attach(info.terminalId)
      .then(({ backlog, exited, exitCode }) => {
        if (disposed) return
        if (backlog && backlog.byteLength > 0) terminal.write(backlog)
        // Exited before we subscribed, so its exit message was never seen.
        if (exited) onExitRef.current(exitCode)
      })
      .catch((caught) => {
        if (!disposed) setError(caught instanceof Error ? caught.message : String(caught))
      })

    const input = terminal.onData((data) => {
      void window.api.localTerminal.input(info.terminalId, data)
    })

    return () => {
      disposed = true
      unsubscribe()
      input.dispose()
    }
  }, [terminal, info.terminalId, info.channel, resize])

  useEffect(() => {
    if (active) terminal?.focus()
  }, [active, terminal])

  return (
    <div className="h-full flex flex-col">
      {(error || info.exited) && (
        <div className="flex items-center gap-2 px-3 py-1 text-micro bg-danger-soft text-danger">
          <span className="truncate">
            {error ??
              t('filesPanel.terminalExited', {
                code: info.exitCode === null ? '—' : info.exitCode
              })}
          </span>
          <button
            onClick={onClose}
            className="ml-auto shrink-0 px-2 rounded-control border border-danger hover:bg-danger hover:text-canvas"
          >
            {t('filesPanel.terminalClose')}
          </button>
        </div>
      )}
      {/* xterm measures this element, so it must have a real size of its own. */}
      <div
        ref={hostRef}
        className="flex-1 min-h-0 px-2 py-1.5"
        style={{ background: TERMINAL_BACKGROUND }}
      />
    </div>
  )
}
