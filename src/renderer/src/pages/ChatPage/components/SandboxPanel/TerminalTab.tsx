import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiAlertTriangle } from 'react-icons/fi'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
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

/**
 * The app's chosen monospace stack, resolved to a real font list.
 *
 * xterm measures character cells itself and takes a plain font-family string, so a CSS
 * `var(--font-mono)` never resolves for it — it silently falls back to the browser's
 * default monospace. Reading the variable off the document keeps the terminal on the font
 * picked in Settings → Appearance.
 */
const resolveMonoFont = (): string =>
  getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim() ||
  "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace"

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
  const fitRef = useRef<FitAddon | null>(null)
  const [terminal, setTerminal] = useState<Terminal | null>(null)
  const [acknowledged, setAcknowledged] = useState<boolean | null>(null)

  const isRunning = status.state === 'running' || status.state === 'partial'
  const capability = useTerminalCapability(active)
  const ready = active && isRunning && acknowledged === true && capability?.supported === true

  const { error, exitCode, sendInput, resize, reconnect } = useSandboxTerminal(
    sessionId,
    terminal,
    ready,
    service
  )

  useEffect(() => {
    void readAcknowledged().then(setAcknowledged)
  }, [])

  const acknowledge = useCallback(async () => {
    await writeAcknowledged()
    setAcknowledged(true)
  }, [])

  // Create the emulator once the host element exists and the gate has been passed.
  //
  // `terminal` must NOT be a dependency here even though the effect sets it: the cleanup
  // disposes the instance, so depending on it makes the effect tear down and rebuild what
  // it just created, forever. That loop also meant the emulator was disposed before it
  // could paint a prompt.
  useEffect(() => {
    if (!ready || !hostRef.current) return

    const instance = new Terminal({
      convertEol: false,
      cursorBlink: true,
      fontSize: 13,
      fontFamily: resolveMonoFont(),
      // JetBrains Mono Variable is loaded as a variable font; asking for 400/600 rather
      // than the defaults keeps bold text from being synthesised.
      fontWeight: 400,
      fontWeightBold: 600,
      lineHeight: 1.2,
      letterSpacing: 0,
      allowProposedApi: true,
      theme: { background: '#0b0e14', foreground: '#d3d7de', cursor: '#d3d7de' }
    })
    const fit = new FitAddon()
    instance.loadAddon(fit)
    instance.open(hostRef.current)
    fit.fit()

    fitRef.current = fit
    setTerminal(instance)

    // xterm measures one character to size its grid. If it does that before the variable
    // font has loaded it measures the fallback, and every glyph afterwards sits slightly
    // off its cell. Re-measure once the fonts are ready.
    void document.fonts?.ready.then(() => {
      try {
        instance.clearTextureAtlas?.()
        fit.fit()
      } catch {
        // The tab may already be gone; the ResizeObserver covers the next paint.
      }
    })

    return () => {
      // Disposing the emulator does not close the shell: that lives in the main process
      // and outlives this component on purpose.
      instance.dispose()
      fitRef.current = null
      setTerminal(null)
    }
  }, [ready])

  useEffect(() => {
    if (!terminal) return
    const disposable = terminal.onData(sendInput)
    return () => disposable.dispose()
  }, [terminal, sendInput])

  // Changing the Code font in Settings → Appearance flips data-font-mono on <html>, which
  // xterm cannot see: it holds the resolved family from when it was created.
  useEffect(() => {
    if (!terminal) return

    const observer = new MutationObserver(() => {
      const font = resolveMonoFont()
      if (terminal.options.fontFamily === font) return
      terminal.options.fontFamily = font
      // Cell metrics change with the font, so the size has to be recomputed.
      try {
        fitRef.current?.fit()
        resize(terminal.cols, terminal.rows)
      } catch {
        // A hidden tab has no measurable size; the ResizeObserver will catch up.
      }
    })
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-font-mono']
    })

    return () => observer.disconnect()
  }, [terminal, resize])

  // Fit from the element's own size, not from the panel-open flag: the drawer animates
  // for 300ms, so reacting to the flag would measure the wrong width.
  useEffect(() => {
    if (!terminal || !hostRef.current) return

    let frame: number | undefined
    const observer = new ResizeObserver(() => {
      if (frame) window.clearTimeout(frame)
      frame = window.setTimeout(() => {
        try {
          fitRef.current?.fit()
          resize(terminal.cols, terminal.rows)
        } catch {
          // Fitting a detached or zero-sized element throws; the next resize will retry.
        }
      }, 100)
    })
    observer.observe(hostRef.current)

    return () => {
      if (frame) window.clearTimeout(frame)
      observer.disconnect()
    }
  }, [terminal, resize])

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
      <div ref={hostRef} className="flex-1 min-h-0 bg-[#0b0e14] px-2 py-1.5" />
    </div>
  )
}
