import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiAlertTriangle } from 'react-icons/fi'
import { TERMINAL_BACKGROUND, useXterm } from '../Terminal/useXterm'
import { DEFAULT_SANDBOX_CONFIG } from '@/types/sandbox'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import { useSandboxTerminal, useTerminalCapability } from './useSandboxTerminal'

type TerminalTabProps = {
  sessionId?: string
  status: ChatSandboxStatus
  /** True while this tab is the visible one. */
  active: boolean
  /** Compose service to attach to. Undefined means the sandbox's first service. */
  service?: string
  onStart: () => void
}

/** Remembered once per install, under the existing dockerSandboxTool settings key. */
const readAcknowledged = async (): Promise<boolean> => {
  try {
    const config = await window.store.get('dockerSandboxTool')
    return config?.terminalAcknowledged === true
  } catch {
    return false
  }
}

const writeAcknowledged = async (): Promise<void> => {
  const config = await window.store.get('dockerSandboxTool')
  await window.store.set('dockerSandboxTool', {
    ...DEFAULT_SANDBOX_CONFIG,
    ...config,
    terminalAcknowledged: true
  })
}

const State: React.FC<{ title: string; children?: React.ReactNode }> = ({ title, children }) => (
  <div className="h-full flex flex-col items-center justify-center gap-2.5 p-7 text-center">
    <p className="m-0 text-sm font-medium text-ink">{title}</p>
    {children}
  </div>
)

export const TerminalTab: React.FC<TerminalTabProps> = ({
  sessionId,
  status,
  active,
  service,
  onStart
}) => {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement>(null)
  const [acknowledged, setAcknowledged] = useState<boolean | null>(null)
  // The emulator reports its size before the shell hook exists; route it through a ref.
  const resizeRef = useRef<(cols: number, rows: number) => void>(() => {})

  const isRunning = status.state === 'running' || status.state === 'partial'
  const capability = useTerminalCapability(active)
  const ready = active && isRunning && acknowledged === true && capability?.supported === true

  const terminal = useXterm(hostRef, ready, (cols, rows) => resizeRef.current(cols, rows))
  const { error, exitCode, sendInput, resize, reconnect } = useSandboxTerminal(
    sessionId,
    terminal,
    ready,
    service
  )
  resizeRef.current = resize

  useEffect(() => {
    void readAcknowledged().then(setAcknowledged)
  }, [])

  const acknowledge = useCallback(async () => {
    await writeAcknowledged()
    setAcknowledged(true)
  }, [])

  useEffect(() => {
    if (!terminal) return
    const disposable = terminal.onData(sendInput)
    return () => disposable.dispose()
  }, [terminal, sendInput])

  if (!isRunning) {
    return (
      <State title={t('dockerSandbox.terminal.stoppedTitle')}>
        <p className="m-0 max-w-[26rem] text-xs text-ink-muted">
          {t('dockerSandbox.terminal.stoppedBody')}
        </p>
        <button
          onClick={onStart}
          className="px-2.5 py-1 text-xs rounded-control bg-accent text-accent-fg hover:bg-accent-strong"
        >
          {t('dockerSandbox.menu.start')}
        </button>
      </State>
    )
  }

  if (capability && !capability.supported) {
    return (
      <State title={t('dockerSandbox.terminal.unavailableTitle')}>
        <p className="m-0 max-w-[28rem] text-xs text-ink-muted">{capability.reason}</p>
      </State>
    )
  }

  if (acknowledged === false) {
    return (
      <State title={t('dockerSandbox.terminal.ackTitle')}>
        <div className="text-left bg-surface-2 border border-subtle rounded-container px-3 py-2.5 max-w-[28rem]">
          <ul className="m-0 pl-4 list-disc">
            <li className="text-xs text-ink-muted mb-1">
              {t('dockerSandbox.terminal.ackWorkspace', {
                path: status.metadata?.projectPath ?? t('dockerSandbox.terminal.yourProjectFolder')
              })}
            </li>
            <li className="text-xs text-ink-muted mb-1">
              {t('dockerSandbox.terminal.ackNoFilter')}
            </li>
            <li className="text-xs text-ink-muted">{t('dockerSandbox.terminal.ackRoot')}</li>
          </ul>
        </div>
        <button
          onClick={() => void acknowledge()}
          className="px-2.5 py-1 text-xs rounded-control bg-accent text-accent-fg hover:bg-accent-strong"
        >
          {t('dockerSandbox.terminal.ackConfirm')}
        </button>
        <p className="m-0 text-micro text-ink-faint">{t('dockerSandbox.terminal.ackOnce')}</p>
      </State>
    )
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-1.5 px-3 py-1.5 bg-warning-soft text-warning text-micro border-b border-subtle">
        <FiAlertTriangle className="size-3 shrink-0" />
        <span>{t('dockerSandbox.terminal.warning')}</span>
        {status.metadata?.projectPath && (
          <span className="font-mono text-ink-muted truncate" title={status.metadata.projectPath}>
            /workspace = {status.metadata.projectPath}
          </span>
        )}
      </div>

      {(error || exitCode !== undefined) && (
        <div className="flex items-center gap-2 px-3 py-1 text-micro bg-danger-soft text-danger">
          <span className="truncate">
            {error ??
              t('dockerSandbox.terminal.exited', {
                code: exitCode === null ? '—' : exitCode
              })}
          </span>
          <button
            onClick={reconnect}
            className="ml-auto shrink-0 px-2 rounded-control border border-danger hover:bg-danger hover:text-canvas"
          >
            {t('dockerSandbox.terminal.reconnect')}
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
