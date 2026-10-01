import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiExternalLink, FiFolder, FiPlay, FiRefreshCw, FiSquare, FiTrash2 } from 'react-icons/fi'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import { folderName } from '../../lib/folderName'
import type { SandboxInsights } from './useSandboxInsights'

type OverviewTabProps = {
  status: ChatSandboxStatus
  insights: SandboxInsights | null
  isBusy: boolean
  onStart: () => void
  onStop: () => void
  onRemove: (deleteData: boolean) => void
  onOpenFolder: () => void
  onOpenPort: (port: number) => void
}

const formatBytes = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`
}

const formatRate = (bytesPerSecond: number): string =>
  bytesPerSecond < 1 ? '0 B/s' : `${formatBytes(Math.round(bytesPerSecond))}/s`

/** Compact uptime: seconds under a minute, then minutes, then hours and days. */
const formatUptime = (startedAt: string): string | undefined => {
  const started = Date.parse(startedAt)
  if (Number.isNaN(started)) return undefined

  const seconds = Math.max(0, Math.floor((Date.now() - started) / 1000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ${minutes % 60}m`
  return `${Math.floor(hours / 24)}d ${hours % 24}h`
}

const Row: React.FC<{ label: string; children: React.ReactNode; title?: string }> = ({
  label,
  children,
  title
}) => (
  <>
    <dt className="text-ink-faint">{label}</dt>
    <dd className="m-0 text-ink break-all" title={title}>
      {children}
    </dd>
  </>
)

const PanelButton: React.FC<{
  onClick: () => void
  disabled?: boolean
  danger?: boolean
  children: React.ReactNode
}> = ({ onClick, disabled, danger, children }) => (
  <button
    onClick={onClick}
    disabled={disabled}
    className={`inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-control border border-strong bg-surface-2 text-ink-muted hover:text-ink hover:bg-raised disabled:opacity-50 disabled:cursor-not-allowed ${
      danger ? 'hover:text-danger hover:border-danger' : ''
    }`}
  >
    {children}
  </button>
)

const Meter: React.FC<{ label: string; value: string; percent?: number; hot?: boolean }> = ({
  label,
  value,
  percent,
  hot
}) => (
  <div>
    <div className="flex justify-between text-xs text-ink-faint mb-1">
      <span>{label}</span>
      <span className="text-ink font-medium tabular-nums">{value}</span>
    </div>
    <div className="h-1 rounded-full bg-raised overflow-hidden">
      <div
        className={`h-full ${hot ? 'bg-warning' : 'bg-accent'}`}
        style={{ width: `${Math.min(100, Math.max(0, percent ?? 0))}%` }}
      />
    </div>
  </div>
)

/**
 * A two-figure stat with no bar. Network and disk have no limit to fill, so a meter would
 * be inventing a denominator.
 */
const Stat: React.FC<{ label: string; primary: string; secondary?: string }> = ({
  label,
  primary,
  secondary
}) => (
  <div>
    <div className="flex justify-between text-xs text-ink-faint mb-1">
      <span>{label}</span>
      <span className="text-ink font-medium tabular-nums">{primary}</span>
    </div>
    {secondary && <div className="text-micro text-ink-faint tabular-nums">{secondary}</div>}
  </div>
)

