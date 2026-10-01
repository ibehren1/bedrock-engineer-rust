import React from 'react'
import { useTranslation } from 'react-i18next'
import type { ActivityOutcome, SandboxActivityEntry } from './useSandboxActivity'

type ActivityTabProps = {
  entries: SandboxActivityEntry[]
}

const formatTime = (iso: string): string => {
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? '—' : date.toLocaleTimeString(undefined, { hour12: false })
}

const formatDuration = (ms: number): string =>
  ms < 1000 ? `${ms}ms` : ms < 60_000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.round(ms / 1000)}s`

const formatBytes = (bytes: number): string =>
  bytes < 1024
    ? `${bytes} B`
    : bytes < 1024 * 1024
      ? `${Math.round(bytes / 1024)} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`

/** Pill tone per outcome, using the same semantic colours as the rest of the app. */
const outcomeTone = (outcome: ActivityOutcome): string => {
  switch (outcome) {
    case 'completed':
      return 'bg-success-soft text-success'
    case 'failed':
      return 'bg-danger-soft text-danger'
    case 'requires-input':
    case 'timeout':
      return 'bg-warning-soft text-warning'
    case 'detached':
    case 'running':
    default:
      return 'bg-accent-tint text-accent'
  }
}

export const ActivityTab: React.FC<ActivityTabProps> = ({ entries }) => {
  const { t } = useTranslation()

  const outcomeLabel = (entry: SandboxActivityEntry): string => {
    switch (entry.outcome) {
      case 'completed':
      case 'failed':
        return entry.exitCode === undefined
          ? t(`dockerSandbox.activity.outcome.${entry.outcome}`)
          : t('dockerSandbox.activity.exitCode', { code: entry.exitCode })
      case 'requires-input':
        return t('dockerSandbox.activity.outcome.requiresInput')
      case 'timeout':
        return t('dockerSandbox.activity.outcome.timeout')
      case 'detached':
        return t('dockerSandbox.activity.outcome.detached')
      default:
        return t('dockerSandbox.activity.outcome.running')
    }
  }

  if (entries.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2 p-7 text-center">
        <p className="m-0 text-sm text-ink-muted">{t('dockerSandbox.activity.empty')}</p>
        <p className="m-0 text-xs text-ink-faint">{t('dockerSandbox.activity.emptyHint')}</p>
      </div>
    )
  }

  // Newest first: the reason to open this tab is almost always the most recent command.
  const ordered = [...entries].reverse()

  return (
    <div className="text-sm">
      {ordered.map((entry) => (
        <div
          key={entry.id}
          className="flex gap-2.5 px-3 py-2 border-b border-subtle hover:bg-surface-2"
        >
          <div className="text-micro text-ink-faint pt-0.5 min-w-[3rem] tabular-nums">
            {formatTime(entry.startedAt)}
          </div>
          <div className="flex-1 min-w-0">
            <div className="font-mono text-xs text-ink truncate" title={entry.command}>
              {entry.command}
            </div>
            <div className="flex gap-2 items-center mt-0.5 text-micro text-ink-faint flex-wrap">
              <span
                className={`px-1.5 rounded-full border ${
                  entry.source === 'user' ? 'text-accent border-accent' : 'border-strong'
                }`}
              >
                {t(`dockerSandbox.activity.source.${entry.source}`)}
              </span>
              <span className={`px-1.5 rounded-full ${outcomeTone(entry.outcome)}`}>
                {outcomeLabel(entry)}
              </span>
              {entry.durationMs !== undefined && <span>{formatDuration(entry.durationMs)}</span>}
              {entry.pid !== undefined && <span>pid {entry.pid}</span>}
              {entry.stdoutBytes !== undefined && entry.stdoutBytes > 0 && (
                <span>stdout {formatBytes(entry.stdoutBytes)}</span>
              )}
              {entry.stdinBytes !== undefined && (
                // Length only — the text is deliberately never recorded.
                <span>{t('dockerSandbox.activity.stdinSent', { bytes: entry.stdinBytes })}</span>
              )}
            </div>
          </div>
        </div>
      ))}
      <p className="px-3 py-2 m-0 text-micro text-ink-faint">
        {t('dockerSandbox.activity.storageNote')}
      </p>
    </div>
  )
}
