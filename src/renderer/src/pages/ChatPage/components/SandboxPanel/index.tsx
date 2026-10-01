import React, { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FaDocker } from 'react-icons/fa'
import toast from 'react-hot-toast'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import { ActivityTab } from './ActivityTab'
import { ComposeTab } from './ComposeTab'
import { OverviewTab } from './OverviewTab'
import { TerminalTab } from './TerminalTab'
import { useSandboxActivity } from './useSandboxActivity'
import { useSandboxInsights } from './useSandboxInsights'

type SandboxPanelProps = {
  sessionId?: string
  status: ChatSandboxStatus
  isBusy: boolean
  isOpen: boolean
  onStart: () => void
  onStop: () => void
  onRemove: (deleteData: boolean) => void
  onOpenFolder: () => void
}

type TabKey = 'overview' | 'compose' | 'terminal' | 'activity'

/** Status pill styling, mirroring the tones used by StatusBadge elsewhere. */
const stateTone = (state: ChatSandboxStatus['state']): string => {
  switch (state) {
    case 'running':
      return 'bg-success-soft text-success'
    case 'partial':
      return 'bg-warning-soft text-warning'
    case 'missing':
      return 'bg-danger-soft text-danger'
    default:
      return 'bg-raised text-ink-faint'
  }
}

export const SandboxPanel: React.FC<SandboxPanelProps> = ({
  sessionId,
  status,
  isBusy,
  isOpen,
  onStart,
  onStop,
  onRemove,
  onOpenFolder
}) => {
  const { t } = useTranslation()
  const [tab, setTab] = useState<TabKey>('overview')

  const services = useMemo(
    () => (status.metadata?.services ?? []).map((service) => service.name),
    [status.metadata?.services]
  )
  const [service, setService] = useState<string | undefined>(undefined)

  // Follow the sandbox: a service can disappear when a stack is recreated, and the first
  // status arrives after this component mounts.
  useEffect(() => {
    if (services.length === 0) {
      setService(undefined)
      return
    }
    setService((current) => (current && services.includes(current) ? current : services[0]))
  }, [services])

  // Resource figures cost a Docker round trip every few seconds, so they are only
  // gathered while the panel is actually showing them.
  const { insights } = useSandboxInsights(sessionId, isOpen && tab === 'overview')
  const { entries } = useSandboxActivity(sessionId, isOpen)

  // Only compose-driven sandboxes have a file and a layout worth drawing. Without compose
  // the sandbox is a single `docker run` container.
  const hasCompose = status.metadata ? !status.metadata.composeless : false

  const stateLabel = () => {
    switch (status.state) {
      case 'running':
        return t('dockerSandbox.menu.stateRunning')
      case 'partial':
        return t('dockerSandbox.menu.statePartial')
      case 'missing':
        return t('dockerSandbox.panel.stateMissing')
      default:
        return t('dockerSandbox.menu.stateStopped')
    }
  }

  const openPort = async (port: number) => {
    if (!sessionId) return
    const result = await window.api.dockerSandbox.openPort(sessionId, port)
    if (!result.success) toast.error(result.error ?? t('dockerSandbox.panel.openPortFailed'))
  }

  const tabs: { key: TabKey; label: string; count?: number }[] = [
    { key: 'overview', label: t('dockerSandbox.panel.tabOverview') },
    ...(hasCompose
      ? [{ key: 'compose' as TabKey, label: t('dockerSandbox.panel.tabCompose') }]
      : []),
    { key: 'terminal', label: t('dockerSandbox.panel.tabTerminal') },
    { key: 'activity', label: t('dockerSandbox.panel.tabActivity'), count: entries.length }
  ]

  return (
    <div className="w-[40rem] h-full flex flex-col bg-surface border border-subtle rounded-container shadow-raised overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-subtle">
        <FaDocker className="text-[#2496ED] size-4 shrink-0" />
        <div className="min-w-0">
          <div className="text-sm font-medium text-ink">{t('dockerSandbox.panel.title')}</div>
          <div className="text-xs text-ink-faint truncate" title={status.metadata?.projectName}>
            {status.metadata?.projectName ?? '—'}
          </div>
        </div>
        <span
          className={`ml-auto inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-micro shrink-0 ${stateTone(
            status.state
          )}`}
        >
          {status.state === 'running' && (
            // Pulsing dot, same idiom the chat history uses for "still responding".
            <span className="relative flex size-1.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-current opacity-75" />
              <span className="relative inline-flex size-1.5 rounded-full bg-current" />
            </span>
          )}
          {stateLabel()}
        </span>
      </div>

      <div className="flex gap-0.5 px-2 pt-1.5 border-b border-subtle">
        {tabs.map((item) => (
          <button
            key={item.key}
            onClick={() => setTab(item.key)}
            className={`px-2.5 py-1 text-xs -mb-px border-b-2 ${
              tab === item.key
                ? 'text-ink border-accent'
                : 'text-ink-faint border-transparent hover:text-ink'
            }`}
          >
            {item.label}
            {item.count !== undefined && item.count > 0 && (
              <span className="ml-1 text-micro text-ink-faint">{item.count}</span>
            )}
          </button>
        ))}
      </div>

      <div className="flex-1 min-h-0 overflow-auto">
        {tab === 'overview' && (
          <OverviewTab
            status={status}
            insights={insights}
            isBusy={isBusy}
            onStart={onStart}
            onStop={onStop}
            onRemove={onRemove}
            onOpenFolder={onOpenFolder}
            onOpenPort={(port) => void openPort(port)}
          />
        )}
        {tab === 'compose' && (
          <ComposeTab
            sessionId={sessionId}
            status={status}
            active={isOpen && tab === 'compose'}
            onOpenFolder={onOpenFolder}
          />
        )}
        {/* Kept mounted so a shell the user is watching is not torn down by a tab switch. */}
        <div className={tab === 'terminal' ? 'h-full flex flex-col' : 'hidden'}>
          {services.length > 1 && (
            <div className="flex gap-1 px-2 py-1 border-b border-subtle bg-surface-2 shrink-0">
              {services.map((name) => {
                const container = status.containers.find((item) => item.service === name)
                const up = (container?.state ?? '').toLowerCase() === 'running'
                return (
                  <button
                    key={name}
                    onClick={() => setService(name)}
                    className={`inline-flex items-center gap-1.5 px-2 py-0.5 text-micro rounded-control ${
                      service === name
                        ? 'bg-surface text-ink border border-strong'
                        : 'text-ink-faint hover:text-ink border border-transparent'
                    }`}
                  >
                    <span className={`size-1.5 rounded-full ${up ? 'bg-success' : 'bg-sunken'}`} />
                    {name}
                  </button>
                )
              })}
            </div>
          )}
          {/* Keyed on the service so switching builds a fresh emulator for that container;
              the shell itself lives in the main process and its scrollback is replayed. */}
          <div className="flex-1 min-h-0">
            <TerminalTab
              key={service ?? 'default'}
              sessionId={sessionId}
              status={status}
              active={isOpen && tab === 'terminal'}
              service={service}
              onStart={onStart}
            />
          </div>
        </div>
        {tab === 'activity' && <ActivityTab entries={entries} />}
      </div>
    </div>
  )
}