export const OverviewTab: React.FC<OverviewTabProps> = ({
  status,
  insights,
  isBusy,
  onStart,
  onStop,
  onRemove,
  onOpenFolder,
  onOpenPort
}) => {
  const { t } = useTranslation()
  const isRunning = status.state === 'running' || status.state === 'partial'

  const memoryPercent =
    insights?.memoryUsed !== undefined && insights?.memoryLimit
      ? (insights.memoryUsed / insights.memoryLimit) * 100
      : undefined
  const uptime = insights?.startedAt ? formatUptime(insights.startedAt) : undefined

  return (
    <div className="text-sm">
      <section className="px-3 py-3 border-b border-subtle">
        <h4 className="m-0 mb-2 text-micro uppercase tracking-wide text-ink-faint font-medium">
          {t('dockerSandbox.panel.container')}
        </h4>
        <dl className="grid grid-cols-[6rem_1fr] gap-x-3 gap-y-1.5 m-0">
          <Row label={t('dockerSandbox.panel.name')} title={insights?.containerName}>
            <span className="font-mono text-xs">{insights?.containerName ?? '—'}</span>
          </Row>
          <Row label={t('dockerSandbox.panel.image')}>
            <span className="font-mono text-xs">{insights?.image ?? '—'}</span>
            {insights?.imageId && (
              <span className="ml-1.5 font-mono text-xs text-ink-faint">
                {insights.imageId.replace('sha256:', '').slice(0, 12)}
              </span>
            )}
          </Row>
          {uptime && (
            <Row label={t('dockerSandbox.panel.started')}>
              {t('dockerSandbox.panel.ago', { duration: uptime })}
            </Row>
          )}
          <Row label={t('dockerSandbox.panel.workspace')} title={status.metadata?.projectPath}>
            <span className="font-mono text-xs">
              /workspace{status.metadata?.projectPath ? ` → ${status.metadata.projectPath}` : ''}
            </span>
          </Row>
          {status.metadata?.directory && (
            <Row label={t('dockerSandbox.panel.folder')} title={status.metadata.directory}>
              <span className="font-mono text-xs">{folderName(status.metadata.directory)}</span>
            </Row>
          )}
        </dl>
      </section>

      <section className="px-3 py-3 border-b border-subtle">
        <h4 className="m-0 mb-2 text-micro uppercase tracking-wide text-ink-faint font-medium">
          {t('dockerSandbox.panel.resources')}
        </h4>
        {isRunning && !insights?.error ? (
          <div className="grid grid-cols-2 gap-x-3 gap-y-3.5">
            <Meter
              label={t('dockerSandbox.panel.cpu')}
              value={
                insights?.cpuPercent === undefined
                  ? t('dockerSandbox.panel.sampling')
                  : `${insights.cpuPercent.toFixed(1)}%`
              }
              percent={insights?.cpuPercent}
              hot={(insights?.cpuPercent ?? 0) > 80}
            />
            <Meter
              label={t('dockerSandbox.panel.memory')}
              value={
                insights?.memoryUsed === undefined
                  ? '—'
                  : `${formatBytes(insights.memoryUsed)} / ${formatBytes(insights.memoryLimit ?? 0)}`
              }
              percent={memoryPercent}
              hot={(memoryPercent ?? 0) > 85}
            />
            <Stat
              label={t('dockerSandbox.panel.network')}
              primary={
                insights?.netRxPerSecond === undefined
                  ? t('dockerSandbox.panel.sampling')
                  : `↓ ${formatRate(insights.netRxPerSecond)}  ↑ ${formatRate(insights.netTxPerSecond ?? 0)}`
              }
              secondary={
                insights?.netRx === undefined
                  ? undefined
                  : t('dockerSandbox.panel.totalInOut', {
                      in: formatBytes(insights.netRx),
                      out: formatBytes(insights.netTx ?? 0)
                    })
              }
            />
            <Stat
              label={t('dockerSandbox.panel.disk')}
              primary={
                // Undefined rather than zero when the host does not report block IO at
                // all, which is every macOS install: the containers run inside a VM.
                insights?.blockRead === undefined
                  ? t('dockerSandbox.panel.diskUnavailable')
                  : insights.blockReadPerSecond === undefined
                    ? t('dockerSandbox.panel.sampling')
                    : `↓ ${formatRate(insights.blockReadPerSecond)}  ↑ ${formatRate(insights.blockWritePerSecond ?? 0)}`
              }
              secondary={
                insights?.blockRead === undefined
                  ? undefined
                  : t('dockerSandbox.panel.totalReadWritten', {
                      read: formatBytes(insights.blockRead),
                      written: formatBytes(insights.blockWrite ?? 0)
                    })
              }
            />
          </div>
        ) : (
          <p className="m-0 text-xs text-ink-faint">
            {insights?.error ?? t('dockerSandbox.panel.notRunning')}
          </p>
        )}
        {isRunning && !insights?.error && insights?.blockRead !== undefined && (
          // Without this the figure reads as "how much disk is this using", which it is not:
          // /workspace and /data are bind mounts, and that IO is the host's.
          <p className="mt-2.5 mb-0 text-micro text-ink-faint">
            {t('dockerSandbox.panel.diskHint')}
          </p>
        )}
      </section>

      <section className="px-3 py-3 border-b border-subtle">
        <h4 className="m-0 mb-2 text-micro uppercase tracking-wide text-ink-faint font-medium">
          {t('dockerSandbox.panel.services')}
        </h4>
        {(status.metadata?.services ?? []).length === 0 ? (
          <p className="m-0 text-xs text-ink-faint">—</p>
        ) : (
          (status.metadata?.services ?? []).map((service) => {
            const container = status.containers.find((item) => item.service === service.name)
            const up = (container?.state ?? '').toLowerCase() === 'running'
            return (
              <div
                key={service.name}
                className="flex items-center gap-2 py-1.5 border-t border-subtle first:border-t-0"
              >
                <span
                  className={`size-1.5 rounded-full shrink-0 ${up ? 'bg-success' : 'bg-sunken'}`}
                />
                <span className="font-medium min-w-[3rem]">{service.name}</span>
                <span className="font-mono text-xs text-ink-faint truncate">{service.image}</span>
                <div className="ml-auto flex gap-1.5 flex-wrap justify-end">
                  {service.ports.map((port) => (
                    <button
                      key={`${port.host}-${port.container}`}
                      onClick={() => onOpenPort(port.host)}
                      title={t('dockerSandbox.panel.openPort', { port: port.host })}
                      className="inline-flex items-center gap-1 px-2 rounded-full border border-strong text-accent text-micro hover:bg-accent-tint"
                    >
                      localhost:{port.host} → {port.container}
                      <FiExternalLink className="size-2.5" />
                    </button>
                  ))}
                </div>
              </div>
            )
          })
        )}
      </section>

      <section className="px-3 py-3">
        <h4 className="m-0 mb-2 text-micro uppercase tracking-wide text-ink-faint font-medium">
          {t('dockerSandbox.panel.actions')}
        </h4>
        <div className="flex gap-1.5 flex-wrap">
          <PanelButton onClick={onOpenFolder} disabled={isBusy}>
            <FiFolder className="size-3" />
            {t('dockerSandbox.menu.openFolder')}
          </PanelButton>
          {isRunning ? (
            <PanelButton onClick={onStop} disabled={isBusy}>
              <FiSquare className="size-3" />
              {t('dockerSandbox.menu.stop')}
            </PanelButton>
          ) : (
            <PanelButton onClick={onStart} disabled={isBusy}>
              <FiPlay className="size-3" />
              {t('dockerSandbox.menu.start')}
            </PanelButton>
          )}
          {isRunning && (
            <PanelButton onClick={onStart} disabled={isBusy}>
              <FiRefreshCw className={`size-3 ${isBusy ? 'animate-spin' : ''}`} />
              {t('dockerSandbox.panel.restart')}
            </PanelButton>
          )}
          <PanelButton
            danger
            disabled={isBusy}
            onClick={() => {
              if (window.confirm(t('dockerSandbox.menu.confirmRemove'))) onRemove(false)
            }}
          >
            <FiTrash2 className="size-3" />
            {t('dockerSandbox.menu.remove')}
          </PanelButton>
          <PanelButton
            danger
            disabled={isBusy}
            onClick={() => {
              if (window.confirm(t('dockerSandbox.menu.confirmRemoveWithData'))) onRemove(true)
            }}
          >
            <FiTrash2 className="size-3" />
            {t('dockerSandbox.menu.removeWithData')}
          </PanelButton>
        </div>
      </section>
    </div>
  )
}
